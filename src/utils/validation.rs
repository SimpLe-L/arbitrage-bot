//! 验证工具

/// 验证交易哈希格式
pub fn is_valid_tx_hash(hash: &str) -> bool {
    if !hash.starts_with("0x") {
        return false;
    }
    
    let hex_part = &hash[2..];
    hex_part.len() == 64 && hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// 验证私钥格式
pub fn is_valid_private_key(private_key: &str) -> bool {
    if !private_key.starts_with("0x") {
        return false;
    }
    
    let hex_part = &private_key[2..];
    hex_part.len() == 64 && hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

/// 验证URL格式
pub fn is_valid_url(url: &str) -> bool {
    url::Url::parse(url).is_ok()
}

/// 验证WebSocket URL格式
pub fn is_valid_ws_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        matches!(parsed.scheme(), "ws" | "wss")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_tx_hash() {
        // Valid transaction hash
        let valid_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(is_valid_tx_hash(valid_hash));
        
        // Invalid - no 0x prefix
        let no_prefix = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(!is_valid_tx_hash(no_prefix));
        
        // Invalid - wrong length
        let wrong_length = "0x123456";
        assert!(!is_valid_tx_hash(wrong_length));
        
        // Invalid - non-hex characters
        let non_hex = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdeg";
        assert!(!is_valid_tx_hash(non_hex));
    }

    #[test]
    fn test_is_valid_private_key() {
        // Valid private key
        let valid_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(is_valid_private_key(valid_key));
        
        // Invalid - no 0x prefix
        let no_prefix = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(!is_valid_private_key(no_prefix));
        
        // Invalid - wrong length
        let wrong_length = "0x123456";
        assert!(!is_valid_private_key(wrong_length));
    }

    #[test]
    fn test_is_valid_url() {
        assert!(is_valid_url("https://example.com"));
        assert!(is_valid_url("http://localhost:8080"));
        assert!(is_valid_url("ftp://files.example.com"));
        
        assert!(!is_valid_url("invalid_url"));
        assert!(!is_valid_url(""));
    }

    #[test]
    fn test_is_valid_ws_url() {
        assert!(is_valid_ws_url("ws://localhost:8080"));
        assert!(is_valid_ws_url("wss://api.avax.network/ext/bc/C/ws"));
        
        assert!(!is_valid_ws_url("http://localhost:8080"));
        assert!(!is_valid_ws_url("https://example.com"));
        assert!(!is_valid_ws_url("invalid"));
    }
}
