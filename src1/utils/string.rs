//! 字符串处理工具

/// 截断字符串到指定长度，如果超长则添加省略号
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// 格式化大数值为可读字符串（添加千分符）
pub fn format_number(num: u64) -> String {
    let num_str = num.to_string();
    let mut result = String::new();
    
    for (i, ch) in num_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, ch);
    }
    
    result
}

/// 将十六进制字符串转换为字节数组
pub fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, hex::FromHexError> {
    let hex_str = if hex_str.starts_with("0x") {
        &hex_str[2..]
    } else {
        hex_str
    };
    hex::decode(hex_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        // Test normal case
        let short = "hello";
        assert_eq!(truncate_string(short, 10), "hello");
        
        // Test truncation
        let long = "this is a very long string";
        let truncated = truncate_string(long, 10);
        assert_eq!(truncated, "this is...");
        assert!(truncated.len() <= 10);
        
        // Test edge case with max_len < 3
        let edge_case = truncate_string(long, 2);
        assert_eq!(edge_case.len(), 5); // "..." is always added even if max_len < 3
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(1000000000), "1,000,000,000");
    }

    #[test]
    fn test_hex_to_bytes() {
        // Test with 0x prefix
        let hex_with_prefix = "0x48656c6c6f";
        let bytes = hex_to_bytes(hex_with_prefix).unwrap();
        assert_eq!(bytes, b"Hello");
        
        // Test without prefix
        let hex_without_prefix = "48656c6c6f";
        let bytes = hex_to_bytes(hex_without_prefix).unwrap();
        assert_eq!(bytes, b"Hello");
        
        // Test invalid hex
        let invalid_hex = "invalid_hex";
        assert!(hex_to_bytes(invalid_hex).is_err());
    }
}
