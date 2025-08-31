//! 地址工具函数

use ethers::types::Address;
use std::str::FromStr;
use log::warn;

/// 验证地址格式是否正确
pub fn is_valid_address(addr_str: &str) -> bool {
    Address::from_str(addr_str).is_ok()
}

/// 将字符串转换为Address，如果失败返回零地址
pub fn parse_address_safe(addr_str: &str) -> Address {
    Address::from_str(addr_str).unwrap_or_else(|_| {
        warn!("无法解析地址: {}, 使用零地址", addr_str);
        Address::zero()
    })
}

/// 检查地址是否为零地址
pub fn is_zero_address(addr: &Address) -> bool {
    *addr == Address::zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_validation() {
        assert!(is_valid_address("0x1234567890123456789012345678901234567890"));
        assert!(!is_valid_address("invalid_address"));
        assert!(is_zero_address(&Address::zero()));
    }

    #[test]
    fn test_parse_address_safe() {
        let valid_addr = "0x1234567890123456789012345678901234567890";
        let parsed = parse_address_safe(valid_addr);
        assert!(!is_zero_address(&parsed));
        
        let invalid_addr = "invalid";
        let parsed_invalid = parse_address_safe(invalid_addr);
        assert!(is_zero_address(&parsed_invalid));
    }
}
