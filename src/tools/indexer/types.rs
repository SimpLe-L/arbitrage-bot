use std::{
    collections::HashSet,
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::infra::{async_trait, Executor};
use dashmap::DashMap;
use eyre::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use ethers::types::{Address, Log, H256, BlockNumber};
use tracing::error;

use crate::normalize_token_address;

// token_address -> pools
pub type TokenPools = DashMap<String, HashSet<Pool>>;
// (token0_address, token1_address) -> pools
pub type Token01Pools = DashMap<(String, String), HashSet<Pool>>;

#[derive(Debug, Clone)]
pub struct PoolCache {
    pub token_pools: Arc<TokenPools>,
    pub token01_pools: Arc<Token01Pools>,
    pub pool_map: Arc<DashMap<Address, Pool>>,
}

impl PoolCache {
    pub fn new(token_pools: TokenPools, token01_pools: Token01Pools, pool_map: DashMap<Address, Pool>) -> Self {
        Self {
            token_pools: Arc::new(token_pools),
            token01_pools: Arc::new(token01_pools),
            pool_map: Arc::new(pool_map),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub protocol: Protocol,
    pub pool: Address,
    pub tokens: Vec<Token>,
    pub extra: PoolExtra,
}

impl PartialEq for Pool {
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool
    }
}

impl Eq for Pool {}

impl Hash for Pool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pool.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub token_address: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolExtra {
    None,
    TraderJoe {
        fee_rate: u64,
    },
    Pangolin {
        fee_rate: u64,
    },
    SushiSwap {
        fee_rate: u64,
    },
    UniswapV3 {
        fee_rate: u64,
        tick_spacing: i32,
    },
    Curve {
        fee_rate: u64,
        amplification: u64,
    },
}

impl fmt::Display for Pool {
    // protocol|pool|tokens|extra
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}|{}|{}|{}",
            self.protocol,
            self.pool,
            serde_json::to_string(&self.tokens).unwrap(),
            serde_json::to_string(&self.extra).unwrap()
        )
    }
}

impl TryFrom<&str> for Pool {
    type Error = eyre::Error;

    fn try_from(value: &str) -> Result<Self> {
        let parts: Vec<&str> = value.split('|').collect();
        ensure!(parts.len() == 4, "Invalid pool format: {}", value);

        let protocol = Protocol::try_from(parts[0])?;
        let pool = parts[1].parse()?;
        let tokens: Vec<Token> = serde_json::from_str(parts[2])?;
        let extra: PoolExtra = serde_json::from_str(parts[3])?;

        Ok(Pool {
            protocol,
            pool,
            tokens,
            extra,
        })
    }
}

impl Pool {
    pub fn token0_address(&self) -> String {
        self.tokens[0].token_address.clone()
    }

    pub fn token1_address(&self) -> String {
        self.tokens[1].token_address.clone()
    }

    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn token_index(&self, token_address: &str) -> Option<usize> {
        self.tokens.iter().position(|token| token.token_address == token_address)
    }

    pub fn token(&self, index: usize) -> Option<Token> {
        self.tokens.get(index).cloned()
    }

    // (token0_address, token1_address)
    pub fn token01_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for i in 0..self.tokens.len() {
            for j in i + 1..self.tokens.len() {
                pairs.push((self.tokens[i].token_address.clone(), self.tokens[j].token_address.clone()));
            }
        }

        pairs
    }

}

impl Token {
    pub fn new(token_address: &str, decimals: u8) -> Self {
        Self {
            token_address: normalize_token_address(token_address),
            decimals,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwapEvent {
    pub protocol: Protocol,
    pub pool: Option<Address>,
    pub tokens_in: Vec<String>,
    pub tokens_out: Vec<String>,
    pub amounts_in: Vec<u64>,
    pub amounts_out: Vec<u64>,
}

impl SwapEvent {
    pub fn pool_address(&self) -> Option<Address> {
        self.pool
    }

    pub fn involved_token_one_side(&self) -> String {
        if self.tokens_in[0] != crate::WAVAX_ADDRESS {
            self.tokens_in[0].to_string()
        } else {
            self.tokens_out[0].to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    TraderJoe,
    Pangolin,
    SushiSwap,
    UniswapV3,
    Curve,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::TraderJoe => write!(f, "trader_joe"),
            Protocol::Pangolin => write!(f, "pangolin"),
            Protocol::SushiSwap => write!(f, "sushi_swap"),
            Protocol::UniswapV3 => write!(f, "uniswap_v3"),
            Protocol::Curve => write!(f, "curve"),
        }
    }
}

impl TryFrom<&str> for Protocol {
    type Error = eyre::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "trader_joe" => Ok(Protocol::TraderJoe),
            "pangolin" => Ok(Protocol::Pangolin),
            "sushi_swap" => Ok(Protocol::SushiSwap),
            "uniswap_v3" => Ok(Protocol::UniswapV3),
            "curve" => Ok(Protocol::Curve),
            _ => bail!("Unsupported protocol: {}", value),
        }
    }
}

impl Protocol {
    pub async fn related_contract_addresses(&self) -> Result<HashSet<String>> {
        let res = match self {
            Protocol::TraderJoe => vec![
                "0x60aE616a2155Ee3d9A68541Ba4544862310933d4".to_string(), // TraderJoe Router
                "0x9Ad6C38BE94206cA50bb0d90783181662f0Cfa10".to_string(), // TraderJoe Factory
            ],
            Protocol::Pangolin => vec![
                "0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106".to_string(), // Pangolin Router
                "0xefa94DE7a4656D787667C749f7E1223D71E9FD88".to_string(), // Pangolin Factory
            ],
            Protocol::SushiSwap => vec![
                "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string(), // SushiSwap Router
                "0xc35DADB65012eC5796536bD9864eD8773aBc74C4".to_string(), // SushiSwap Factory
            ],
            Protocol::UniswapV3 => vec![
                "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE".to_string(), // UniswapV3 Router
                "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD".to_string(), // UniswapV3 Factory
            ],
            Protocol::Curve => vec![
                "0x7f90122BF0700F9E7e1F688fe926940E8839F353".to_string(), // Curve Pool Registry
                "0x8474DdbE98F5aA3179B3B3F5942D724aFcdec9f6".to_string(), // Curve Address Provider
            ],
        }
        .into_iter()
        .collect::<HashSet<String>>();

        Ok(res)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    QueryEventTrigger,
}

#[derive(Debug, Clone)]
pub struct NoAction;

#[derive(Debug, Clone)]
pub struct DummyExecutor;

#[async_trait]
impl Executor<NoAction> for DummyExecutor {
    async fn execute(&self, _action: NoAction) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "DummyDexIndexerExecutor"
    }
}
