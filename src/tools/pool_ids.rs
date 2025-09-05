use std::collections::HashSet;
use std::fs;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use dex_indexer::{types::Protocol, DexIndexer};
use eyre::Result;
use mev_logger::LevelFilter;
use object_pool::ObjectPool;
use simulator::{DBSimulator, SimulateCtx, Simulator};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use ethers::types::{Address, H160};
use tracing::info;

use crate::common::get_latest_block;
use crate::defi::{DexSearcher, IndexerDexSearcher, TradeType, Trader};
use crate::HttpConfig;

#[derive(Clone, Debug, Parser)]
pub struct Args {
    #[clap(long, default_value = "./pool_related_ids.txt")]
    pub result_path: String,

    #[command(flatten)]
    pub http_config: HttpConfig,

    #[clap(long, help = "Run test only")]
    pub test: bool,

    #[clap(long, help = "Simulate with fallback")]
    pub with_fallback: bool,

    #[clap(long, default_value = "10000000")]
    pub amount_in: u64,

    #[clap(
        long,
        default_value = "0x60781C2586D68229fde47564546784ab3fACA982,0xd586E7F844cEa2F87f50152665BCbc2C279D8d70,0x1f1E7c893855525b303f99bDF5c3c05Be09ca251,0xA389f9430876455C36478DeEa9769B7Ca4E3DDB1"
    )]
    pub path: String,

    #[clap(long, help = "Delete objects before simulation")]
    pub delete_objects: Option<String>,
}

fn supported_protocols() -> Vec<Protocol> {
    vec![
        Protocol::TraderJoe,
        Protocol::Pangolin,
        Protocol::SushiSwap,
        Protocol::UniswapV3,
        Protocol::Curve,
    ]
}

/// Write all pool and related contract addresses to the `args.result_path`.
pub async fn run(args: Args) -> Result<()> {
    mev_logger::init_console_logger_with_directives(
        Some(LevelFilter::INFO),
        &[
            "arb=debug",
            // "dex_indexer=warn",
            // "simulator=trace",
        ],
    );
    if args.test {
        return test_pool_related_objects(args).await;
    }

    let result_path = args.result_path;
    let rpc_url = args.http_config.rpc_url;

    let dex_indexer = DexIndexer::new(&rpc_url).await?;
    let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_default_slow().await);

    let _ = fs::remove_file(&result_path);
    let file = File::create(&result_path)?;
    let mut writer = BufWriter::new(file);

    // load existing addresses
    let mut addresses: HashSet<String> = fs::read_to_string(&result_path)?
        .lines()
        .map(|line| line.to_string())
        .collect();

    // add new addresses
    for protocol in supported_protocols() {
        // protocol related addresses
        addresses.extend(protocol.related_contract_addresses().await?);

        // pool related addresses
        for pool in dex_indexer.get_all_pools(&protocol)? {
            addresses.extend(pool.related_contract_addresses(simulator.clone()).await);
        }
    }

    addresses.extend(global_addresses());

    let all_addresses: Vec<String> = addresses.into_iter().collect();
    writeln!(writer, "{}", all_addresses.join("\n"))?;

    writer.flush()?;

    info!("🎉 write pool and related contract addresses to {}", result_path);

    Ok(())
}

fn global_addresses() -> HashSet<String> {
    // AVAX native token and system addresses
    let mut result = HashSet::new();
    
    // WAVAX (Wrapped AVAX) - most important for swaps
    result.insert("0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7".to_string());
    
    // USDC.e
    result.insert("0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664".to_string());
    
    // USDC
    result.insert("0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E".to_string());
    
    // USDT.e  
    result.insert("0xc7198437980c041c805A1EDcbA50c1Ce5db95118".to_string());
    
    // USDT
    result.insert("0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7".to_string());
    
    // DAI.e
    result.insert("0xd586E7F844cEa2F87f50152665BCbc2C279D8d70".to_string());
    
    // WETH.e
    result.insert("0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB".to_string());
    
    // WBTC.e
    result.insert("0x50b7545627a5162F82A992c33b87aDc75187B218".to_string());

    result
}

async fn test_pool_related_objects(args: Args) -> Result<()> {
    // Test Data ==================================
    let sender = Address::from_str("0xac5bceec1b789ff840d7d4e6ce4ce61c90d190a7f8c4f4ddf0bff6ee2413c33c").unwrap();
    let amount_in = args.amount_in;

    let path = args
        .path
        .split(',')
        .map(|addr| Address::from_str(addr).unwrap())
        .collect::<Vec<_>>();

    let with_fallback = args.with_fallback;
    let rpc_url = args.http_config.rpc_url;

    let simulator_pool = Arc::new(ObjectPool::new(1, move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { Box::new(DBSimulator::new_test(with_fallback).await) as Box<dyn Simulator> })
    }));

    let dex_searcher: Arc<dyn DexSearcher> = Arc::new(IndexerDexSearcher::new(&rpc_url, simulator_pool.clone()).await?);
    let path = dex_searcher.find_test_path(&path).await?;
    info!(?with_fallback, ?amount_in, ?path, ?args.delete_objects, "test data");
    // Test Data ==================================

    let block_number = get_latest_block(&rpc_url).await?;

    // Get all pool-related addresses;
    let mut override_addresses = pool_related_objects(&args.result_path).await?;
    if let Some(delete_objects) = args.delete_objects {
        let delete_addresses = delete_objects
            .split(',')
            .map(|addr| Address::from_str(addr).unwrap())
            .collect::<Vec<_>>();
        override_addresses.retain(|addr| !delete_addresses.contains(addr));
    }

    let sim_ctx = SimulateCtx::new(block_number, override_addresses);

    let trader = Trader::new(simulator_pool).await?;
    let result = trader
        .get_trade_result(&path, sender, amount_in, TradeType::Flashloan, vec![], sim_ctx)
        .await?;
    info!(?result, "trade result");

    Ok(())
}

async fn pool_related_objects(file_path: &str) -> Result<Vec<Address>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut res = vec![];
    for line in reader.lines() {
        let line = line?;
        if let Ok(address) = Address::from_str(&line) {
            res.push(address);
        }
    }

    Ok(res)
}
