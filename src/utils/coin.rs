use std::str::FromStr;

use eyre::{eyre, Result};
use ethers::{
    providers::{Http, Provider, Middleware},
    types::{Address, U256},
    abi::{Abi, Token},
    contract::Contract,
};
use std::sync::Arc;

pub const AVAX_NATIVE_ADDRESS: Address = Address::zero(); // 0x0 represents native AVAX

pub async fn get_gas_balance(
    provider: &Arc<Provider<Http>>,
    owner: Address,
) -> Result<U256> {
    provider.get_balance(owner, None).await.map_err(Into::into)
}

pub async fn get_token_balance(
    provider: &Arc<Provider<Http>>,
    owner: Address,
    token_address: Address,
) -> Result<U256> {
    if token_address == AVAX_NATIVE_ADDRESS {
        // Native AVAX
        return get_gas_balance(provider, owner).await;
    }

    // ERC20 token balance
    let erc20_abi = r#"[
        {
            "constant": true,
            "inputs": [{"name": "owner", "type": "address"}],
            "name": "balanceOf",
            "outputs": [{"name": "", "type": "uint256"}],
            "type": "function"
        }
    ]"#;

    let abi: Abi = serde_json::from_str(erc20_abi)?;
    let contract = Contract::new(token_address, abi, provider.clone());

    let balance: U256 = contract
        .method("balanceOf", owner)?
        .call()
        .await?;

    Ok(balance)
}

pub async fn get_token_balance_with_min(
    provider: &Arc<Provider<Http>>,
    owner: Address,
    token_address: Address,
    min_balance: U256,
) -> Result<Option<U256>> {
    let balance = get_token_balance(provider, owner, token_address).await?;
    
    if balance >= min_balance {
        Ok(Some(balance))
    } else {
        Ok(None)
    }
}

pub async fn get_required_token_balance(
    provider: &Arc<Provider<Http>>,
    owner: Address,
    token_address: Address,
    min_balance: U256,
) -> Result<U256> {
    let balance = get_token_balance(provider, owner, token_address).await?;
    
    if balance >= min_balance {
        Ok(balance)
    } else {
        Err(eyre!("Insufficient balance. Required: {}, Available: {}", min_balance, balance))
    }
}

pub fn mocked_avax_balance(amount: U256) -> U256 {
    // For testing purposes, return the amount directly
    amount
}

pub fn is_native_token(token_address: &Address) -> bool {
    *token_address == AVAX_NATIVE_ADDRESS
}

pub fn format_avax_with_symbol(value: U256) -> String {
    let one_avax = U256::from(10u64.pow(18)); // 1 AVAX = 10^18 wei
    
    // Convert to f64 for display (losing some precision for very large numbers)
    let value_f64 = value.as_u128() as f64;
    let one_avax_f64 = one_avax.as_u128() as f64;
    let avax_value = value_f64 / one_avax_f64;

    format!("{:.6} AVAX", avax_value)
}

pub fn format_token_with_decimals(value: U256, decimals: u8, symbol: &str) -> String {
    let divisor = U256::from(10u64.pow(decimals as u32));
    
    // Convert to f64 for display
    let value_f64 = value.as_u128() as f64;
    let divisor_f64 = divisor.as_u128() as f64;
    let token_value = value_f64 / divisor_f64;

    format!("{:.6} {}", token_value, symbol)
}

pub fn parse_avax_amount(amount_str: &str) -> Result<U256> {
    let amount: f64 = amount_str.parse()?;
    let one_avax = U256::from(10u64.pow(18));
    
    // Convert f64 to wei (with potential precision loss)
    let wei_amount = (amount * 1e18) as u128;
    Ok(U256::from(wei_amount))
}

pub fn wei_to_avax(wei: U256) -> f64 {
    let wei_f64 = wei.as_u128() as f64;
    wei_f64 / 1e18
}

pub fn avax_to_wei(avax: f64) -> U256 {
    let wei = (avax * 1e18) as u128;
    U256::from(wei)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_avax() {
        let one_avax = U256::from(10u64.pow(18));
        let result = format_avax_with_symbol(one_avax);
        assert_eq!(result, "1.000000 AVAX");

        let half_avax = one_avax / 2;
        let result = format_avax_with_symbol(half_avax);
        assert_eq!(result, "0.500000 AVAX");
    }

    #[test]
    fn test_is_native_token() {
        assert!(is_native_token(&AVAX_NATIVE_ADDRESS));
        
        let erc20_address = Address::from_str("0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664").unwrap(); // USDC.e
        assert!(!is_native_token(&erc20_address));
    }

    #[test]
    fn test_wei_conversions() {
        let avax_amount = 1.5;
        let wei = avax_to_wei(avax_amount);
        let back_to_avax = wei_to_avax(wei);
        
        assert!((back_to_avax - avax_amount).abs() < 1e-10);
    }
}
