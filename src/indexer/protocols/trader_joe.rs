use eyre::Result;
use ethers::{
    types::{Address, Log, H256, U256},
    abi::{AbiDecode, AbiEncode},
    utils::keccak256,
};
use crate::types::{Pool, Token, PoolExtra, Protocol, SwapEvent};

// TraderJoe Factory and Router addresses on AVAX
pub const TRADER_JOE_FACTORY: &str = "0x9Ad6C38BE94206cA50bb0d90783181662f0Cfa10";
pub const TRADER_JOE_ROUTER: &str = "0x60aE616a2155Ee3d9A68541Ba4544862310933d4";

// Event signatures
pub const PAIR_CREATED_TOPIC: H256 = H256([
    0x0d, 0x3c, 0x4c, 0x0e, 0x5e, 0x6a, 0x62, 0x48, 0x6a, 0x5a, 0xe9, 0xe4, 0x1e, 0x98, 0x4c, 0x8b,
    0xc9, 0x3f, 0x72, 0x6a, 0x5e, 0x98, 0x4c, 0x8b, 0xc9, 0x3f, 0x72, 0x6a, 0x5e, 0x98, 0x4c, 0x8b
]);

pub const SWAP_TOPIC: H256 = H256([
    0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b, 0x6c, 0xa1, 0x23, 0x72, 0xe5, 0x9d, 0xc2, 0x9c,
    0xe7, 0xf8, 0xed, 0x46, 0x7f, 0xa2, 0x16, 0x5e, 0x8e, 0x17, 0xfb, 0x6d, 0xfe, 0x8e, 0x6b, 0x8b
]);

pub async fn trader_joe_related_contract_addresses() -> Vec<String> {
    vec![
        TRADER_JOE_FACTORY.to_string(),
        TRADER_JOE_ROUTER.to_string(),
    ]
}

#[derive(Debug, Clone)]
pub struct TraderJoePoolCreated {
    pub token0: Address,
    pub token1: Address,
    pub pair: Address,
    pub all_pairs_length: U256,
}

impl TryFrom<&Log> for TraderJoePoolCreated {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        // Verify this is a PairCreated event
        if log.topics.len() < 4 {
            return Err(eyre::eyre!("Invalid PairCreated log: insufficient topics"));
        }

        // Extract data from topics (indexed parameters)
        let token0 = Address::from_slice(&log.topics[1].as_bytes()[12..32]);
        let token1 = Address::from_slice(&log.topics[2].as_bytes()[12..32]);
        let pair = Address::from_slice(&log.topics[3].as_bytes()[12..32]);
        
        // Extract all_pairs_length from data (non-indexed parameter)
        let all_pairs_length = if log.data.len() >= 32 {
            U256::from_big_endian(&log.data.0[0..32])
        } else {
            U256::zero()
        };

        Ok(Self {
            token0,
            token1,
            pair,
            all_pairs_length,
        })
    }
}

impl TraderJoePoolCreated {
    pub async fn to_pool(&self) -> Result<Pool> {
        // For TraderJoe, we use default decimals of 18. In a production system,
        // you would query the token contracts to get the actual decimals.
        let tokens = vec![
            Token::new(&format!("{:?}", self.token0), 18),
            Token::new(&format!("{:?}", self.token1), 18),
        ];

        Ok(Pool {
            protocol: Protocol::TraderJoe,
            pool: self.pair,
            tokens,
            extra: PoolExtra::TraderJoe { fee_rate: 300 }, // 0.3% fee for TraderJoe
        })
    }
}

#[derive(Debug, Clone)]
pub struct TraderJoeSwapEvent {
    pub sender: Address,
    pub amount0_in: U256,
    pub amount1_in: U256,
    pub amount0_out: U256,
    pub amount1_out: U256,
    pub to: Address,
    pub pair: Address,
}

impl TryFrom<&Log> for TraderJoeSwapEvent {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        // Verify this is a Swap event
        if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
            return Err(eyre::eyre!("Invalid Swap log"));
        }

        // Extract indexed parameters from topics
        let sender = if log.topics.len() > 1 {
            Address::from_slice(&log.topics[1].as_bytes()[12..32])
        } else {
            Address::zero()
        };

        let to = if log.topics.len() > 2 {
            Address::from_slice(&log.topics[2].as_bytes()[12..32])
        } else {
            Address::zero()
        };

        // Extract non-indexed parameters from data
        if log.data.len() < 128 {
            return Err(eyre::eyre!("Invalid Swap log: insufficient data"));
        }

        let amount0_in = U256::from_big_endian(&log.data.0[0..32]);
        let amount1_in = U256::from_big_endian(&log.data.0[32..64]);
        let amount0_out = U256::from_big_endian(&log.data.0[64..96]);
        let amount1_out = U256::from_big_endian(&log.data.0[96..128]);

        Ok(Self {
            sender,
            amount0_in,
            amount1_in,
            amount0_out,
            amount1_out,
            to,
            pair: log.address,
        })
    }
}

impl TraderJoeSwapEvent {
    pub async fn to_swap_event(&self, token0: String, token1: String) -> Result<SwapEvent> {
        let mut tokens_in = Vec::new();
        let mut tokens_out = Vec::new();
        let mut amounts_in = Vec::new();
        let mut amounts_out = Vec::new();

        // Determine which tokens were swapped in/out
        if !self.amount0_in.is_zero() {
            tokens_in.push(token0.clone());
            amounts_in.push(self.amount0_in.as_u64());
        }
        if !self.amount1_in.is_zero() {
            tokens_in.push(token1.clone());
            amounts_in.push(self.amount1_in.as_u64());
        }
        if !self.amount0_out.is_zero() {
            tokens_out.push(token0);
            amounts_out.push(self.amount0_out.as_u64());
        }
        if !self.amount1_out.is_zero() {
            tokens_out.push(token1);
            amounts_out.push(self.amount1_out.as_u64());
        }

        Ok(SwapEvent {
            protocol: Protocol::TraderJoe,
            pool: Some(self.pair),
            tokens_in,
            tokens_out,
            amounts_in,
            amounts_out,
        })
    }
}

// Helper function to check if a log is a TraderJoe PairCreated event
pub fn is_pair_created_event(log: &Log) -> bool {
    !log.topics.is_empty() && 
    log.address.to_string().to_lowercase() == TRADER_JOE_FACTORY.to_lowercase() &&
    log.topics[0] == PAIR_CREATED_TOPIC
}

// Helper function to check if a log is a TraderJoe Swap event
pub fn is_swap_event(log: &Log) -> bool {
    !log.topics.is_empty() && log.topics[0] == SWAP_TOPIC
}
