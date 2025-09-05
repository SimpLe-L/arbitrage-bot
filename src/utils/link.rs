use ethers::types::{Address, H256};

const SNOWTRACE_URL: &str = "https://snowtrace.io";

// https://snowtrace.io/tx/0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
pub fn tx(tx_hash: &H256, tag: Option<String>) -> String {
    format!(
        "[{tag_str}]({prefix}/tx/{tx_hash:#x})",
        tag_str = tag.unwrap_or_else(|| format!("{:#x}", tx_hash)),
        prefix = SNOWTRACE_URL,
        tx_hash = tx_hash,
    )
}

// https://snowtrace.io/address/0x1234567890abcdef1234567890abcdef12345678
pub fn address(address: &Address, tag: Option<String>) -> String {
    format!(
        "[{tag_str}]({prefix}/address/{address:#x})",
        tag_str = tag.unwrap_or_else(|| format!("{:#x}", address)),
        prefix = SNOWTRACE_URL,
        address = address,
    )
}

// For contracts, same as address but with different semantic meaning
pub fn contract(contract_address: &Address, tag: Option<String>) -> String {
    address(contract_address, tag)
}

// Alias for contract, maintaining compatibility with original code
pub fn object(contract_address: Address, tag: Option<String>) -> String {
    contract(&contract_address, tag)
}

// https://snowtrace.io/token/0xa7d7079b0fead91f3e65f86e8915cb59c1a4c664
pub fn token(token_address: &Address, tag: Option<String>) -> String {
    format!(
        "[{tag}]({prefix}/token/{token_address:#x})",
        tag = tag.unwrap_or_else(|| format!("{:#x}", token_address)),
        token_address = token_address,
        prefix = SNOWTRACE_URL,
    )
}

// For ERC20 tokens, alias for token
pub fn coin(token_address: &str, tag: Option<String>) -> String {
    // Handle native AVAX case
    if token_address == "0x0000000000000000000000000000000000000000" || token_address.to_lowercase() == "avax" {
        format!(
            "[{tag}]({prefix})",
            tag = tag.unwrap_or_else(|| "AVAX".to_string()),
            prefix = SNOWTRACE_URL,
        )
    } else {
        // Try to parse as address
        if let Ok(addr) = token_address.parse::<Address>() {
            token(&addr, tag)
        } else {
            // Fallback to generic tag
            format!(
                "[{tag}]({prefix})",
                tag = tag.unwrap_or_else(|| token_address.to_string()),
                prefix = SNOWTRACE_URL,
            )
        }
    }
}

// https://snowtrace.io/block/12345678
pub fn block(block_number: u64, tag: Option<String>) -> String {
    format!(
        "[{tag}]({prefix}/block/{block_number})",
        tag = tag.unwrap_or_else(|| block_number.to_string()),
        block_number = block_number,
        prefix = SNOWTRACE_URL,
    )
}

// https://snowtrace.io/blocks (for latest blocks)
pub fn latest_blocks() -> String {
    format!("[Latest Blocks]({}/blocks)", SNOWTRACE_URL)
}

// https://snowtrace.io/txs (for latest transactions)  
pub fn latest_transactions() -> String {
    format!("[Latest Transactions]({}/txs)", SNOWTRACE_URL)
}

// Helper function for account portfolio/holdings
pub fn account_tokens(address: &Address, tag: Option<String>) -> String {
    format!(
        "[{tag}]({prefix}/address/{address:#x}#tokentxns)",
        tag = tag.unwrap_or_else(|| format!("{:#x}", address)),
        prefix = SNOWTRACE_URL,
        address = address,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_tx_link() {
        let hash = H256::from_str("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap();
        let link = tx(&hash, None);
        assert!(link.contains("snowtrace.io/tx/"));
        assert!(link.contains("0x1234567890abcdef"));
    }

    #[test]
    fn test_address_link() {
        let addr = Address::from_str("0x1234567890abcdef1234567890abcdef12345678").unwrap();
        let link = address(&addr, Some("Test Address".to_string()));
        assert!(link.contains("snowtrace.io/address/"));
        assert!(link.contains("Test Address"));
    }

    #[test]
    fn test_token_link() {
        let token_addr = Address::from_str("0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664").unwrap(); // USDC.e
        let link = token(&token_addr, Some("USDC.e".to_string()));
        assert!(link.contains("snowtrace.io/token/"));
        assert!(link.contains("USDC.e"));
    }

    #[test]
    fn test_coin_native_avax() {
        let link = coin("avax", None);
        assert!(link.contains("AVAX"));
        assert!(link.contains("snowtrace.io"));
    }

    #[test]
    fn test_coin_erc20_address() {
        let link = coin("0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664", Some("USDC.e".to_string()));
        assert!(link.contains("snowtrace.io/token/"));
        assert!(link.contains("USDC.e"));
    }

    #[test]
    fn test_block_link() {
        let link = block(12345678, None);
        assert!(link.contains("snowtrace.io/block/"));
        assert!(link.contains("12345678"));
    }
}
