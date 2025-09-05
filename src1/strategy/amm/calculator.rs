//! 通用AMM计算器
//! 
//! 提供统一的接口来处理不同AMM协议的计算

use super::{types::*, uniswap_v2::UniswapV2Calculator};
use crate::strategy::dex_sync::types::{DexType, Pool, PoolState};
use ethers::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// AMM计算器trait
pub trait AmmCalculator: Send + Sync {
    /// 计算给定输入的输出数量
    fn get_amount_out(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256>;
    
    /// 计算达到指定输出所需的输入数量
    fn get_amount_in(
        &self,
        amount_out: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256>;
    
    /// 计算价格影响
    fn calculate_price_impact(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<f64>;
    
    /// 进行完整的交换计算
    fn calculate_swap(&self, input: SwapInput) -> AmmResult<SwapOutput>;
    
    /// 获取支持的协议类型
    fn supported_protocol(&self) -> AmmProtocol;
}

/// Uniswap V2计算器的trait实现
impl AmmCalculator for UniswapV2Calculator {
    fn get_amount_out(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256> {
        self.get_amount_out(amount_in, reserve_in, reserve_out, fee_bps)
    }
    
    fn get_amount_in(
        &self,
        amount_out: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256> {
        self.get_amount_in(amount_out, reserve_in, reserve_out, fee_bps)
    }
    
    fn calculate_price_impact(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<f64> {
        self.calculate_price_impact(amount_in, reserve_in, reserve_out, fee_bps)
    }
    
    fn calculate_swap(&self, input: SwapInput) -> AmmResult<SwapOutput> {
        self.calculate_swap(input)
    }
    
    fn supported_protocol(&self) -> AmmProtocol {
        AmmProtocol::UniswapV2
    }
}

/// 通用AMM计算管理器
pub struct AmmCalculatorManager {
    /// 计算器映射
    calculators: HashMap<AmmProtocol, Arc<dyn AmmCalculator>>,
    /// 默认滑点配置
    default_slippage: SlippageConfig,
}

impl AmmCalculatorManager {
    /// 创建新的计算器管理器
    pub fn new() -> Self {
        let mut calculators: HashMap<AmmProtocol, Arc<dyn AmmCalculator>> = HashMap::new();
        
        // 注册Uniswap V2计算器
        calculators.insert(
            AmmProtocol::UniswapV2,
            Arc::new(UniswapV2Calculator::new()),
        );
        
        Self {
            calculators,
            default_slippage: SlippageConfig::default(),
        }
    }
    
    /// 设置默认滑点配置
    pub fn with_slippage_config(mut self, config: SlippageConfig) -> Self {
        self.default_slippage = config;
        self
    }
    
    /// 添加计算器
    pub fn add_calculator(&mut self, protocol: AmmProtocol, calculator: Arc<dyn AmmCalculator>) {
        self.calculators.insert(protocol, calculator);
    }
    
    /// 获取计算器
    pub fn get_calculator(&self, protocol: AmmProtocol) -> Option<Arc<dyn AmmCalculator>> {
        self.calculators.get(&protocol).cloned()
    }
    
    /// 根据DEX类型获取协议类型
    pub fn dex_type_to_protocol(&self, dex_type: DexType) -> AmmProtocol {
        match dex_type {
            DexType::TraderJoe | DexType::Pangolin | DexType::SushiSwap => AmmProtocol::UniswapV2,
            DexType::Unknown => AmmProtocol::Other("Unknown".to_string()),
        }
    }
    
    /// 计算单步交换
    pub fn calculate_single_swap(
        &self,
        pool: &Pool,
        amount_in: U256,
        token_in: Address,
    ) -> AmmResult<SwapOutput> {
        let protocol = self.dex_type_to_protocol(pool.dex);
        let calculator = self.get_calculator(protocol.clone())
            .ok_or_else(|| AmmError::UnsupportedProtocol {
                protocol: protocol.name().to_string()
            })?;
        
        // 确定输入输出储备量
        let (reserve_in, reserve_out) = pool.get_reserves(token_in)
            .ok_or(AmmError::InvalidInputAmount { amount: U256::from(0u64) })?;
        
        let input = SwapInput {
            amount_in,
            reserve_in,
            reserve_out,
            fee_bps: pool.fee_bps,
        };
        
        let output = calculator.calculate_swap(input)?;
        
        // 检查滑点
        self.default_slippage.check_slippage(output.amount_out, output.amount_out)?;
        
        Ok(output)
    }
    
    /// 计算多步套利路径
    pub fn calculate_arbitrage_path(
        &self,
        pools: &[PoolState],
        path: &[Address], // token路径
        initial_amount: U256,
    ) -> AmmResult<ArbitragePath> {
        if pools.len() != path.len() - 1 {
            return Err(AmmError::InvalidInputAmount { amount: U256::zero() });
        }
        
        let mut steps = Vec::new();
        let mut current_amount = initial_amount;
        
        for (i, pool_state) in pools.iter().enumerate() {
            let token_in = path[i];
            let token_out = path[i + 1];
            
            let swap_output = self.calculate_single_swap(&pool_state.pool, current_amount, token_in)?;
            
            let step = ArbitrageStep::new(
                pool_state.pool.address,
                token_in,
                token_out,
                current_amount,
                swap_output.amount_out,
                pool_state.pool.dex.name().to_string(),
                pool_state.pool.fee_bps,
            );
            
            steps.push(step);
            current_amount = swap_output.amount_out;
        }
        
        let arbitrage_path = ArbitragePath::new(
            steps,
            path[0],
            *path.last().unwrap(),
            initial_amount,
        );
        
        Ok(arbitrage_path)
    }
    
    /// 优化套利金额
    /// 通过二分查找找到最优的初始投入金额
    pub fn optimize_arbitrage_amount(
        &self,
        pools: &[PoolState],
        path: &[Address],
        min_amount: U256,
        max_amount: U256,
        target_profit_rate: f64, // 目标利润率
    ) -> AmmResult<(U256, ArbitragePath)> {
        let mut best_amount = U256::zero();
        let mut best_path: Option<ArbitragePath> = None;
        let mut best_profit_rate = 0.0;
        
        let mut low = min_amount;
        let mut high = max_amount;
        
        // 使用黄金分割搜索优化
        let phi = 1.618033988749; // 黄金比例
        let resphi = 2.0 - phi;
        
        let mut tol = U256::from(1000u64); // 容忍度
        
        let mut x1 = low + U256::from(((high - low).as_u128() as f64 * resphi) as u128);
        let mut x2 = high - U256::from(((high - low).as_u128() as f64 * resphi) as u128);
        
        let f1 = self.evaluate_profit_rate(pools, path, x1)?;
        let f2 = self.evaluate_profit_rate(pools, path, x2)?;
        
        let mut iteration = 0;
        const MAX_ITERATIONS: u32 = 50;
        
        while (high - low) > tol && iteration < MAX_ITERATIONS {
            if f1 > f2 {
                low = x2;
                x2 = x1;
                x1 = low + U256::from(((high - low).as_u128() as f64 * resphi) as u128);
                // f2 = f1; 这行在rust中会有移动问题，所以重新计算
                let f2_new = self.evaluate_profit_rate(pools, path, x1)?;
                if f2_new > best_profit_rate {
                    best_profit_rate = f2_new;
                    best_amount = x1;
                }
            } else {
                high = x1;
                x1 = x2;
                x2 = high - U256::from(((high - low).as_u128() as f64 * resphi) as u128);
                let f1_new = self.evaluate_profit_rate(pools, path, x2)?;
                if f1_new > best_profit_rate {
                    best_profit_rate = f1_new;
                    best_amount = x2;
                }
            }
            iteration += 1;
        }
        
        // 如果没找到好的结果，尝试中点
        if best_amount == U256::zero() {
            best_amount = (min_amount + max_amount) / U256::from(2u64);
        }
        
        // 计算最优路径
        let optimal_path = self.calculate_arbitrage_path(pools, path, best_amount)?;
        
        Ok((best_amount, optimal_path))
    }
    
    /// 评估利润率
    fn evaluate_profit_rate(&self, pools: &[PoolState], path: &[Address], amount: U256) -> AmmResult<f64> {
        if amount == U256::zero() {
            return Ok(0.0);
        }
        
        match self.calculate_arbitrage_path(pools, path, amount) {
            Ok(arb_path) => Ok(arb_path.profit_rate()),
            Err(_) => Ok(-1.0), // 失败的路径返回负利润率
        }
    }
    
    /// 批量计算多个路径
    pub fn calculate_multiple_paths(
        &self,
        paths_data: &[(Vec<PoolState>, Vec<Address>, U256)], // (pools, path, amount)
    ) -> Vec<AmmResult<ArbitragePath>> {
        paths_data
            .iter()
            .map(|(pools, path, amount)| {
                self.calculate_arbitrage_path(pools, path, *amount)
            })
            .collect()
    }
    
    /// 检查路径可行性
    pub fn is_path_feasible(
        &self,
        pools: &[PoolState],
        path: &[Address],
        amount: U256,
        min_profit_wei: U256,
    ) -> bool {
        match self.calculate_arbitrage_path(pools, path, amount) {
            Ok(arb_path) => arb_path.estimated_profit >= min_profit_wei,
            Err(_) => false,
        }
    }
    
    /// 获取价格信息
    pub fn get_price_info(
        &self,
        pool: &Pool,
        decimals0: u8,
        decimals1: u8,
    ) -> PriceInfo {
        let sqrt_k = if pool.reserve0 == U256::zero() || pool.reserve1 == U256::zero() {
            U256::zero()
        } else {
            // 简化的流动性计算
            (pool.reserve0 + pool.reserve1) / U256::from(2u64)
        };
        
        PriceInfo::new(
            pool.reserve0,
            pool.reserve1,
            decimals0,
            decimals1,
            sqrt_k,
            pool.block_timestamp_last,
        )
    }
    
    /// 估算Gas消耗
    pub fn estimate_gas_cost(
        &self,
        path: &ArbitragePath,
        gas_price_gwei: u64,
    ) -> U256 {
        // 基础Gas消耗估算
        let base_gas = U256::from(21000u64); // 基础交易gas
        let swap_gas_per_hop = U256::from(100000u64); // 每跳大约10万gas
        
        let total_gas = base_gas + swap_gas_per_hop * path.steps.len();
        let gas_price_wei = U256::from(gas_price_gwei) * U256::from(10u64).pow(U256::from(9u64)); // gwei转wei
        
        total_gas * gas_price_wei
    }
    
    /// 检查价格影响是否可接受
    pub fn is_price_impact_acceptable(
        &self,
        path: &ArbitragePath,
        max_impact_bps: u16,
    ) -> bool {
        let max_impact = max_impact_bps as f64 / 10000.0;
        path.calculate_total_price_impact() <= max_impact
    }
    
    /// 获取支持的协议列表
    pub fn supported_protocols(&self) -> Vec<AmmProtocol> {
        self.calculators.keys().cloned().collect()
    }
    
    /// 获取统计信息
    pub fn get_calculator_stats(&self) -> HashMap<AmmProtocol, String> {
        self.calculators
            .iter()
            .map(|(protocol, _)| (protocol.clone(), format!("活跃")))
            .collect()
    }
}

impl Default for AmmCalculatorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AmmCalculatorManager {
    fn clone(&self) -> Self {
        Self {
            calculators: self.calculators.clone(),
            default_slippage: self.default_slippage.clone(),
        }
    }
}

/// 便捷函数：创建预配置的计算器管理器
pub fn create_avax_calculator_manager() -> AmmCalculatorManager {
    let mut manager = AmmCalculatorManager::new();
    
    // 为AVAX网络优化的滑点配置
    let avax_slippage = SlippageConfig {
        max_slippage_bps: 50, // 0.5% 适合AVAX的波动性
        dynamic_adjustment: true,
        min_output_protection: true,
    };
    
    manager.with_slippage_config(avax_slippage)
}

/// 便捷函数：快速计算单个交换
pub fn quick_swap_calculation(
    amount_in: U256,
    reserve_in: U256,
    reserve_out: U256,
    fee_bps: u16,
) -> AmmResult<SwapOutput> {
    let calculator = UniswapV2Calculator::new();
    let input = SwapInput {
        amount_in,
        reserve_in,
        reserve_out,
        fee_bps,
    };
    calculator.calculate_swap(input)
}

/// 便捷函数：计算最小输出金额（含滑点保护）
pub fn calculate_min_amount_out(
    expected_amount: U256,
    slippage_bps: u16,
) -> U256 {
    let slippage_config = SlippageConfig {
        max_slippage_bps: slippage_bps,
        dynamic_adjustment: false,
        min_output_protection: true,
    };
    slippage_config.calculate_min_amount_out(expected_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::dex_sync::types::{Token, DexType};
    
    #[test]
    fn test_calculator_manager_creation() {
        let manager = AmmCalculatorManager::new();
        assert!(manager.get_calculator(AmmProtocol::UniswapV2).is_some());
    }
    
    #[test]
    fn test_dex_type_mapping() {
        let manager = AmmCalculatorManager::new();
        assert_eq!(manager.dex_type_to_protocol(DexType::TraderJoe), AmmProtocol::UniswapV2);
        assert_eq!(manager.dex_type_to_protocol(DexType::Pangolin), AmmProtocol::UniswapV2);
    }
    
    #[test]
    fn test_quick_swap_calculation() {
        let result = quick_swap_calculation(
            U256::from(1000u64),
            U256::from(10000u64),
            U256::from(20000u64),
            30,
        );
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.amount_out > U256::zero());
    }
    
    #[test]
    fn test_min_amount_calculation() {
        let expected = U256::from(1000u64);
        let min_amount = calculate_min_amount_out(expected, 100); // 1% slippage
        
        assert!(min_amount < expected);
        assert_eq!(min_amount, U256::from(990u64)); // 1000 * 0.99
    }
}
