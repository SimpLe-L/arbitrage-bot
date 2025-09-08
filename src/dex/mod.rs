mod curve;
mod indexer_searcher;
mod pangolin;
mod sushi_swap;
mod trade;
mod trader_joe;
mod uniswap_v3;
mod utils;

use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
    sync::Arc,
};

use ::utils::coin;
use dex_indexer::types::Protocol;
use eyre::{bail, ensure, Result};
pub use indexer_searcher::IndexerDexSearcher;
use object_pool::ObjectPool;
use simulator::{SimulateCtx, Simulator};
use ethers::types::{Address, TransactionRequest};
use tokio::task::JoinSet;
use tracing::Instrument;
use trade::{FlashResult, TradeResult};
pub use trade::{Path, TradeCtx, TradeType, Trader};

use crate::{config::pegged_coin_types, types::Source};

const MAX_HOP_COUNT: usize = 2;
const MAX_POOL_COUNT: usize = 10;
const MIN_LIQUIDITY: u128 = 1000;

// WAVAX address - the native token for most swaps
pub const WAVAX_ADDRESS: &str = "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7";

#[async_trait::async_trait]
pub trait DexSearcher: Send + Sync {
    // token_address: e.g. "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7"
    async fn find_dexes(&self, token_in_address: &str, token_out_address: Option<String>) -> Result<Vec<Box<dyn Dex>>>;

    async fn find_test_path(&self, path: &[Address]) -> Result<Path>;
}

#[async_trait::async_trait]
pub trait Dex: Send + Sync + CloneBoxedDex {
    fn support_flashloan(&self) -> bool {
        false
    }

    /// Extend the trade_tx with a flashloan tx.
    /// Returns (token_out, receipt).
    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        bail!("flashloan not supported")
    }

    /// Extend the trade_tx with a repay tx.
    /// Returns the token_profit after repaying the flashloan.
    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _token: ethers::types::Bytes, _flash_res: FlashResult) -> Result<ethers::types::Bytes> {
        bail!("flashloan not supported")
    }

    /// Extend the trade_tx with a swap tx.
    /// Returns token_out for the next swap.
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: Address,
        token_in: ethers::types::Bytes,
        amount_in: Option<u64>,
    ) -> Result<ethers::types::Bytes>;

    fn coin_in_type(&self) -> String;
    fn coin_out_type(&self) -> String;
    fn protocol(&self) -> Protocol;
    fn liquidity(&self) -> u128;
    fn pool_address(&self) -> Address;

    /// flip the coin_in_type and coin_out_type
    fn flip(&mut self);

    // for debug
    fn is_a2b(&self) -> bool;
    async fn swap_tx(&self, sender: Address, recipient: Address, amount_in: u64) -> Result<TransactionRequest>;
}

pub trait CloneBoxedDex {
    fn clone_boxed(&self) -> Box<dyn Dex>;
}

impl<T> CloneBoxedDex for T
where
    T: 'static + Dex + Clone,
{
    fn clone_boxed(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Dex> {
    fn clone(&self) -> Box<dyn Dex> {
        self.clone_boxed()
    }
}

impl PartialEq for Box<dyn Dex> {
    fn eq(&self, other: &Self) -> bool {
        self.pool_address() == other.pool_address()
    }
}

impl Eq for Box<dyn Dex> {}

impl Hash for Box<dyn Dex> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pool_address().hash(state);
    }
}

impl fmt::Debug for Box<dyn Dex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({}, {}, {})",
            self.protocol(),
            self.pool_address(),
            self.coin_in_type(),
            self.coin_out_type()
        )
    }
}

#[derive(Clone)]
pub struct Defi {
    dex_searcher: Arc<dyn DexSearcher>,
    trader: Arc<Trader>,
}

impl Defi {
    pub async fn new(http_url: &str, simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        let dex_searcher = IndexerDexSearcher::new(http_url, simulator_pool.clone()).await?;
        let trade = Trader::new(simulator_pool).await?;

        Ok(Self {
            dex_searcher: Arc::new(dex_searcher),
            trader: Arc::new(trade),
        })
    }

    #[allow(dead_code)]
    pub async fn find_dexes(&self, token_in_address: &str, token_out_address: Option<String>) -> Result<Vec<Box<dyn Dex>>> {
        self.dex_searcher.find_dexes(token_in_address, token_out_address).await
    }

    pub async fn find_sell_paths(&self, token_in_address: &str) -> Result<Vec<Path>> {
        if coin::is_native_coin(token_in_address) {
            return Ok(vec![Path::default()]);
        }

        let mut all_hops = HashMap::new();
        let mut stack = vec![token_in_address.to_string()];
        let mut visited = HashSet::new();
        let mut visited_dexes = HashSet::new();

        for nth_hop in 0..MAX_HOP_COUNT {
            let is_last_hop = nth_hop == MAX_HOP_COUNT - 1;
            let mut new_stack = vec![];

            while let Some(token_address) = stack.pop() {
                if visited.contains(&token_address) || coin::is_native_coin(&token_address) {
                    continue;
                }
                visited.insert(token_address.clone());

                let token_out_address = if pegged_coin_types().contains(token_address.as_str()) || is_last_hop {
                    Some(WAVAX_ADDRESS.to_string())
                } else {
                    None
                };
                let mut dexes = if let Ok(dexes) = self.dex_searcher.find_dexes(&token_address, token_out_address).await {
                    dexes
                } else {
                    continue;
                };

                dexes.retain(|dex| dex.liquidity() >= MIN_LIQUIDITY);

                if dexes.len() > MAX_POOL_COUNT {
                    dexes.retain(|dex| !visited_dexes.contains(&dex.pool_address()));
                    dexes.sort_by_key(|dex| std::cmp::Reverse(dex.liquidity()));
                    dexes.truncate(MAX_POOL_COUNT);
                }

                if dexes.is_empty() {
                    continue;
                }

                for dex in &dexes {
                    let out_token_address = dex.coin_out_type();
                    if !visited.contains(&out_token_address) {
                        new_stack.push(out_token_address.clone());
                    }
                    visited_dexes.insert(dex.pool_address());
                }
                all_hops.insert(token_address.clone(), dexes);
            }

            if is_last_hop {
                break;
            }

            stack = new_stack;
        }

        let mut routes = vec![];
        dfs(token_in_address, &mut vec![], &all_hops, &mut routes);

        Ok(routes.into_iter().map(Path::new).collect())
    }

    pub async fn find_buy_paths(&self, token_out_address: &str) -> Result<Vec<Path>> {
        let mut paths = self.find_sell_paths(token_out_address).await?;
        for path in &mut paths {
            path.path.reverse();
            for dex in &mut path.path {
                dex.flip();
            }
        }

        Ok(paths)
    }

    pub async fn find_best_path_exact_in(
        &self,
        paths: &[Path],
        sender: Address,
        amount_in: u64,
        trade_type: TradeType,
        gas_limit: u64,
        sim_ctx: &SimulateCtx,
    ) -> Result<PathTradeResult> {
        let mut joinset = JoinSet::new();

        for (idx, path) in paths.iter().enumerate() {
            if path.is_empty() {
                continue;
            }

            let trade = self.trader.clone();
            let path = path.clone();
            let sim_ctx = sim_ctx.clone();

            joinset.spawn(
                async move {
                    let result = trade
                        .get_trade_result(&path, sender, amount_in, trade_type, gas_limit, sim_ctx)
                        .await;

                    (idx, result)
                }
                .in_current_span(),
            );
        }

        let (mut best_idx, mut best_trade_res) = (0, TradeResult::default());
        while let Some(Ok((idx, trade_res))) = joinset.join_next().await {
            match trade_res {
                Ok(trade_res) => {
                    if trade_res > best_trade_res {
                        best_idx = idx;
                        best_trade_res = trade_res;
                    }
                }
                Err(_error) => {
                    // tracing::error!(path = ?paths[idx], ?error, "trade
                    // error");
                }
            }
        }

        ensure!(best_trade_res.amount_out > 0, "zero amount_out");

        Ok(PathTradeResult::new(paths[best_idx].clone(), amount_in, best_trade_res))
    }

    pub async fn build_final_tx_data(
        &self,
        sender: Address,
        amount_in: u64,
        path: &Path,
        gas_limit: u64,
        gas_price: u64,
        source: Source,
    ) -> Result<TransactionRequest> {
        let (tx_data, _) = self
            .trader
            .get_flashloan_trade_tx(path, sender, amount_in, gas_limit, gas_price, source)
            .await?;

        Ok(tx_data)
    }
}

fn dfs(
    token_address: &str,
    path: &mut Vec<Box<dyn Dex>>,
    hops: &HashMap<String, Vec<Box<dyn Dex>>>,
    routes: &mut Vec<Vec<Box<dyn Dex>>>,
) {
    if coin::is_native_coin(token_address) {
        routes.push(path.clone());
        return;
    }
    if path.len() >= MAX_HOP_COUNT {
        return;
    }
    if !hops.contains_key(token_address) {
        return;
    }
    for dex in hops.get(token_address).unwrap() {
        path.push(dex.clone());
        dfs(&dex.coin_out_type(), path, hops, routes);
        path.pop();
    }
}

#[derive(Debug, Clone)]
pub struct PathTradeResult {
    pub path: Path,
    pub amount_in: u64,
    pub amount_out: u64,
    pub gas_cost: i64,
    pub cache_misses: u64,
}

impl PathTradeResult {
    pub fn new(path: Path, amount_in: u64, trade_res: TradeResult) -> Self {
        Self {
            path,
            amount_in,
            amount_out: trade_res.amount_out,
            gas_cost: trade_res.gas_cost,
            cache_misses: trade_res.cache_misses,
        }
    }

    pub fn profit(&self) -> i128 {
        if self.path.coin_in_type() == WAVAX_ADDRESS {
            if self.path.coin_out_type() == WAVAX_ADDRESS {
                return self.amount_out as i128 - self.amount_in as i128 - self.gas_cost as i128;
            }
            0 - self.gas_cost as i128 - self.amount_in as i128
        } else {
            0
        }
    }
}

impl fmt::Display for PathTradeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PathTradeResult {{ amount_in: {}, amount_out: {}, profit: {}, path: {:?} ... }}",
            self.amount_in,
            self.amount_out,
            self.profit(),
            self.path
        )
    }
}

#[cfg(test)]
mod tests {

    use simulator::HttpSimulator;
    use tracing::info;

    use super::*;
    use crate::config::tests::TEST_HTTP_URL;

    #[tokio::test]
    async fn test_find_sell_paths() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let simulator_pool = ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        });

        let defi = Defi::new(TEST_HTTP_URL, Arc::new(simulator_pool)).await.unwrap();

        let token_in_address = "0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664"; // USDC.e
        let paths = defi.find_sell_paths(token_in_address).await.unwrap();
        assert!(!paths.is_empty(), "No sell paths found");

        for path in paths {
            info!(?path, "sell")
        }
    }

    #[tokio::test]
    async fn test_find_buy_paths() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let simulator_pool = ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        });

        let defi = Defi::new(TEST_HTTP_URL, Arc::new(simulator_pool)).await.unwrap();

        let token_out_address = "0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664"; // USDC.e
        let paths = defi.find_buy_paths(token_out_address).await.unwrap();
        assert!(!paths.is_empty(), "No buy paths found");
        for path in paths {
            info!(?path, "buy")
        }
    }
}
