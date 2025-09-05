//! 模拟执行器实现

use super::{traits::TransactionExecutor, types::ExecutionResult};
use crate::core::types::{ArbitragePath, BotError, Result};
use async_trait::async_trait;
use ethers::types::{H256, U256};
use log::{info, error, debug};

/// 模拟执行器 - 用于测试和演示
pub struct MockExecutor {
    name: String,
    simulate_success: bool,
    simulate_delay_ms: u64,
}

impl MockExecutor {
    pub fn new(name: String) -> Self {
        Self {
            name,
            simulate_success: true,
            simulate_delay_ms: 100,
        }
    }
    
    /// 设置是否模拟成功执行
    pub fn with_simulate_success(mut self, success: bool) -> Self {
        self.simulate_success = success;
        self
    }
    
    /// 设置模拟延迟
    pub fn with_simulate_delay(mut self, delay_ms: u64) -> Self {
        self.simulate_delay_ms = delay_ms;
        self
    }
}

#[async_trait]
impl TransactionExecutor for MockExecutor {
    async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult> {
        info!("🚀 模拟执行套利交易:");
        info!("{}", path);
        
        // 模拟执行延迟
        if self.simulate_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.simulate_delay_ms)).await;
        }
        
        let result = if self.simulate_success {
            ExecutionResult::success(
                H256::random(),
                path.gas_estimate,
                path.net_profit,
                self.name.clone(),
            )
        } else {
            ExecutionResult::failure(
                "模拟执行失败".to_string(),
                self.name.clone(),
            )
        };
        
        if result.success {
            info!("✅ 套利交易执行成功!");
            info!("   交易哈希: {:?}", result.transaction_hash.unwrap());
            info!("   Gas使用: {} wei", result.gas_used.unwrap_or_default());
            info!("   实际利润: {} wei ({:.6} AVAX)", 
                result.actual_profit.unwrap_or_default(), 
                result.actual_profit.unwrap_or_default().as_u128() as f64 / 1e18
            );
        } else {
            error!("❌ 套利交易执行失败: {}", result.error_message.as_ref().unwrap());
        }
        
        Ok(result)
    }
    
    async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256> {
        // 简单的gas估算：基础gas + 每跳额外gas
        let base_gas = U256::from(21000); // 基础转账gas
        let per_hop_gas = U256::from(60000); // 每跳大约消耗的gas
        let hops = U256::from(path.pools.len());
        
        let estimated_gas = base_gas + per_hop_gas * hops;
        
        debug!("Gas估算: {} hops, 预计消耗 {} gas", path.pools.len(), estimated_gas);
        
        Ok(estimated_gas)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        true // 模拟执行器总是可用
    }
}

/// 打印执行器 - 只打印套利信息，不执行实际交易
pub struct PrintExecutor {
    name: String,
}

impl PrintExecutor {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl TransactionExecutor for PrintExecutor {
    async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult> {
        info!("🔍 发现套利机会!");
        info!("==================================================");
        info!("{}", path);
        info!("==================================================");
        
        // 详细信息打印
        info!("📊 套利路径详细信息:");
        for (i, pool) in path.pools.iter().enumerate() {
            let hop_num = i + 1;
            let amount_in = if i < path.amounts_in.len() { 
                path.amounts_in[i] 
            } else { 
                U256::zero() 
            };
            let amount_out = if i < path.amounts_out.len() { 
                path.amounts_out[i] 
            } else { 
                U256::zero() 
            };
            
            info!("   跳 {}: {} -> {} ({})", 
                hop_num,
                pool.token0.symbol,
                pool.token1.symbol,
                pool.dex
            );
            info!("         池地址: {:?}", pool.address);
            info!("         输入: {} wei", amount_in);
            info!("         输出: {} wei", amount_out);
            info!("         手续费: {}%", pool.fee.as_u64() as f64 / 100.0);
            info!("         储备: {} / {}", pool.reserve0, pool.reserve1);
        }
        
        // 盈利分析
        let profit_avax = path.net_profit.as_u128() as f64 / 1e18;
        let gas_cost_avax = path.gas_estimate.as_u128() as f64 / 1e18 * 25e-9; // 假设25 gwei gas价格
        let gross_profit_avax = path.expected_profit.as_u128() as f64 / 1e18;
        
        info!("💰 盈利分析:");
        info!("   总预期利润: {:.6} AVAX ({} wei)", gross_profit_avax, path.expected_profit);
        info!("   估算Gas成本: {:.6} AVAX ({} wei)", gas_cost_avax, path.gas_estimate);
        info!("   净利润: {:.6} AVAX ({} wei)", profit_avax, path.net_profit);
        info!("   利润率: {:.2}%", (profit_avax / gross_profit_avax) * 100.0);
        
        // 创建结果（实际不执行）
        let result = ExecutionResult {
            success: true, // 打印总是成功
            transaction_hash: None, // 没有实际交易
            gas_used: Some(U256::zero()), // 没有实际消耗gas
            actual_profit: Some(U256::zero()), // 没有实际利润
            error_message: None,
            timestamp: chrono::Utc::now().timestamp() as u64,
            executor_name: self.name.clone(),
        };
        
        info!("📝 注意: 这是模拟模式，未执行实际交易");
        info!("==================================================");
        
        Ok(result)
    }
    
    async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256> {
        // 使用路径中的预估gas
        Ok(path.gas_estimate)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        true // 打印执行器总是可用
    }
}
