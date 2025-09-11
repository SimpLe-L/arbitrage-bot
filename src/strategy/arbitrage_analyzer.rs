//! 套利机会分析器 - 负责分析和寻找套利机会

use std::sync::Arc;
use ethers::types::Address;
use eyre::Result;
use object_pool::ObjectPool;
use tracing::{info, warn};

use crate::{
    strategy::{ArbStrategy, arb::Arb},
    common::get_latest_block,
    simulator::{SimulateCtx, SimEpoch, HttpSimulator, Simulator},
    types::Source,
    dex::{Defi, TradeType},
    utils::token_config::TokenConfig,
};

/// 套利机会结构
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub token_address: String,
    pub token_name: String,
    pub path_description: String,
    pub involved_dexes: Vec<String>,
    pub amount_in: u64,
    pub estimated_profit: u64,
    pub gas_cost: u64,
    pub net_profit: u64,
    pub profit_percentage: f64,
}

impl ArbitrageOpportunity {
    pub fn new(
        token_address: String,
        path_description: String,
        involved_dexes: Vec<String>,
        amount_in: u64,
        estimated_profit: u64,
    ) -> Self {
        let gas_cost = 1_000_000_000_000_000u64; // 0.001 AVAX估算gas费用
        let net_profit = estimated_profit.saturating_sub(gas_cost);
        let profit_percentage = if amount_in > 0 {
            (net_profit as f64 / amount_in as f64) * 100.0
        } else {
            0.0
        };
        
        // 从TokenConfig获取代币名称
        let token_config = TokenConfig::new();
        let token_name = if let Some(token_info) = token_config.get_token_by_address(&token_address) {
            token_info.symbol.clone()
        } else {
            "Unknown".to_string()
        };

        Self {
            token_address,
            token_name,
            path_description,
            involved_dexes,
            amount_in,
            estimated_profit,
            gas_cost,
            net_profit,
            profit_percentage,
        }
    }

    pub fn display(&self) {
        println!("\n🔍 ===== 发现套利机会！ =====");
        println!("💰 代币: {} ({})", self.token_name, self.token_address);
        println!("🔄 路径: {}", self.path_description);
        println!("🏪 涉及DEX: {}", self.involved_dexes.join(", "));
        println!("💵 交易金额: {:.4} AVAX", self.amount_in as f64 / 1e18);
        println!("📈 预估利润: {:.4} AVAX", self.estimated_profit as f64 / 1e18);
        println!("⛽ Gas费用: {:.4} AVAX", self.gas_cost as f64 / 1e18);
        println!("✨ 净利润: {:.4} AVAX ({:.2}%)", self.net_profit as f64 / 1e18, self.profit_percentage);
        println!("===============================\n");
    }
}

/// 套利机会分析器
pub struct ArbitrageAnalyzer {
    token_config: TokenConfig,
}

impl ArbitrageAnalyzer {
    pub fn new() -> Self {
        Self {
            token_config: TokenConfig::new(),
        }
    }

    /// 寻找套利机会的完善版本
    /// 使用现有的Defi模块进行路径查找和利润计算
    pub async fn find_arbitrage_opportunity(
        &self,
        _arb_strategy: &ArbStrategy,
        token_address: &str,
        sender: Address,
        rpc_url: &str,
    ) -> Result<Option<ArbitrageOpportunity>> {
        use std::str::FromStr;
        
        // 检查token地址格式
        let _token_addr = match Address::from_str(token_address) {
            Ok(addr) => addr,
            Err(_) => {
                warn!("Invalid token address format: {}", token_address);
                return Ok(None);
            }
        };
        
        // 创建一个临时的模拟器池用于套利检测
        let simulator_pool: ObjectPool<Box<dyn Simulator>> = ObjectPool::new(2, move || {
            let rpc_url = rpc_url.to_string();
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { 
                    Box::new(HttpSimulator::new(&rpc_url).await) as Box<dyn Simulator> 
                })
        });
        
        // 创建Defi实例进行路径分析
        let defi = match Defi::new(rpc_url, Arc::new(simulator_pool)).await {
            Ok(defi) => defi,
            Err(e) => {
                warn!("Failed to create Defi instance: {}", e);
                return Ok(None);
            }
        };
        
        // 获取当前区块信息
        let block_number = match get_latest_block(rpc_url).await {
            Ok(block) => block,
            Err(e) => {
                warn!("Failed to get latest block: {}", e);
                return Ok(None);
            }
        };
        
        let epoch = SimEpoch {
            block_number: block_number.as_u64(),
            block_timestamp: 0,
            base_fee: ethers::types::U256::from(25_000_000_000u64), // 25 gwei
            gas_limit: ethers::types::U256::from(30_000_000u64),
        };
        let sim_ctx = SimulateCtx::new(epoch);
        
        info!("Analyzing arbitrage opportunity for token: {}", token_address);
        
        // 查找可能的套利路径
        let arbitrage_paths = match defi.find_sell_paths_with_hops(token_address, 2).await {
            Ok(paths) => paths,
            Err(e) => {
                // 没有路径不一定是错误，可能只是这个代币没有套利机会
                if !e.to_string().contains("no arbitrage paths") {
                    warn!("Error finding arbitrage paths for {}: {}", token_address, e);
                }
                return Ok(None);
            }
        };
        
        if arbitrage_paths.is_empty() {
            return Ok(None);
        }
        
        // 尝试不同的交易金额
        let test_amounts = [
            100_000_000_000_000_000u64,   // 0.1 AVAX
            500_000_000_000_000_000u64,   // 0.5 AVAX  
            1_000_000_000_000_000_000u64, // 1.0 AVAX
            5_000_000_000_000_000_000u64, // 5.0 AVAX
        ];
        
        let mut best_opportunity: Option<ArbitrageOpportunity> = None;
        let gas_limit = 300_000u64;
        
        for &amount_in in &test_amounts {
            // 使用现有的路径查找最佳交易结果
            if let Ok(best_result) = defi.find_best_path_exact_in(
                &arbitrage_paths,
                sender,
                amount_in,
                TradeType::Flashloan, // 使用闪电贷进行套利
                gas_limit,
                &sim_ctx,
            ).await {
                let profit = best_result.profit();
                
                if profit > 0 {
                    // 构建真实的路径描述和DEX信息
                    let (path_description, involved_dexes) = self.build_path_info(&best_result.path);
                    
                    let opportunity = ArbitrageOpportunity::new(
                        token_address.to_string(),
                        path_description,
                        involved_dexes,
                        amount_in,
                        profit as u64,
                    );
                    
                    // 保留最佳机会
                    if best_opportunity.is_none() || profit > best_opportunity.as_ref().unwrap().estimated_profit as i128 {
                        best_opportunity = Some(opportunity);
                    }
                    
                    info!("Found profitable opportunity: amount_in={}, profit={}", amount_in, profit);
                }
            }
        }
        
        Ok(best_opportunity)
    }

    /// 从路径中构建描述信息和涉及的DEX列表
    fn build_path_info(&self, path: &crate::dex::Path) -> (String, Vec<String>) {
        if path.path.is_empty() {
            return ("Empty path".to_string(), vec![]);
        }
        
        let mut path_steps = vec![];
        let mut involved_dexes = vec![];
        
        for (i, dex) in path.path.iter().enumerate() {
            let protocol_name = format!("{:?}", dex.protocol());
            if !involved_dexes.contains(&protocol_name) {
                involved_dexes.push(protocol_name);
            }
            
            let coin_in = self.extract_token_symbol(&dex.coin_in_type());
            let coin_out = self.extract_token_symbol(&dex.coin_out_type());
            
            if i == 0 {
                path_steps.push(format!("Buy {} with {}", coin_out, coin_in));
            } else if i == path.path.len() - 1 {
                path_steps.push(format!("Sell {} for {}", coin_in, coin_out));
            } else {
                path_steps.push(format!("Swap {} → {}", coin_in, coin_out));
            }
        }
        
        let path_description = if path_steps.len() <= 2 {
            path_steps.join(" → ")
        } else {
            format!("{} → ... → {}", path_steps.first().unwrap(), path_steps.last().unwrap())
        };
        
        (path_description, involved_dexes)
    }

    /// 从完整代币类型中提取简短的符号
    fn extract_token_symbol(&self, coin_type: &str) -> String {
        // 如果是AVAX原生代币
        if crate::utils::coin::is_native_coin(coin_type) {
            return "AVAX".to_string();
        }
        
        // 从配置中获取代币信息
        if let Some(token_info) = self.token_config.get_token_by_address(coin_type) {
            return token_info.symbol.clone();
        }
        
        // 如果是未知代币，尝试从地址提取简短格式
        if coin_type.len() > 10 {
            format!("{}...{}", &coin_type[0..6], &coin_type[coin_type.len()-4..])
        } else {
            coin_type.to_string()
        }
    }
}

impl Default for ArbitrageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
