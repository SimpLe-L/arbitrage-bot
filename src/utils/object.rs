use eyre::{bail, OptionExt, Result};
use ethers::{
    abi::Token,
    types::{Address, U256},
};

pub fn extract_u256_from_tokens(tokens: &[Token], index: usize) -> Result<U256> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Uint(value) => Ok(*value),
        _ => bail!("expected uint256"),
    }
}

pub fn extract_address_from_tokens(tokens: &[Token], index: usize) -> Result<Address> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Address(addr) => Ok(*addr),
        _ => bail!("expected address"),
    }
}

pub fn extract_bool_from_tokens(tokens: &[Token], index: usize) -> Result<bool> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Bool(value) => Ok(*value),
        _ => bail!("expected bool"),
    }
}

pub fn extract_token_array_from_tokens(tokens: &[Token], index: usize) -> Result<Vec<Token>> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Array(tokens) => Ok(tokens.clone()),
        Token::FixedArray(tokens) => Ok(tokens.clone()),
        _ => bail!("expected array"),
    }
}

pub fn extract_u256_array_from_tokens(tokens: &[Token], index: usize) -> Result<Vec<U256>> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Array(tokens) => {
            tokens.iter()
                .map(|t| match t {
                    Token::Uint(value) => Ok(*value),
                    _ => bail!("expected uint256"),
                })
                .collect()
        },
        Token::FixedArray(tokens) => {
            tokens.iter()
                .map(|t| match t {
                    Token::Uint(value) => Ok(*value),
                    _ => bail!("expected uint256"),
                })
                .collect()
        },
        _ => bail!("expected array"),
    }
}

pub fn extract_address_array_from_tokens(tokens: &[Token], index: usize) -> Result<Vec<Address>> {
    let token = tokens.get(index).ok_or_eyre("Token index out of bounds")?;
    
    match token {
        Token::Array(tokens) => {
            tokens.iter()
                .map(|t| match t {
                    Token::Address(addr) => Ok(*addr),
                    _ => bail!("expected address"),
                })
                .collect()
        },
        Token::FixedArray(tokens) => {
            tokens.iter()
                .map(|t| match t {
                    Token::Address(addr) => Ok(*addr),
                    _ => bail!("expected address"),
                })
                .collect()
        },
        _ => bail!("expected array"),
    }
}

// EVM equivalent of shared_obj_arg - simple address wrapper
pub fn contract_address_arg(contract_address: Address) -> Address {
    contract_address
}
