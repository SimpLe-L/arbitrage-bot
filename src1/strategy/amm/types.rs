//! AMM计算相关类型定义

use ethers::prelude::*;
use serde::{Deserialize, Serialize};

/// 交换参数
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapParams {
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub recipient: Address,
    pub deadline: U256,
}

/// 交换输入
#[derive(Debug, Clone)]
pub struct SwapInput {
    pub amount_in: U256,
    pub reserve_in: U256,
    pub reserve_out: U256,
    pub fee_bps: u16, // 手续费基点(如30 = 0.3%)
}

/// 交换输出
#[derive(Debug, Clone)]
pub struct SwapOutput {
    pub amount_out: U256,
    pub price_impact: f64,     // 价格影响百分比
    pub effective_price: f64,  // 有效价格
    pub fee_amount: U256,      // 手续费金额
}

impl SwapOutput {
    /// 创建新的交换输出
    pub fn new(amount_out: U256, price_impact: f64, effective_price: f64, fee_amount: U256) -> Self {
        Self {
            amount_out,
            price_impact,
            effective_price,
            fee_amount,
        }
    }
    
    /// 检查价格影响是否可接受
    pub fn is_price_impact_acceptable(&self, max_impact_bps: u16) -> bool {
        let max_impact = max_impact_bps as f64 / 10000.0; // 转换为百分比
        self.price_impact <= max_impact
    }
}

/// 流动性添加参数
#[derive(Debug, Clone)]
pub struct AddLiquidityParams {
    pub token_a: Address,
    pub token_b: Address,
    pub amount_a_desired: U256,
    pub amount_b_desired: U256,
    pub amount_a_min: U256,
    pub amount_b_min: U256,
    pub recipient: Address,
    pub deadline: U256,
}

/// 流动性移除参数
#[derive(Debug, Clone)]
pub struct RemoveLiquidityParams {
    pub token_a: Address,
    pub token_b: Address,
    pub liquidity: U256,
    pub amount_a_min: U256,
    pub amount_b_min: U256,
    pub recipient: Address,
    pub deadline: U256,
}

/// 价格查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: f64,           // 价格
    pub inverse_price: f64,   // 反向价格
    pub reserve0: U256,       // 储备量0
    pub reserve1: U256,       // 储备量1
    pub liquidity: U256,      // 总流动性
    pub last_updated: u64,    // 最后更新时间戳
}

impl PriceInfo {
    pub fn new(
        reserve0: U256,
        reserve1: U256,
        decimals0: u8,
        decimals1: u8,
        liquidity: U256,
        last_updated: u64,
    ) -> Self {
        let price = if reserve0 == U256::zero() {
            0.0
        } else {
            let r0_f64 = reserve0.as_u128() as f64 / 10_f64.powi(decimals0 as i32);
            let r1_f64 = reserve1.as_u128() as f64 / 10_f64.powi(decimals1 as i32);
            r1_f64 / r0_f64
        };
        
        let inverse_price = if price == 0.0 { 0.0 } else { 1.0 / price };
        
        Self {
            price,
            inverse_price,
            reserve0,
            reserve1,
            liquidity,
            last_updated,
        }
    }
}

/// 套利路径步骤
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitrageStep {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub dex_name: String,
    pub fee_bps: u16,
}

impl ArbitrageStep {
    pub fn new(
        pool_address: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        amount_out: U256,
        dex_name: String,
        fee_bps: u16,
    ) -> Self {
        Self {
            pool_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            dex_name,
            fee_bps,
        }
    }
    
    /// 计算此步骤的价格影响
    pub fn calculate_price_impact(&self, reserve_in: U256, reserve_out: U256) -> f64 {
        if reserve_in == U256::zero() || reserve_out == U256::zero() {
            return 100.0; // 100%价格影响表示无流动性
        }
        
        // 计算理论价格（无滑点）
        let theoretical_price = reserve_out.as_u128() as f64 / reserve_in.as_u128() as f64;
        
        // 计算实际价格
        let actual_price = if self.amount_in == U256::zero() {
            0.0
        } else {
            self.amount_out.as_u128() as f64 / self.amount_in.as_u128() as f64
        };
        
        // 计算价格影响
        if theoretical_price == 0.0 {
            100.0
        } else {
            ((theoretical_price - actual_price) / theoretical_price * 100.0).abs()
        }
    }
}

/// 套利路径
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitragePath {
    pub steps: Vec<ArbitrageStep>,
    pub start_token: Address,
    pub end_token: Address,
    pub initial_amount: U256,
    pub final_amount: U256,
    pub estimated_profit: U256,
    pub total_gas_cost: U256,
    pub net_profit: U256,
}

impl ArbitragePath {
    pub fn new(
        steps: Vec<ArbitrageStep>,
        start_token: Address,
        end_token: Address,
        initial_amount: U256,
    ) -> Self {
        let final_amount = steps.last().map(|s| s.amount_out).unwrap_or(U256::zero());
        
        let estimated_profit = if final_amount > initial_amount {
            final_amount - initial_amount
        } else {
            U256::zero()
        };
        
        Self {
            steps,
            start_token,
            end_token,
            initial_amount,
            final_amount,
            estimated_profit,
            total_gas_cost: U256::zero(), // 需要单独计算
            net_profit: estimated_profit, // 扣除gas后的净利润
        }
    }
    
    /// 计算总价格影响
    pub fn calculate_total_price_impact(&self) -> f64 {
        // 简化实现：取最大价格影响
        // 实际应该是复合计算
        self.steps.iter()
            .map(|step| {
                // 这里需要从池管理器获取储备量信息
                // 暂时返回0.0
                0.0
            })
            .fold(0.0, f64::max)
    }
    
    /// 检查路径是否有效
    pub fn is_valid(&self) -> bool {
        if self.steps.is_empty() {
            return false;
        }
        
        // 检查路径连续性
        for i in 0..self.steps.len() - 1 {
            if self.steps[i].token_out != self.steps[i + 1].token_in {
                return false;
            }
        }
        
        // 检查起始和结束代币
        if let (Some(first), Some(last)) = (self.steps.first(), self.steps.last()) {
            first.token_in == self.start_token && last.token_out == self.end_token
        } else {
            false
        }
    }
    
    /// 获取路径长度(跳数)
    pub fn hops(&self) -> usize {
        self.steps.len()
    }
    
    /// 计算利润率
    pub fn profit_rate(&self) -> f64 {
        if self.initial_amount == U256::zero() {
            return 0.0;
        }
        
        let initial_f64 = self.initial_amount.as_u128() as f64;
        let profit_f64 = self.estimated_profit.as_u128() as f64;
        
        profit_f64 / initial_f64
    }
    
    /// 获取涉及的DEX列表
    pub fn get_dexes(&self) -> Vec<String> {
        self.steps.iter()
            .map(|step| step.dex_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }
    
    /// 设置gas成本并更新净利润
    pub fn with_gas_cost(mut self, gas_cost: U256) -> Self {
        self.total_gas_cost = gas_cost;
        self.net_profit = if self.estimated_profit > gas_cost {
            self.estimated_profit - gas_cost
        } else {
            U256::zero()
        };
        self
    }
    
    /// 检查套利是否有利可图
    pub fn is_profitable(&self) -> bool {
        self.net_profit > U256::zero()
    }
}

/// AMM计算错误类型
#[derive(Debug, thiserror::Error)]
pub enum AmmError {
    #[error("计算溢出")]
    Overflow,
    
    #[error("除零错误")]
    DivisionByZero,
    
    #[error("储备量不足: 需要 {required}, 可用 {available}")]
    InsufficientReserves { required: U256, available: U256 },
    
    #[error("流动性不足")]
    InsufficientLiquidity,
    
    #[error("输入金额无效: {amount}")]
    InvalidInputAmount { amount: U256 },
    
    #[error("输出金额无效: {amount}")]
    InvalidOutputAmount { amount: U256 },
    
    #[error("滑点过大: 期望 {expected}, 实际 {actual}")]
    ExcessiveSlippage { expected: U256, actual: U256 },
    
    #[error("价格影响过大: {impact}%")]
    ExcessivePriceImpact { impact: f64 },
    
    #[error("不支持的AMM协议: {protocol}")]
    UnsupportedProtocol { protocol: String },
}

pub type AmmResult<T> = Result<T, AmmError>;

/// AMM协议类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmProtocol {
    /// Uniswap V2及其分叉(Trader Joe, Pangolin, SushiSwap等)
    UniswapV2,
    /// Uniswap V3
    UniswapV3,
    /// Curve Finance
    Curve,
    /// Balancer
    Balancer,
    /// 其他协议
    Other(String),
}

impl AmmProtocol {
    pub fn name(&self) -> &str {
        match self {
            Self::UniswapV2 => "Uniswap V2",
            Self::UniswapV3 => "Uniswap V3",
            Self::Curve => "Curve",
            Self::Balancer => "Balancer",
            Self::Other(name) => name,
        }
    }
}

/// 滑点保护配置
#[derive(Debug, Clone)]
pub struct SlippageConfig {
    /// 最大滑点基点(如100 = 1%)
    pub max_slippage_bps: u16,
    /// 动态调整滑点
    pub dynamic_adjustment: bool,
    /// 最小输出金额保护
    pub min_output_protection: bool,
}

impl Default for SlippageConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: 100, // 1%
            dynamic_adjustment: true,
            min_output_protection: true,
        }
    }
}

impl SlippageConfig {
    /// 计算最小输出金额
    pub fn calculate_min_amount_out(&self, expected_amount: U256) -> U256 {
        let slippage_factor = U256::from(10000u32 - self.max_slippage_bps as u32);
        expected_amount * slippage_factor / U256::from(10000u32)
    }
    
    /// 检查输出是否满足滑点要求
    pub fn check_slippage(&self, expected: U256, actual: U256) -> AmmResult<()> {
        if !self.min_output_protection {
            return Ok(());
        }
        
        let min_amount = self.calculate_min_amount_out(expected);
        if actual < min_amount {
            return Err(AmmError::ExcessiveSlippage { expected, actual });
        }
        
        Ok(())
    }
}
