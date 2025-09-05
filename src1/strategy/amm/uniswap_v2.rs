//! Uniswap V2类型AMM计算
//! 
//! 实现精确的恒定乘积公式计算，支持Trader Joe、Pangolin、SushiSwap等

use super::types::*;
use ethers::prelude::*;
use std::cmp;

/// Uniswap V2计算器
pub struct UniswapV2Calculator {
    /// 最小流动性阈值
    min_liquidity: U256,
}

impl UniswapV2Calculator {
    /// 创建新的计算器
    pub fn new() -> Self {
        Self {
            min_liquidity: U256::from(1000u64), // 最小流动性1000 wei
        }
    }
    
    /// 设置最小流动性阈值
    pub fn with_min_liquidity(mut self, min_liquidity: U256) -> Self {
        self.min_liquidity = min_liquidity;
        self
    }
    
    /// 计算给定输入数量的输出数量
    /// 使用公式: amount_out = (amount_in * fee_factor * reserve_out) / (reserve_in * 10000 + amount_in * fee_factor)
    /// 其中 fee_factor = 10000 - fee_bps
    pub fn get_amount_out(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256> {
        // 输入验证
        if amount_in == U256::zero() {
            return Err(AmmError::InvalidInputAmount { amount: amount_in });
        }
        
        if reserve_in < self.min_liquidity || reserve_out < self.min_liquidity {
            return Err(AmmError::InsufficientLiquidity);
        }
        
        if fee_bps >= 10000 {
            return Err(AmmError::InvalidInputAmount { amount: U256::from(fee_bps) });
        }
        
        // 计算手续费因子
        let fee_factor = U256::from(10000u32 - fee_bps as u32);
        
        // 防止溢出的计算
        let amount_in_with_fee = amount_in.checked_mul(fee_factor)
            .ok_or(AmmError::Overflow)?;
        
        let numerator = amount_in_with_fee.checked_mul(reserve_out)
            .ok_or(AmmError::Overflow)?;
        
        let denominator = reserve_in.checked_mul(U256::from(10000u32))
            .ok_or(AmmError::Overflow)?
            .checked_add(amount_in_with_fee)
            .ok_or(AmmError::Overflow)?;
        
        if denominator == U256::zero() {
            return Err(AmmError::DivisionByZero);
        }
        
        let amount_out = numerator / denominator;
        
        // 检查输出是否有效
        if amount_out == U256::zero() {
            return Err(AmmError::InvalidOutputAmount { amount: amount_out });
        }
        
        if amount_out >= reserve_out {
            return Err(AmmError::InsufficientReserves { 
                required: amount_out, 
                available: reserve_out 
            });
        }
        
        Ok(amount_out)
    }
    
    /// 计算达到指定输出数量所需的输入数量
    /// 使用公式: amount_in = (reserve_in * amount_out * 10000) / ((reserve_out - amount_out) * fee_factor) + 1
    pub fn get_amount_in(
        &self,
        amount_out: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<U256> {
        // 输入验证
        if amount_out == U256::zero() {
            return Err(AmmError::InvalidOutputAmount { amount: amount_out });
        }
        
        if reserve_in < self.min_liquidity || reserve_out < self.min_liquidity {
            return Err(AmmError::InsufficientLiquidity);
        }
        
        if amount_out >= reserve_out {
            return Err(AmmError::InsufficientReserves {
                required: amount_out,
                available: reserve_out,
            });
        }
        
        if fee_bps >= 10000 {
            return Err(AmmError::InvalidInputAmount { amount: U256::from(fee_bps) });
        }
        
        let fee_factor = U256::from(10000u32 - fee_bps as u32);
        
        // 计算分子: reserve_in * amount_out * 10000
        let numerator = reserve_in
            .checked_mul(amount_out)
            .ok_or(AmmError::Overflow)?
            .checked_mul(U256::from(10000u32))
            .ok_or(AmmError::Overflow)?;
        
        // 计算分母: (reserve_out - amount_out) * fee_factor
        let denominator = reserve_out
            .checked_sub(amount_out)
            .ok_or(AmmError::Overflow)?
            .checked_mul(fee_factor)
            .ok_or(AmmError::Overflow)?;
        
        if denominator == U256::zero() {
            return Err(AmmError::DivisionByZero);
        }
        
        // 为了确保精度，向上舍入（加1）
        let amount_in = numerator / denominator + U256::one();
        
        Ok(amount_in)
    }
    
    /// 计算价格影响
    pub fn calculate_price_impact(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u16,
    ) -> AmmResult<f64> {
        if reserve_in == U256::zero() || reserve_out == U256::zero() {
            return Ok(100.0); // 100%价格影响
        }
        
        // 计算理论价格（当前汇率）
        let current_rate = reserve_out.as_u128() as f64 / reserve_in.as_u128() as f64;
        
        // 计算交换后的实际价格
        let amount_out = self.get_amount_out(amount_in, reserve_in, reserve_out, fee_bps)?;
        
        if amount_in == U256::zero() {
            return Ok(0.0);
        }
        
        let effective_rate = amount_out.as_u128() as f64 / amount_in.as_u128() as f64;
        
        // 计算价格影响百分比
        let price_impact = if current_rate == 0.0 {
            100.0
        } else {
            ((current_rate - effective_rate) / current_rate * 100.0).abs()
        };
        
        Ok(price_impact)
    }
    
    /// 进行完整的交换计算
    pub fn calculate_swap(&self, input: SwapInput) -> AmmResult<SwapOutput> {
        let amount_out = self.get_amount_out(
            input.amount_in,
            input.reserve_in,
            input.reserve_out,
            input.fee_bps,
        )?;
        
        let price_impact = self.calculate_price_impact(
            input.amount_in,
            input.reserve_in,
            input.reserve_out,
            input.fee_bps,
        )?;
        
        // 计算有效价格
        let effective_price = if input.amount_in == U256::zero() {
            0.0
        } else {
            amount_out.as_u128() as f64 / input.amount_in.as_u128() as f64
        };
        
        // 计算手续费金额
        let fee_amount = input.amount_in * U256::from(input.fee_bps) / U256::from(10000u32);
        
        Ok(SwapOutput::new(amount_out, price_impact, effective_price, fee_amount))
    }
    
    /// 获取最优交换路径（多跳）
    pub fn get_optimal_swap_path(
        &self,
        amount_in: U256,
        path_reserves: &[(U256, U256)], // (reserve_in, reserve_out) 对于每一跳
        fee_bps: u16,
    ) -> AmmResult<Vec<U256>> {
        if path_reserves.is_empty() {
            return Ok(vec![amount_in]);
        }
        
        let mut amounts = vec![amount_in];
        let mut current_amount = amount_in;
        
        for &(reserve_in, reserve_out) in path_reserves {
            current_amount = self.get_amount_out(current_amount, reserve_in, reserve_out, fee_bps)?;
            amounts.push(current_amount);
        }
        
        Ok(amounts)
    }
    
    /// 计算最优输入金额（给定期望的最终输出）
    pub fn get_optimal_input_amount(
        &self,
        amount_out: U256,
        path_reserves: &[(U256, U256)], // 需要反向处理
        fee_bps: u16,
    ) -> AmmResult<Vec<U256>> {
        if path_reserves.is_empty() {
            return Ok(vec![amount_out]);
        }
        
        let mut amounts = vec![amount_out];
        let mut current_amount = amount_out;
        
        // 反向计算
        for &(reserve_out, reserve_in) in path_reserves.iter().rev() {
            current_amount = self.get_amount_in(current_amount, reserve_in, reserve_out, fee_bps)?;
            amounts.insert(0, current_amount);
        }
        
        Ok(amounts)
    }
    
    /// 计算流动性数量
    pub fn calculate_liquidity(
        &self,
        amount_a: U256,
        amount_b: U256,
        reserve_a: U256,
        reserve_b: U256,
        total_supply: U256,
    ) -> AmmResult<U256> {
        if total_supply == U256::zero() {
            // 首次添加流动性
            let liquidity = self.sqrt(amount_a.checked_mul(amount_b).ok_or(AmmError::Overflow)?);
            
            // 减去最小流动性锁定量（通常是1000）
            if liquidity <= U256::from(1000u64) {
                return Err(AmmError::InsufficientLiquidity);
            }
            
            Ok(liquidity - U256::from(1000u64))
        } else {
            // 后续添加流动性，取两个比例的最小值
            let liquidity_a = amount_a.checked_mul(total_supply)
                .ok_or(AmmError::Overflow)?
                / reserve_a;
            
            let liquidity_b = amount_b.checked_mul(total_supply)
                .ok_or(AmmError::Overflow)?
                / reserve_b;
            
            Ok(cmp::min(liquidity_a, liquidity_b))
        }
    }
    
    /// 计算可提取的代币数量
    pub fn calculate_removal_amounts(
        &self,
        liquidity: U256,
        reserve_a: U256,
        reserve_b: U256,
        total_supply: U256,
    ) -> AmmResult<(U256, U256)> {
        if total_supply == U256::zero() {
            return Err(AmmError::DivisionByZero);
        }
        
        let amount_a = liquidity.checked_mul(reserve_a)
            .ok_or(AmmError::Overflow)?
            / total_supply;
        
        let amount_b = liquidity.checked_mul(reserve_b)
            .ok_or(AmmError::Overflow)?
            / total_supply;
        
        Ok((amount_a, amount_b))
    }
    
    /// 验证K值恒定（用于验证计算正确性）
    pub fn verify_k_constant(
        &self,
        reserve_in_before: U256,
        reserve_out_before: U256,
        reserve_in_after: U256,
        reserve_out_after: U256,
        tolerance_bps: u16, // 允许的误差基点
    ) -> bool {
        let k_before = reserve_in_before.checked_mul(reserve_out_before);
        let k_after = reserve_in_after.checked_mul(reserve_out_after);
        
        match (k_before, k_after) {
            (Some(k1), Some(k2)) => {
                if k1 == U256::zero() && k2 == U256::zero() {
                    return true;
                }
                
                if k1 == U256::zero() || k2 == U256::zero() {
                    return false;
                }
                
                // K值应该保持不变或略有增加（由于手续费）
                // 允许少量误差
                let tolerance = k1 * U256::from(tolerance_bps) / U256::from(10000u32);
                k2 >= k1 && k2 <= k1 + tolerance
            }
            _ => false,
        }
    }
    
    /// 计算平方根（用于流动性计算）
    fn sqrt(&self, y: U256) -> U256 {
        if y == U256::zero() {
            return U256::zero();
        }
        
        let mut z = y;
        let mut x = (y + U256::one()) / U256::from(2u64);
        
        while x < z {
            z = x;
            x = (y / x + x) / U256::from(2u64);
        }
        
        z
    }
    
    /// 检查交换是否可行
    pub fn is_swap_feasible(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
        min_amount_out: U256,
        fee_bps: u16,
    ) -> bool {
        match self.get_amount_out(amount_in, reserve_in, reserve_out, fee_bps) {
            Ok(amount_out) => amount_out >= min_amount_out,
            Err(_) => false,
        }
    }
    
    /// 计算最大可交换数量（不超过储备量的一定比例）
    pub fn max_swappable_amount(
        &self,
        reserve_in: U256,
        reserve_out: U256,
        max_impact_bps: u16, // 最大价格影响基点
        fee_bps: u16,
    ) -> AmmResult<U256> {
        if reserve_in == U256::zero() || reserve_out == U256::zero() {
            return Err(AmmError::InsufficientLiquidity);
        }
        
        let max_impact = max_impact_bps as f64 / 10000.0;
        
        // 使用二分查找找到最大可交换数量
        let mut low = U256::one();
        let mut high = reserve_in / U256::from(2u64); // 最多不超过储备量的一半
        
        let mut result = U256::zero();
        
        while low <= high {
            let mid = (low + high) / U256::from(2u64);
            
            match self.calculate_price_impact(mid, reserve_in, reserve_out, fee_bps) {
                Ok(impact) => {
                    if impact <= max_impact {
                        result = mid;
                        low = mid + U256::one();
                    } else {
                        if mid == U256::zero() {
                            break;
                        }
                        high = mid - U256::one();
                    }
                }
                Err(_) => {
                    if mid == U256::zero() {
                        break;
                    }
                    high = mid - U256::one();
                }
            }
        }
        
        Ok(result)
    }
}

impl Default for UniswapV2Calculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_amount_out_basic() {
        let calculator = UniswapV2Calculator::new();
        
        // 测试基本交换计算
        let amount_in = U256::from(1000u64);
        let reserve_in = U256::from(10000u64);
        let reserve_out = U256::from(20000u64);
        let fee_bps = 30; // 0.3%
        
        let result = calculator.get_amount_out(amount_in, reserve_in, reserve_out, fee_bps);
        assert!(result.is_ok());
        
        let amount_out = result.unwrap();
        assert!(amount_out > U256::zero());
        assert!(amount_out < reserve_out);
    }
    
    #[test]
    fn test_get_amount_in_basic() {
        let calculator = UniswapV2Calculator::new();
        
        let amount_out = U256::from(1000u64);
        let reserve_in = U256::from(10000u64);
        let reserve_out = U256::from(20000u64);
        let fee_bps = 30;
        
        let result = calculator.get_amount_in(amount_out, reserve_in, reserve_out, fee_bps);
        assert!(result.is_ok());
        
        let amount_in = result.unwrap();
        assert!(amount_in > U256::zero());
    }
    
    #[test]
    fn test_price_impact() {
        let calculator = UniswapV2Calculator::new();
        
        let amount_in = U256::from(1000u64);
        let reserve_in = U256::from(100000u64);
        let reserve_out = U256::from(100000u64);
        let fee_bps = 30;
        
        let result = calculator.calculate_price_impact(amount_in, reserve_in, reserve_out, fee_bps);
        assert!(result.is_ok());
        
        let impact = result.unwrap();
        assert!(impact >= 0.0 && impact <= 100.0);
    }
    
    #[test]
    fn test_insufficient_liquidity() {
        let calculator = UniswapV2Calculator::new();
        
        let amount_in = U256::from(1000u64);
        let reserve_in = U256::from(100u64); // 很小的储备量
        let reserve_out = U256::from(100u64);
        let fee_bps = 30;
        
        let result = calculator.get_amount_out(amount_in, reserve_in, reserve_out, fee_bps);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AmmError::InsufficientLiquidity => {},
            _ => panic!("Expected InsufficientLiquidity error"),
        }
    }
    
    #[test]
    fn test_k_constant_verification() {
        let calculator = UniswapV2Calculator::new();
        
        let reserve_in_before = U256::from(10000u64);
        let reserve_out_before = U256::from(20000u64);
        
        // 模拟交换后的储备量变化
        let amount_in = U256::from(1000u64);
        let amount_out = calculator.get_amount_out(amount_in, reserve_in_before, reserve_out_before, 30).unwrap();
        
        let reserve_in_after = reserve_in_before + amount_in;
        let reserve_out_after = reserve_out_before - amount_out;
        
        let is_valid = calculator.verify_k_constant(
            reserve_in_before,
            reserve_out_before,
            reserve_in_after,
            reserve_out_after,
            100, // 1% tolerance
        );
        
        assert!(is_valid);
    }
}
