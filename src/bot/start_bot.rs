use std::{
    sync::Arc,
    time::Duration,
};

use clap::Parser;
use eyre::Result;
use object_pool::ObjectPool;
use tracing::{info, warn};

use crate::{
    bot::{collector::AvaxMempoolCollector, executor::EnhancedArbExecutor},
    simulator::{HttpSimulator, Simulator},
    strategy::{
        ArbStrategy,
        transaction_analyzer::TransactionAnalyzer,
        arbitrage_analyzer::ArbitrageAnalyzer,
    },
    types::{Action, Event},
    utils::heartbeat,
    HttpConfig,
};

use ethers::types::Address;

#[derive(Clone, Debug, Parser)]
pub struct Args {
    #[arg(long, env = "AVAX_PRIVATE_KEY")]
    pub private_key: String,

    #[arg(long, env = "ARB_CONTRACT_ADDRESS")]
    pub contract_address: Option<String>,

    #[command(flatten)]
    pub http_config: HttpConfig,

    #[command(flatten)]
    worker_config: WorkerConfig,
}

#[derive(Clone, Debug, Parser)]
struct WorkerConfig {
    /// Number of workers to process events
    #[arg(long, env = "WORKER_THREADS", default_value_t = 8)]
    pub workers: usize,

    /// Number of simulator in simulator pool.
    #[arg(long, env = "SIMULATOR_POOL_SIZE", default_value_t = 16)]
    pub num_simulators: usize,

    /// If a new coin comes in and it has been processed within the last `max_recent_arbs` times,
    /// it will be ignored.
    #[arg(long, env = "MAX_RECENT_ARBS", default_value_t = 20)]
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
    let mempool_collector = AvaxMempoolCollector::new(&args.http_config.ws_url);
    
    // 创建执行器
    let contract_address = args.contract_address.as_deref().map(|s| s.parse()).transpose()?;
    let tx_executor = EnhancedArbExecutor::new(&rpc_url, &args.private_key, contract_address).await?;

    info!("Starting mempool monitoring...");

    // 启动心跳
    heartbeat::start("avax-mev-bot", Duration::from_secs(30));

    info!("AVAX MEV Bot initialized successfully!");
    info!("Starting event processing loop...");

    // 创建分析器
    let transaction_analyzer = TransactionAnalyzer::new();
    let arbitrage_analyzer = ArbitrageAnalyzer::new();

    // 创建事件处理循环
    use crate::engine::Collector;
    use futures::StreamExt;
    
    let mut event_stream = mempool_collector.get_event_stream().await?;
    
    info!("Monitoring mempool for arbitrage opportunities...");
    
    while let Some(event) = event_stream.next().await {
        match event {
            Event::PendingTx(tx) => {
                // 使用交易分析器提取代币信息
                if let Some(token_address) = transaction_analyzer.extract_token_from_tx(&tx) {
                    info!("Processing pending tx: {:?}, token: {:?}", tx.hash, token_address);
                    
                    // 使用套利分析器寻找套利机会
                    match arbitrage_analyzer.find_arbitrage_opportunity(
                        &arb_strategy, 
                        &token_address, 
                        attacker,
                        &rpc_url
                    ).await {
                        Ok(Some(opportunity)) => {
                            // 使用新的详细显示方法
                            opportunity.display();
                            
                            // 在实际部署中，这里会执行套利交易
                            // tx_executor.execute(opportunity.tx_data).await?;
                            info!("Arbitrage opportunity logged (execution disabled in demo mode)");
                        },
                        Ok(None) => {
                            // 没有发现套利机会，这是正常的
                        },
                        Err(e) => {
                            warn!("Error analyzing arbitrage opportunity: {}", e);
                        }
                    }
                }
            },
            _ => {
                // 处理其他类型的事件
            }
        }
    }

    Ok(())
}

