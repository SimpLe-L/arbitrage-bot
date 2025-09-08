use eyre::Result;
use ethers::{
    types::{Address, Log, H256, U256},
    abi::{AbiDecode, AbiEncode},
};
use crate::types::{Pool, Token, PoolExtra, Protocol, SwapEvent};

// Curve contract addresses on AVAX
pub const CURVE_REGISTRY: &str = "0x7f90122BF0700F9E7e1F688fe926940E8839F353";
pub const CURVE_ADDRESS_PROVIDER: &str = "0x8474DdbE98F5aA3179B3B3F5942D724aFcdec9f6";

// Event signatures for Curve
pub const POOL_ADDED_TOPIC: H256 = H256([
    0x95, 0xeb, 0x19, 0x5f, 0xce, 0x97, 0x43, 0x52, 0xf8, 0x3a, 0x45, 0xf4, 0x0b, 0x34, 0x12, 0x45,
    0x98, 0x76, 0x54, 0x32, 0x10, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45
]);

pub const TOKEN_EXCHANGE_TOPIC: H256 = H256([
    0x8b, 0x3e, 0x96, 0xf2, 0xc0, 0xb7, 0x03, 0xe1, 0xe4, 0x23, 0x67, 0x8b, 0x9c, 0x2d, 0x3e, 0x4f,
    0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56
]);

pub async fn curve_related_contract_addresses() -> Vec<String> {
    vec![
        CURVE_REGISTRY.to_string(),
        CURVE_ADDRESS_PROVIDER.to_string(),
    ]
}

#[derive(Debug, Clone)]
pub struct CurvePoolAdded {
    pub pool: Address,
    pub rate_method_id: [u8; 4],
}

impl TryFrom<&Log> for CurvePoolAdded {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        if log.topics.len() < 2 {
            return Err(eyre::eyre!("Invalid PoolAdded log: insufficient topics"));
        }

        let pool = Address::from_slice(&log.topics[1].as_bytes()[12..32]);
        
        // Extract rate_method_id from data if available
        let rate_method_id = if log.data.len() >= 4 {
            [log.data.0[0], log.data.0[1], log.data.0[2], log.data.0[3]]
        } else {
            [0u8; 4]
        };

        Ok(Self {
            pool,
            rate_method_id,
        })
    }
}

impl CurvePoolAdded {
    pub async fn to_pool(&self, tokens: Vec<Token>) -> Result<Pool> {
        Ok(Pool {
            protocol: Protocol::Curve,
            pool: self.pool,
            tokens,
            extra: PoolExtra::Curve { 
                fee_rate: 4, // 0.04% typical fee for Curve
                amplification: 2000, // Default amplification parameter
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct CurveTokenExchange {
    pub buyer: Address,
    pub sold_id: U256,
    pub tokens_sold: U256,
    pub bought_id: U256,
    pub tokens_bought: U256,
    pub pool: Address,
}

impl TryFrom<&Log> for CurveTokenExchange {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        if log.topics.is_empty() || log.topics[0] != TOKEN_EXCHANGE_TOPIC {
            return Err(eyre::eyre!("Invalid TokenExchange log"));
        }

        // buyer is indexed
        let buyer = if log.topics.len() > 1 {
            Address::from_slice(&log.topics[1].as_bytes()[12..32])
        } else {
            Address::zero()
        };

        // sold_id, tokens_sold, bought_id, tokens_bought are in data
        if log.data.len() < 128 {
            return Err(eyre::eyre!("Invalid TokenExchange log: insufficient data"));
        }

        let sold_id = U256::from_big_endian(&log.data.0[0..32]);
        let tokens_sold = U256::from_big_endian(&log.data.0[32..64]);
        let bought_id = U256::from_big_endian(&log.data.0[64..96]);
        let tokens_bought = U256::from_big_endian(&log.data.0[96..128]);

        Ok(Self {
            buyer,
            sold_id,
            tokens_sold,
            bought_id,
            tokens_bought,
            pool: log.address,
        })
    }
}

impl CurveTokenExchange {
    pub async fn to_swap_event(&self, pool_tokens: &[Token]) -> Result<SwapEvent> {
        let mut tokens_in = Vec::new();
        let mut tokens_out = Vec::new();
        let mut amounts_in = Vec::new();
        let mut amounts_out = Vec::new();

        // Get token addresses by ID
        let sold_id_usize = self.sold_id.as_usize();
        let bought_id_usize = self.bought_id.as_usize();

        if let Some(sold_token) = pool_tokens.get(sold_id_usize) {
            tokens_in.push(sold_token.token_address.clone());
            amounts_in.push(self.tokens_sold.as_u64());
        }

        if let Some(bought_token) = pool_tokens.get(bought_id_usize) {
            tokens_out.push(bought_token.token_address.clone());
            amounts_out.push(self.tokens_bought.as_u64());
        }

        Ok(SwapEvent {
            protocol: Protocol::Curve,
            pool: Some(self.pool),
            tokens_in,
            tokens_out,
            amounts_in,
            amounts_out,
        })
    }
}

pub fn is_pool_added_event(log: &Log) -> bool {
    !log.topics.is_empty() && 
    log.address.to_string().to_lowercase() == CURVE_REGISTRY.to_lowercase() &&
    log.topics[0] == POOL_ADDED_TOPIC
}

pub fn is_token_exchange_event(log: &Log) -> bool {
    !log.topics.is_empty() && log.topics[0] == TOKEN_EXCHANGE_TOPIC
}

// Helper function to create a basic Curve pool with common stablecoins
pub fn create_default_curve_pool(pool_address: Address) -> Pool {
    let tokens = vec![
        Token::new("0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664", 6), // USDC.e
        Token::new("0xc7198437980c041c805A1EDcbA50c1Ce5db95118", 6), // USDT.e
        Token::new("0xd586E7F844cEa2F87f50152665BCbc2C279D8d70", 18), // DAI.e
    ];

    Pool {
        protocol: Protocol::Curve,
        pool: pool_address,
        tokens,
        extra: PoolExtra::Curve {
            fee_rate: 4,
            amplification: 2000,
        },
    }
}
