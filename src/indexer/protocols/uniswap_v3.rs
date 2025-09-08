use eyre::Result;
use ethers::{
    types::{Address, Log, H256, U256, I256},
    abi::{AbiDecode, AbiEncode},
};
use crate::types::{Pool, Token, PoolExtra, Protocol, SwapEvent};

// UniswapV3 Factory and Router addresses on AVAX
pub const UNISWAP_V3_FACTORY: &str = "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD";
pub const UNISWAP_V3_ROUTER: &str = "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE";

// Event signatures for UniswapV3
pub const POOL_CREATED_TOPIC: H256 = H256([
    0x78, 0x3c, 0xca, 0x1c, 0x0a, 0x2c, 0x97, 0xea, 0x15, 0xaa, 0xb3, 0xa8, 0x5d, 0xd0, 0x23, 0x11,
    0x08, 0x23, 0x15, 0x2e, 0x02, 0x2c, 0x17, 0x95, 0xc1, 0xcd, 0x6b, 0x73, 0xe7, 0x51, 0x72, 0x90
]);

pub const SWAP_TOPIC: H256 = H256([
    0xc4, 0x2b, 0x7f, 0x5a, 0xd0, 0xc4, 0x07, 0xa8, 0x59, 0x97, 0x05, 0xb8, 0xcc, 0x8c, 0x97, 0x83,
    0xba, 0x00, 0x5b, 0x0b, 0xb3, 0xf0, 0x7b, 0x86, 0x5a, 0x5b, 0x5b, 0x5b, 0x5b, 0x5b, 0x5b, 0x5b
]);

pub async fn uniswap_v3_related_contract_addresses() -> Vec<String> {
    vec![
        UNISWAP_V3_FACTORY.to_string(),
        UNISWAP_V3_ROUTER.to_string(),
    ]
}

#[derive(Debug, Clone)]
pub struct UniswapV3PoolCreated {
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub tick_spacing: i32,
    pub pool: Address,
}

impl TryFrom<&Log> for UniswapV3PoolCreated {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        // UniswapV3 PoolCreated event has indexed token0, token1, fee
        if log.topics.len() < 4 {
            return Err(eyre::eyre!("Invalid PoolCreated log: insufficient topics"));
        }

        let token0 = Address::from_slice(&log.topics[1].as_bytes()[12..32]);
        let token1 = Address::from_slice(&log.topics[2].as_bytes()[12..32]);
        
        // Fee is indexed as well
        let fee = u32::from_be_bytes([
            log.topics[3].as_bytes()[28],
            log.topics[3].as_bytes()[29], 
            log.topics[3].as_bytes()[30],
            log.topics[3].as_bytes()[31]
        ]);

        // tick_spacing and pool address are in data
        if log.data.len() < 64 {
            return Err(eyre::eyre!("Invalid PoolCreated log: insufficient data"));
        }

        let tick_spacing = i32::from_be_bytes([
            log.data.0[28], log.data.0[29], log.data.0[30], log.data.0[31]
        ]);
        
        let pool = Address::from_slice(&log.data.0[44..64]);

        Ok(Self {
            token0,
            token1,
            fee,
            tick_spacing,
            pool,
        })
    }
}

impl UniswapV3PoolCreated {
    pub async fn to_pool(&self) -> Result<Pool> {
        let tokens = vec![
            Token::new(&format!("{:?}", self.token0), 18),
            Token::new(&format!("{:?}", self.token1), 18),
        ];

        Ok(Pool {
            protocol: Protocol::UniswapV3,
            pool: self.pool,
            tokens,
            extra: PoolExtra::UniswapV3 { 
                fee_rate: self.fee as u64,
                tick_spacing: self.tick_spacing,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct UniswapV3SwapEvent {
    pub sender: Address,
    pub recipient: Address,
    pub amount0: I256,
    pub amount1: I256,
    pub sqrt_price_x96: U256,
    pub liquidity: u128,
    pub tick: i32,
    pub pool: Address,
}

impl TryFrom<&Log> for UniswapV3SwapEvent {
    type Error = eyre::Error;

    fn try_from(log: &Log) -> Result<Self> {
        if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
            return Err(eyre::eyre!("Invalid Swap log"));
        }

        // sender and recipient are indexed
        let sender = if log.topics.len() > 1 {
            Address::from_slice(&log.topics[1].as_bytes()[12..32])
        } else {
            Address::zero()
        };

        let recipient = if log.topics.len() > 2 {
            Address::from_slice(&log.topics[2].as_bytes()[12..32])
        } else {
            Address::zero()
        };

        // amount0, amount1, sqrtPriceX96, liquidity, tick are in data
        if log.data.len() < 160 {
            return Err(eyre::eyre!("Invalid Swap log: insufficient data"));
        }

        let amount0 = I256::from_raw(U256::from_big_endian(&log.data.0[0..32]));
        let amount1 = I256::from_raw(U256::from_big_endian(&log.data.0[32..64]));
        let sqrt_price_x96 = U256::from_big_endian(&log.data.0[64..96]);
        let liquidity = u128::from_be_bytes([
            log.data.0[112], log.data.0[113], log.data.0[114], log.data.0[115],
            log.data.0[116], log.data.0[117], log.data.0[118], log.data.0[119],
            log.data.0[120], log.data.0[121], log.data.0[122], log.data.0[123],
            log.data.0[124], log.data.0[125], log.data.0[126], log.data.0[127],
        ]);
        let tick = i32::from_be_bytes([
            log.data.0[156], log.data.0[157], log.data.0[158], log.data.0[159]
        ]);

        Ok(Self {
            sender,
            recipient,
            amount0,
            amount1,
            sqrt_price_x96,
            liquidity,
            tick,
            pool: log.address,
        })
    }
}

impl UniswapV3SwapEvent {
    pub async fn to_swap_event(&self, token0: String, token1: String) -> Result<SwapEvent> {
        let mut tokens_in = Vec::new();
        let mut tokens_out = Vec::new();
        let mut amounts_in = Vec::new();
        let mut amounts_out = Vec::new();

        // In UniswapV3, amounts can be positive or negative
        // Positive means outgoing, negative means incoming
        if self.amount0.is_negative() {
            tokens_in.push(token0.clone());
            amounts_in.push((-self.amount0).as_u64());
        } else if !self.amount0.is_zero() {
            tokens_out.push(token0.clone());
            amounts_out.push(self.amount0.as_u64());
        }

        if self.amount1.is_negative() {
            tokens_in.push(token1.clone());
            amounts_in.push((-self.amount1).as_u64());
        } else if !self.amount1.is_zero() {
            tokens_out.push(token1);
            amounts_out.push(self.amount1.as_u64());
        }

        Ok(SwapEvent {
            protocol: Protocol::UniswapV3,
            pool: Some(self.pool),
            tokens_in,
            tokens_out,
            amounts_in,
            amounts_out,
        })
    }
}

pub fn is_pool_created_event(log: &Log) -> bool {
    !log.topics.is_empty() && 
    log.address.to_string().to_lowercase() == UNISWAP_V3_FACTORY.to_lowercase() &&
    log.topics[0] == POOL_CREATED_TOPIC
}

pub fn is_swap_event(log: &Log) -> bool {
    !log.topics.is_empty() && log.topics[0] == SWAP_TOPIC
}
