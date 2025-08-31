//! 数值计算工具

use ethers::types::U256;

/// 计算滑点后的最小接收量
/// 
/// # 参数
/// * `amount` - 原始数量
/// * `slippage_bps` - 滑点（基点，例如 100 = 1%）
pub fn calculate_min_amount_out(amount: U256, slippage_bps: u64) -> U256 {
    if slippage_bps >= 10000 {
        return U256::zero();
    }
    
    let multiplier = U256::from(10000 - slippage_bps);
    amount.saturating_mul(multiplier) / U256::from(10000)
}

/// 计算百分比
pub fn calculate_percentage(part: U256, total: U256) -> f64 {
    if total.is_zero() {
        return 0.0;
    }
    
    let part_f64 = part.as_u64() as f64;
    let total_f64 = total.as_u64() as f64;
    
    (part_f64 / total_f64) * 100.0
}

/// 将Wei转换为Ether (f64格式，用于显示)
pub fn wei_to_ether_f64(wei: U256) -> f64 {
    const WEI_PER_ETHER: f64 = 1e18;
    let wei_f64 = wei.as_u128() as f64;
    wei_f64 / WEI_PER_ETHER
}

/// 将Ether转换为Wei
pub fn ether_to_wei(ether: f64) -> U256 {
    const WEI_PER_ETHER: f64 = 1e18;
    let wei_f64 = ether * WEI_PER_ETHER;
    U256::from(wei_f64 as u128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_min_amount_out() {
        let amount = U256::from(1000u64);
        let min_out = calculate_min_amount_out(amount, 100); // 1% slippage
        assert_eq!(min_out, U256::from(990u64));
        
        // Test edge case: 100% slippage should return 0
        let zero_out = calculate_min_amount_out(amount, 10000);
        assert_eq!(zero_out, U256::zero());
    }

    #[test]
    fn test_calculate_percentage() {
        let percentage = calculate_percentage(U256::from(25u64), U256::from(100u64));
        assert_eq!(percentage, 25.0);
        
        // Test zero total
        let zero_percentage = calculate_percentage(U256::from(25u64), U256::zero());
        assert_eq!(zero_percentage, 0.0);
    }

    #[test]
    fn test_wei_ether_conversion() {
        let ether_amount = 1.5;
        let wei_amount = ether_to_wei(ether_amount);
        let converted_back = wei_to_ether_f64(wei_amount);
        
        // Allow small floating point difference
        assert!((converted_back - ether_amount).abs() < 0.0001);
    }
}
