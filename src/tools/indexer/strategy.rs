use std::sync::Arc;

use crate::infra::{async_trait, ActionSubmitter, Strategy};
use eyre::Result;
use ethers::{
    providers::{Http, Provider},
    types::BlockNumber,
};
use tokio::task::JoinSet;
use tracing::{debug, error, info};

use crate::{
    supported_protocols, token01_key,
    types::{Event, NoAction, PoolCache, Protocol},
    DB,
};

#[derive(Clone)]
pub struct PoolCreatedStrategy {
    pool_cache: PoolCache,
    db: Arc<dyn DB>,
    provider: Arc<Provider<Http>>,
}

impl PoolCreatedStrategy {
    pub fn new(db: Arc<dyn DB>, http_url: &str, pool_cache: PoolCache) -> Result<Self> {
        let provider = Arc::new(Provider::<Http>::try_from(http_url)?);
        
        Ok(Self { 
            pool_cache, 
            db, 
            provider,
        })
    }

    pub async fn backfill_pools(&self) -> Result<()> {
        let mut joinset = JoinSet::new();
        let processed_blocks = self.db.get_processed_blocks()?;
        
        for protocol in supported_protocols() {
            let (provider, db) = (self.provider.clone(), self.db.clone());
            let pool_cache = self.pool_cache.clone();
            let start_block = processed_blocks.get(&protocol).cloned().flatten();

            joinset.spawn(async move { 
                backfill_pools_for_protocol(provider, db, protocol, start_block, pool_cache).await 
            });
        }

        while let Some(res) = joinset.join_next().await {
            if let Err(e) = res {
                error!("backfill_pools error: {:?}", e);
            }
        }

        info!("backfill_pools done");
        Ok(())
    }
}

#[async_trait]
impl Strategy<Event, NoAction> for PoolCreatedStrategy {
    fn name(&self) -> &str {
        "PoolCreatedStrategy"
    }

    async fn sync_state(&mut self, _submitter: Arc<dyn ActionSubmitter<NoAction>>) -> Result<()> {
        self.backfill_pools().await
    }

    async fn process_event(&mut self, _event: Event, _: Arc<dyn ActionSubmitter<NoAction>>) {
        if let Err(error) = self.backfill_pools().await {
            error!("backfill_pools error: {:?}", error);
        }
    }
}

async fn backfill_pools_for_protocol(
    provider: Arc<Provider<Http>>,
    db: Arc<dyn DB>,
    protocol: Protocol,
    start_block: Option<BlockNumber>,
    pool_cache: PoolCache,
) -> Result<()> {
    debug!(%protocol, ?start_block, "starting backfill for protocol");
    
    // Get current block number
    let current_block = provider.get_block_number().await?;
    let from_block = start_block.unwrap_or(BlockNumber::from(0u64));
    
    if from_block >= current_block {
        debug!(%protocol, "no new blocks to process");
        return Ok(());
    }
    
    // Process blocks in chunks to avoid RPC limits
    const CHUNK_SIZE: u64 = 1000;
    let from_block_u64: u64 = from_block.into();
    let current_block_u64: u64 = current_block.into();
    
    let PoolCache {
        token_pools,
        token01_pools,
        pool_map,
    } = pool_cache;

    for chunk_start in (from_block_u64..=current_block_u64).step_by(CHUNK_SIZE as usize) {
        let chunk_end = std::cmp::min(chunk_start + CHUNK_SIZE - 1, current_block_u64);
        
        debug!(%protocol, chunk_start, chunk_end, "processing block chunk");
        
        // Get protocol-specific contract addresses and event signatures
        let contract_addresses = protocol.related_contract_addresses().await?;
        
        // Query events for this protocol in this block range
        // Note: This is a simplified implementation. In practice, you would need to:
        // 1. Query logs for specific event signatures (e.g., PairCreated events)
        // 2. Parse the logs to extract pool information
        // 3. Create Pool structs from the parsed data
        
        let mut pools = vec![];
        
        // For each contract address, query relevant events
        for contract_address in contract_addresses {
            // This is where you would implement the actual event querying logic
            // using ethers filters and log parsing
            debug!(%protocol, contract_address, "querying events for contract");
            
            // Placeholder: In a real implementation, you would:
            // 1. Create event filters
            // 2. Query logs
            // 3. Parse events to Pool structs
            // 4. Add pools to the pools vector
        }
        
        if !pools.is_empty() {
            // Update caches
            for pool in &pools {
                // token_pools
                for token in &pool.tokens {
                    let key = token.token_address.clone();
                    token_pools.entry(key).or_default().insert(pool.clone());
                }
                // token01_pools
                for (token0_address, token1_address) in pool.token01_pairs() {
                    let key = token01_key(&token0_address, &token1_address);
                    token01_pools.entry(key).or_default().insert(pool.clone());
                }
                // pool_map
                pool_map.insert(pool.pool, pool.clone());
            }
            
            debug!("{}: {} pools found in blocks {}..{}", protocol, pools.len(), chunk_start, chunk_end);
        }
        
        // Flush to database
        let last_block = Some(BlockNumber::from(chunk_end));
        db.flush(&protocol, &pools, last_block)?;
    }

    info!(
        "{}: backfill complete, pool_count = {}",
        protocol,
        db.pool_count(&protocol).unwrap_or(0)
    );

    Ok(())
}
