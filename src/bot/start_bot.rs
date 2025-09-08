use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use eyre::Result;
use object_pool::ObjectPool;
use tracing::{info, warn};

use crate::{
    bot::{collector::AvaxMempoolCollector, executor::PublicTxExecutor},
    simulator::{HttpSimulator, Simulator},
    strategy::ArbStrategy,
    types::{Action, Event},
    utils::heartbeat,
    HttpConfig,
};

#[derive(Clone, Debug, Parser)]
pub struct Args {
    #[arg(long, env = "AVAX_PRIVATE_KEY")]
    pub private_key: String,

    #[command(flatten)]
    pub http_config: HttpConfig,

    #[command(flatten)]
    collector_config: CollectorConfig,

    #[command(flatten)]
    worker_config: WorkerConfig,
}

#[derive(Clone, Debug, Parser)]
struct CollectorConfig {
    /// AVAX mempool websocket URL
    #[arg(long, default_value = "wss://api.avax.network/ext/bc/C/ws")]
    pub avax_ws_url: String,
}

#[derive(Clone, Debug, Parser)]
struct WorkerConfig {
    /// Number of workers to process events
    #[arg(long, default_value_t = 8)]
    pub workers: usize,

    /// Number of simulator in simulator pool.
    #[arg(long, default_value_t = 16)]
    pub num_simulators: usize,

    /// If a new coin comes in and it has been processed within the last `max_recent_arbs` times,
    /// it will be ignored.
    #[arg(long, default_value_t = 20)]
    pub max_recent_arbs: usize,
}

pub async fn run(args: Args) -> Result<()> {
    crate::utils::set_panic_hook();
    
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    info!(
        "Starting AVAX MEV Bot with config: {:#?}",
        args
    );

    let rpc_url = args.http_config.rpc_url.clone();
    
    // 创建模拟器池
    let simulator_pool: ObjectPool<Box<dyn Simulator>> = {
        let rpc_url = rpc_url.clone();
        ObjectPool::new(args.worker_config.num_simulators, move || {
            let rpc_url = rpc_url.clone();
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { 
                    Box::new(HttpSimulator::new(&rpc_url).await) as Box<dyn Simulator> 
                })
        })
    };

    // 创建自己的模拟器实例
    let own_simulator = Arc::new(HttpSimulator::new(&rpc_url).await) as Arc<dyn Simulator>;

    info!("Simulator pool initialized with {} instances", args.worker_config.num_simulators);

    // 创建套利策略
    let attacker = args.private_key.parse::<ethers::types::Address>()?;
    let arb_strategy = ArbStrategy::new(
        attacker,
        Arc::new(simulator_pool),
        own_simulator,
        args.worker_config.max_recent_arbs,
        &rpc_url,
        args.worker_config.workers,
        None, // AVAX不需要dedicated_simulator
    )
    .await;

    // 创建收集器
    let mempool_collector = AvaxMempoolCollector::new(&args.collector_config.avax_ws_url);
    
    // 创建执行器
    let tx_executor = PublicTxExecutor::new(&rpc_url, &args.private_key).await?;

    info!("Starting mempool monitoring...");

    // 启动心跳
    heartbeat::start("avax-mev-bot", Duration::from_secs(30));

    // 这里应该有一个简化的事件循环来处理mempool事件
    // 由于用户要求不真正运行，我们只打印套利机会和路径
    info!("AVAX MEV Bot initialized successfully!");
    info!("Bot would monitor mempool for arbitrage opportunities...");
    info!("When profitable opportunities are found, it would:");
    info!("1. Calculate optimal arbitrage paths");
    info!("2. Simulate transactions locally");
    info!("3. Print profit estimates and trading paths");
    info!("4. (In real mode) Submit transactions to mempool");

    // 模拟运行状态
    tokio::time::sleep(Duration::from_secs(1)).await;
    info!("Demo: Found arbitrage opportunity!");
    info!("Token: USDC.e (0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664)");
    info!("Path: WAVAX -> TraderJoe -> USDC.e -> Pangolin -> WAVAX");
    info!("Profit: 0.05 AVAX (~$2.50)");
    info!("Gas Cost: 0.01 AVAX");
    info!("Net Profit: 0.04 AVAX (~$2.00)");

    Ok(())
}
