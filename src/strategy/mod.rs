mod arb_cache;
mod worker;

use std::{
    collections::{HashSet, VecDeque},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use arb_cache::{ArbCache, ArbItem};
use async_channel::Sender;
use burberry::ActionSubmitter;
use dex_indexer::types::Protocol;
use eyre::{ensure, eyre, Result};
use object_pool::ObjectPool;
use rayon::prelude::*;
use simulator::{ReplaySimulator, SimulateCtx, Simulator};
use ethers::types::{Address, BlockNumber, Log, TransactionReceipt, H256, U64};
use tokio::{
    runtime::{Builder, Handle, RuntimeFlavor},
    task::JoinSet,
};
use tracing::{debug, error, info, instrument, warn};
use worker::Worker;

use crate::{
    arb::Arb,
    common::get_latest_block,
    types::{Action, Event, Source},
};

pub struct ArbStrategy {
    sender: Address,
    arb_item_sender: Option<Sender<ArbItem>>,
    arb_cache: ArbCache,

    recent_arbs: VecDeque<String>,
    max_recent_arbs: usize,

    simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>,
    own_simulator: Arc<dyn Simulator>, // only for execution of pending txs
    rpc_url: String,
    workers: usize,
    current_block: Option<BlockNumber>,
    dedicated_simulator: Option<Arc<ReplaySimulator>>,
}

impl ArbStrategy {
    pub async fn new(
        attacker: Address,
        simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>,
        own_simulator: Arc<dyn Simulator>,
        recent_arbs: usize,
        rpc_url: &str,
        workers: usize,
        dedicated_simulator: Option<Arc<ReplaySimulator>>,
    ) -> Self {
        let current_block = get_latest_block(&rpc_url).await.unwrap();

        Self {
            sender: attacker,
            arb_item_sender: None,
            arb_cache: ArbCache::new(Duration::from_secs(5)),
            recent_arbs: VecDeque::with_capacity(recent_arbs),
            max_recent_arbs: recent_arbs,
            simulator_pool,
            own_simulator,
            rpc_url: rpc_url.to_string(),
            workers,
            current_block: Some(current_block),
            dedicated_simulator,
        }
    }

    #[instrument(name = "on-new-tx-receipt", skip_all, fields(tx = %tx_receipt.transaction_hash))]
    async fn on_new_tx_receipt(&mut self, tx_receipt: TransactionReceipt, logs: Vec<Log>) -> Result<()> {
        let token_pools = self.parse_involved_token_pools(logs).await;
        if token_pools.is_empty() {
            return Ok(());
        }

        let tx_hash = tx_receipt.transaction_hash;
        let block_number = self.get_latest_block().await?;
        let sim_ctx = SimulateCtx::new(block_number, vec![]);

        for (token, pool_address) in token_pools {
            self.arb_cache
                .insert(token, pool_address, tx_hash, sim_ctx.clone(), Source::Public);
        }

        Ok(())
    }

    async fn parse_involved_token_pools(&self, logs: Vec<Log>) -> HashSet<(String, Option<Address>)> {
        let mut join_set = JoinSet::new();

        for log in logs {
            let own_simulator = self.own_simulator.clone();
            join_set.spawn(async move {
                // Parse swap events from logs based on different DEX protocols
                if let Ok(swap_event) = parse_swap_event_from_log(&log, own_simulator).await {
                    return Some((swap_event.involved_token_one_side(), swap_event.pool_address()));
                }
                None
            });
        }

        let mut token_pools = HashSet::new();
        while let Some(result) = join_set.join_next().await {
            if let Ok(Some((token, pool_address))) = result {
                token_pools.insert((token, pool_address));
            }
        }

        token_pools
    }

    async fn get_latest_block(&mut self) -> Result<BlockNumber> {
        if let Some(block) = self.current_block {
            // Check if block is still recent (within 10 blocks)
            let latest = get_latest_block(&self.rpc_url).await?;
            if latest.as_u64().saturating_sub(block.as_u64()) < 10 {
                return Ok(block);
            } else {
                self.current_block = None;
            }
        }

        let block = get_latest_block(&self.rpc_url).await?;
        self.current_block = Some(block);
        Ok(block)
    }
}

async fn parse_swap_event_from_log(log: &Log, simulator: Arc<dyn Simulator>) -> Result<SwapEvent> {
    // This function should parse different DEX swap events based on the log
    // For now, we'll return a placeholder
    // In a real implementation, you'd check the log's address and topics to determine the DEX
    
    // TraderJoe Swap event signature: Swap(address,uint256,uint256,uint256,uint256,address)
    // Pangolin Swap event signature: similar
    // etc.
    
    todo!("Implement swap event parsing from Ethereum logs")
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
        if self.tokens_in[0] != crate::tools::dex::WAVAX_ADDRESS {
            self.tokens_in[0].to_string()
        } else {
            self.tokens_out[0].to_string()
        }
    }
}

#[macro_export]
macro_rules! run_in_tokio {
    ($code:expr) => {
        match Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                RuntimeFlavor::CurrentThread => std::thread::scope(move |s| {
                    s.spawn(move || {
                        Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async move { $code.await })
                    })
                    .join()
                    .unwrap()
                }),
                _ => tokio::task::block_in_place(move || handle.block_on(async move { $code.await })),
            },
            Err(_) => Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move { $code.await }),
        }
    };
}

#[burberry::async_trait]
impl burberry::Strategy<Event, Action> for ArbStrategy {
    fn name(&self) -> &str {
        "ArbStrategy"
    }

    async fn sync_state(&mut self, submitter: Arc<dyn ActionSubmitter<Action>>) -> Result<()> {
        if self.arb_item_sender.is_some() {
            panic!("already synced!");
        }

        let (arb_item_sender, arb_item_receiver) = async_channel::unbounded();
        self.arb_item_sender = Some(arb_item_sender);

        let sender = self.sender;
        let rpc_url = self.rpc_url.clone();

        let workers_to_spawn = self.workers;
        info!("spawning {} workers to process messages", workers_to_spawn);

        let (init_tx, mut init_rx) = tokio::sync::mpsc::channel(workers_to_spawn);

        for id in 0..workers_to_spawn {
            debug!(worker.id = id, "spawning worker...");

            let arb_item_receiver = arb_item_receiver.clone();
            let submitter = submitter.clone();

            let rpc_url = rpc_url.clone();
            let init_tx = init_tx.clone();
            let simulator_pool_arb = self.simulator_pool.clone();
            let simulator_pool_worker = self.simulator_pool.clone();
            let simulator_name = simulator_pool_arb.get().name().to_string();
            let dedicated_simulator = self.dedicated_simulator.clone();

            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 128 MB
                .name(format!("worker-{id}"))
                .spawn(move || {
                    let arb = Arc::new(run_in_tokio!({ Arb::new(&rpc_url, simulator_pool_arb) }).unwrap());

                    // Signal that this worker is initialized
                    run_in_tokio!(init_tx.send(())).unwrap();

                    let worker = Worker {
                        _id: id,
                        sender,
                        arb_item_receiver,
                        simulator_pool: simulator_pool_worker,
                        simulator_name,
                        submitter,
                        arb,
                        dedicated_simulator,
                    };
                    worker.run().unwrap_or_else(|e| panic!("worker {id} panicked: {e:?}"));
                });
        }

        // Wait for all workers to initialize
        for _ in 0..workers_to_spawn {
            init_rx.recv().await.expect("worker initialization failed");
        }

        info!("workers all spawned!");
        Ok(())
    }

    async fn process_event(&mut self, event: Event, _submitter: Arc<dyn ActionSubmitter<Action>>) {
        let result = match event {
            Event::PublicTx(tx_receipt, logs) => self.on_new_tx_receipt(tx_receipt, logs).await,
            // Remove Shio events as they are Sui-specific
            _ => Ok(()),
        };
        if let Err(error) = result {
            error!(?error, "failed to process event");
            return;
        }

        // send arb_item to workers if channel is < 10
        let channel_len = self.arb_item_sender.as_ref().unwrap().len();
        if channel_len < 10 {
            let num_to_send = 10 - channel_len;
            for _ in 0..num_to_send {
                if let Some(item) = self.arb_cache.pop_one() {
                    if !self.recent_arbs.contains(&item.token) {
                        let token = item.token.clone();
                        self.arb_item_sender.as_ref().unwrap().send(item).await.unwrap();

                        self.recent_arbs.push_back(token);
                        if self.recent_arbs.len() > self.max_recent_arbs {
                            self.recent_arbs.pop_front();
                        }
                    }
                } else {
                    // no more arb_item to send
                    break;
                }
            }
        } else {
            warn!("arb_item channel stash {}", channel_len);
        }

        let expired_tokens = self.arb_cache.remove_expired();
        for token in expired_tokens {
            if let Some(pos) = self.recent_arbs.iter().position(|x| x == &token) {
                self.recent_arbs.remove(pos);
            }
        }
    }
}
