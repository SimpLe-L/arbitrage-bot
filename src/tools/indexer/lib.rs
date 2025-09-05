//! Dex indexer
//! Usage: see unit tests `test_get_pools`.

mod collector;
mod file_db;
mod protocols;
mod strategy;
pub mod types;

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use crate::infra::Engine;
use collector::QueryEventCollector;
use eyre::Result;
use strategy::PoolCreatedStrategy;
use ethers::types::{Address, BlockNumber, H256};
use tokio::task::JoinSet;
use tracing::info;
use types::{DummyExecutor, Event, NoAction, Pool, PoolCache, Protocol};

pub const FILE_DB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");

// WAVAX address - the native token for AVAX
pub const WAVAX_ADDRESS: &str = "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7";

pub fn supported_protocols() -> Vec<Protocol> {
    vec![
        Protocol::TraderJoe,
        Protocol::Pangolin,
        Protocol::SushiSwap,
        Protocol::UniswapV3,
        Protocol::Curve,
    ]
}

#[derive(Clone)]
pub struct DexIndexer {
    pool_cache: PoolCache,

    db: Arc<dyn DB>,
    _live_indexer_tasks: Arc<JoinSet<()>>,
}

impl DexIndexer {
    pub async fn new(http_url: &str) -> Result<Self> {
        let db = Arc::new(file_db::FileDB::new(FILE_DB_DIR, &supported_protocols())?);

        let timer = Instant::now();
        info!("loading token pools...");
        let pool_cache = db.load_token_pools(&supported_protocols())?;
        info!(elapsed = ?timer.elapsed(), token_pools_count = %pool_cache.token_pools.len(), token01_pools_count = %pool_cache.token01_pools.len(), "token pools loaded");

        let strategy = PoolCreatedStrategy::new(db.clone(), http_url, pool_cache.clone())?;
        strategy.backfill_pools().await?;

        // Build the bubbery engine
        let mut engine = Engine::<Event, NoAction>::new();
        let collector = QueryEventCollector::new();
        engine.add_collector(Box::new(collector));
        engine.add_strategy(Box::new(strategy));
        engine.add_executor(Box::new(DummyExecutor));

        let join_set = engine.run().await.expect("Burberry engine run failed");

        Ok(Self {
            pool_cache,
            db,
            _live_indexer_tasks: Arc::new(join_set),
        })
    }

    /// Get the pools by the given token address.
    pub fn get_pools_by_token(&self, token_address: &str) -> Option<HashSet<Pool>> {
        self.pool_cache.token_pools.get(token_address).map(|p| p.clone())
    }

    /// Get the pools by the given token01 addresses.
    pub fn get_pools_by_token01(&self, token0_address: &str, token1_address: &str) -> Option<HashSet<Pool>> {
        let key = token01_key(token0_address, token1_address);
        self.pool_cache.token01_pools.get(&key).map(|p| p.clone())
    }

    /// Get the pool by the given pool address.
    pub fn get_pool_by_address(&self, pool_address: &Address) -> Option<Pool> {
        self.pool_cache.pool_map.get(pool_address).map(|p| p.clone())
    }

    /// Get the pools count by the given protocol.
    pub fn pool_count(&self, protocol: &Protocol) -> usize {
        self.db.pool_count(protocol).unwrap_or_default()
    }

    /// Get all pools by the given protocol.
    pub fn get_all_pools(&self, protocol: &Protocol) -> Result<Vec<Pool>> {
        self.db.get_all_pools(protocol)
    }
}

#[inline]
pub fn token01_key(token0_address: &str, token1_address: &str) -> (String, String) {
    if token0_address < token1_address {
        (token0_address.to_string(), token1_address.to_string())
    } else {
        (token1_address.to_string(), token0_address.to_string())
    }
}

#[inline]
pub fn normalize_token_address(token_address: &str) -> String {
    if token_address == "0x0000000000000000000000000000000000000000" {
        WAVAX_ADDRESS.to_string()
    } else {
        token_address.to_lowercase()
    }
}

pub trait DB: Debug + Send + Sync {
    fn flush(&self, protocol: &Protocol, pools: &[Pool], block_number: Option<BlockNumber>) -> Result<()>;
    fn load_token_pools(&self, protocols: &[Protocol]) -> Result<PoolCache>;
    fn get_processed_blocks(&self) -> Result<HashMap<Protocol, Option<BlockNumber>>>;
    fn pool_count(&self, protocol: &Protocol) -> Result<usize>;
    fn get_all_pools(&self, protocol: &Protocol) -> Result<Vec<Pool>>;
}

#[cfg(test)]
mod tests {

    use super::*;

    pub const TEST_HTTP_URL: &str = "https://api.avax.network/ext/bc/C/rpc";
    const TOKEN0_ADDRESS: &str = "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7"; // WAVAX
    const TOKEN1_ADDRESS: &str = "0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664"; // USDC.e

    #[tokio::test]
    async fn test_get_pools() {
        // `DexIndexer::new` will backfill pools first.
        let indexer = DexIndexer::new(TEST_HTTP_URL).await.unwrap();

        // get pools by token
        let pools = indexer.get_pools_by_token(TOKEN0_ADDRESS).unwrap();
        println!("pools_len: {}", pools.len());
        println!("first pool: {:?}", pools.iter().next());

        // get pools by token01
        let pools = indexer.get_pools_by_token01(TOKEN0_ADDRESS, TOKEN1_ADDRESS).unwrap();
        println!("pools_len: {}", pools.len());
        println!("first pool: {:?}", pools.iter().next());
    }

    #[test]
    fn test_normalize_token_address() {
        assert_eq!(
            normalize_token_address("0x0000000000000000000000000000000000000000"),
            WAVAX_ADDRESS.to_string()
        );

        assert_eq!(normalize_token_address(TOKEN1_ADDRESS), TOKEN1_ADDRESS.to_lowercase());
    }

    #[tokio::test]
    async fn test_pools_count() {
        let indexer = DexIndexer::new(TEST_HTTP_URL).await.unwrap();

        for protocol in supported_protocols() {
            let count = indexer.pool_count(&protocol);
            println!("{}: {}", protocol, count);
        }
    }
}
