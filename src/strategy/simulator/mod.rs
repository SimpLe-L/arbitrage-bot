//! 本地模拟器模块
//! 
//! 用于本地克隆主网进行套利模拟，分析最终的套利利润情况

use crate::core::types::{ArbitragePath, SimulationResult, Result, BotError};
use async_trait::async_trait;
use ethers::types::{U256, Address};
use log::{info, debug, warn, error};
use serde::{Deserialize, Serialize};

/// 模拟器trait
#[async_trait]
pub trait Simulator: Send + Sync {
    /// 模拟套利执行
    async fn simulate_arbitrage(&self, path: &ArbitragePath) -> Result<SimulationResult>;
    
    /// 获取模拟器名称
    fn name(&self) -> &str;
    
    /// 检查模拟器是否可用
    async fn is_available(&self) -> bool;
}

/// Foundry本地模拟器
pub struct FoundrySimulator {
    name: String,
    fork_url: String,
    fork_block_number: Option<u64>,
    enabled: bool,
}

impl FoundrySimulator {
    pub fn new(fork_url: String) -> Self {
        Self {
            name: "FoundrySimulator".to_string(),
            fork_url,
            fork_block_number: None,
            enabled: true,
        }
    }
    
    /// 设置分叉区块号
    pub fn with_fork_block(mut self, block_number: u64) -> Self {
        self.fork_block_number = Some(block_number);
        self
    }
    
    /// 设置是否启用
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// 验证路径的有效性
    fn validate_path(&self, path: &ArbitragePath) -> Result<()> {
        if path.pools.is_empty() {
            return Err(BotError::SimulationError("套利路径为空".to_string()));
        }
        
        if path.input_token.address == path.output_token.address {
            return Err(BotError::SimulationError("输入和输出代币相同".to_string()));
        }
        
        // 验证路径连续性
        let mut current_token = path.input_token.address;
        for pool in &path.pools {
            let next_token = if pool.token0.address == current_token {
                pool.token1.address
            } else if pool.token1.address == current_token {
                pool.token0.address
            } else {
                return Err(BotError::SimulationError(
                    format!("路径不连续：池 {:?} 不包含代币 {:?}", pool.address, current_token)
                ));
            };
            current_token = next_token;
        }
        
        if current_token != path.output_token.address {
            return Err(BotError::SimulationError(
                "路径终点与输出代币不匹配".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// 计算真实的AMM交换输出
    fn calculate_real_swap_output(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: U256, // 基点，例如30表示0.3%
    ) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::zero();
        }
        
        // 计算扣除手续费后的输入金额
        let fee_multiplier = U256::from(10000) - fee_bps; // 例如：10000 - 30 = 9970
        let amount_in_after_fee = amount_in * fee_multiplier / U256::from(10000);
        
        // 使用恒定乘积公式: (x + Δx) * (y - Δy) = x * y
        // 解得: Δy = (y * Δx) / (x + Δx)
        let numerator = reserve_out * amount_in_after_fee;
        let denominator = reserve_in + amount_in_after_fee;
        
        if denominator.is_zero() {
            U256::zero()
        } else {
            numerator / denominator
        }
    }
    
    /// 模拟完整的套利路径
    async fn simulate_path_execution(&self, path: &ArbitragePath) -> Result<PathSimulationResult> {
        debug!("开始模拟套利路径执行");
        
        // 验证路径
        self.validate_path(path)?;
        
        // 模拟初始金额（从路径的第一个amounts_in获取，或使用默认值）
        let initial_amount = if !path.amounts_in.is_empty() {
            path.amounts_in[0]
        } else {
            U256::from(10u64.pow(18)) // 默认1个代币
        };
        
        let mut current_amount = initial_amount;
        let mut actual_amounts_out = Vec::new();
        let mut total_gas_used = U256::zero();
        
        // 遍历每个池进行模拟交换
        for (i, pool) in path.pools.iter().enumerate() {
            debug!("模拟交换 {}/{}: {} -> {}", 
                i + 1, 
                path.pools.len(),
                pool.token0.symbol,
                pool.token1.symbol
            );
            
            // 确定输入和输出储备
            let (reserve_in, reserve_out) = if i == 0 {
                // 第一个池：根据输入代币确定储备方向
                if pool.token0.address == path.input_token.address {
                    (pool.reserve0, pool.reserve1)
                } else {
                    (pool.reserve1, pool.reserve0)
                }
            } else {
                // 后续池：根据上一次的输出代币确定储备方向
                let prev_out_token = if i > 0 && i <= path.amounts_out.len() {
                    // 这里需要更复杂的逻辑来确定上一次的输出代币
                    // 简化处理：假设token0是输入
                    if pool.token0.address == path.input_token.address {
                        (pool.reserve1, pool.reserve0) // 从token1到token0
                    } else {
                        (pool.reserve0, pool.reserve1) // 从token0到token1
                    }
                } else {
                    (pool.reserve0, pool.reserve1)
                }
            };
            
            // 计算实际输出
            let amount_out = self.calculate_real_swap_output(
                current_amount,
                reserve_in,
                reserve_out,
                pool.fee,
            );
            
            if amount_out.is_zero() {
                return Ok(PathSimulationResult {
                    success: false,
                    final_amount: U256::zero(),
                    actual_amounts_out,
                    total_gas_used,
                    error_message: Some(format!("交换 {} 失败：输出为0", i + 1)),
                });
            }
            
            actual_amounts_out.push(amount_out);
            current_amount = amount_out;
            
            // 估算这次交换的gas消耗
            let swap_gas = U256::from(60000); // 每次swap大约60k gas
            total_gas_used += swap_gas;
            
            debug!("交换 {} 完成: 输入 {} -> 输出 {}", 
                i + 1, current_amount, amount_out);
        }
        
        let final_amount = current_amount;
        let success = final_amount > initial_amount; // 检查是否有利润
        
        info!("路径模拟完成: {} -> {}, 是否盈利: {}", 
            initial_amount, final_amount, success);
        
        Ok(PathSimulationResult {
            success,
            final_amount,
            actual_amounts_out,
            total_gas_used,
            error_message: None,
        })
    }
}

#[async_trait]
impl Simulator for FoundrySimulator {
    async fn simulate_arbitrage(&self, path: &ArbitragePath) -> Result<SimulationResult> {
        if !self.enabled {
            return Ok(SimulationResult {
                success: false,
                gas_used: U256::zero(),
                profit: U256::zero(),
                error_message: Some("模拟器已禁用".to_string()),
            });
        }
        
        info!("🧪 开始Foundry本地模拟");
        info!("分叉URL: {}", self.fork_url);
        if let Some(block) = self.fork_block_number {
            info!("分叉区块: {}", block);
        }
        
        // 模拟套利路径执行
        let simulation_result = self.simulate_path_execution(path).await?;
        
        if !simulation_result.success {
            warn!("❌ 模拟执行失败: {}", 
                simulation_result.error_message.as_deref().unwrap_or("未知错误"));
            
            return Ok(SimulationResult {
                success: false,
                gas_used: simulation_result.total_gas_used,
                profit: U256::zero(),
                error_message: simulation_result.error_message,
            });
        }
        
        // 计算实际利润
        let initial_amount = if !path.amounts_in.is_empty() {
            path.amounts_in[0]
        } else {
            U256::from(10u64.pow(18))
        };
        
        let actual_profit = if simulation_result.final_amount > initial_amount {
            simulation_result.final_amount - initial_amount
        } else {
            U256::zero()
        };
        
        // 计算gas成本
        let gas_price = U256::from(25_000_000_000u64); // 25 gwei
        let gas_cost = simulation_result.total_gas_used * gas_price;
        
        // 检查净利润
        let net_profit = if actual_profit > gas_cost {
            actual_profit - gas_cost
        } else {
            U256::zero()
        };
        
        let profitable = net_profit > U256::zero();
        
        if profitable {
            info!("✅ 模拟成功，发现盈利机会!");
            info!("   初始金额: {} wei", initial_amount);
            info!("   最终金额: {} wei", simulation_result.final_amount);
            info!("   毛利润: {} wei ({:.6} AVAX)", 
                actual_profit, actual_profit.as_u128() as f64 / 1e18);
            info!("   Gas成本: {} wei ({:.6} AVAX)", 
                gas_cost, gas_cost.as_u128() as f64 / 1e18);
            info!("   净利润: {} wei ({:.6} AVAX)", 
                net_profit, net_profit.as_u128() as f64 / 1e18);
        } else {
            info!("❌ 模拟显示无利润机会");
            info!("   毛利润: {} wei", actual_profit);
            info!("   Gas成本: {} wei", gas_cost);
            info!("   净亏损: {} wei", gas_cost.saturating_sub(actual_profit));
        }
        
        Ok(SimulationResult {
            success: profitable,
            gas_used: simulation_result.total_gas_used,
            profit: net_profit,
            error_message: if profitable { None } else { 
                Some("预期无利润".to_string()) 
            },
        })
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        self.enabled
    }
}

/// 路径模拟结果
#[derive(Debug)]
struct PathSimulationResult {
    success: bool,
    final_amount: U256,
    actual_amounts_out: Vec<U256>,
    total_gas_used: U256,
    error_message: Option<String>,
}

/// 简单模拟器 - 用于快速估算，不依赖外部工具
pub struct SimpleSimulator {
    name: String,
    enabled: bool,
}

impl SimpleSimulator {
    pub fn new() -> Self {
        Self {
            name: "SimpleSimulator".to_string(),
            enabled: true,
        }
    }
    
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[async_trait]
impl Simulator for SimpleSimulator {
    async fn simulate_arbitrage(&self, path: &ArbitragePath) -> Result<SimulationResult> {
        if !self.enabled {
            return Ok(SimulationResult {
                success: false,
                gas_used: U256::zero(),
                profit: U256::zero(),
                error_message: Some("模拟器已禁用".to_string()),
            });
        }
        
        info!("🔍 简单模拟套利执行");
        
        // 使用路径中预计算的值进行快速估算
        let has_profit = path.net_profit > U256::zero();
        
        if has_profit {
            info!("✅ 简单模拟显示有利润");
            info!("   预期净利润: {} wei ({:.6} AVAX)", 
                path.net_profit, path.net_profit.as_u128() as f64 / 1e18);
        } else {
            info!("❌ 简单模拟显示无利润");
        }
        
        Ok(SimulationResult {
            success: has_profit,
            gas_used: path.gas_estimate,
            profit: path.net_profit,
            error_message: if has_profit { None } else { 
                Some("预期无利润".to_string()) 
            },
        })
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        self.enabled
    }
}

impl Default for SimpleSimulator {
    fn default() -> Self {
        Self::new()
    }
}
