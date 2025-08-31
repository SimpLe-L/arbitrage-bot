//! 区块收集器
//! 
//! 监听新区块事件，参考sui-mev的简洁设计

use super::{Collector, Event, EventStream, SystemEvent};
use crate::core::types::{Result, BotError};
use async_trait::async_trait;
use ethers::prelude::*;
use futures::StreamExt;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

/// 区块收集器
pub struct BlockCollector {
    provider: Arc<Provider<Ws>>,
    chain_id: u64,
}

impl BlockCollector {
    /// 创建新的区块收集器
    pub async fn new(ws_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Ws>::connect(ws_url).await
            .map_err(|e| BotError::Connection(format!("Failed to connect to WebSocket: {}", e)))?;
        
        Ok(Self {
            provider: Arc::new(provider),
            chain_id,
        })
    }
}

#[async_trait]
impl Collector for BlockCollector {
    fn name(&self) -> &str {
        "BlockCollector"
    }
    
    async fn get_event_stream(&self) -> Result<EventStream> {
        let provider = self.provider.clone();
        let chain_id = self.chain_id; // 复制chain_id以避免生命周期问题
        
        let stream = async_stream::stream! {
            info!("Starting block collection for chain {}", chain_id);
            
            // 订阅新区块
            let mut block_stream = match provider.subscribe_blocks().await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Failed to subscribe to blocks: {}", e);
                    yield Event::System(SystemEvent::Error(format!("Block subscription failed: {}", e)));
                    return;
                }
            };
            
            yield Event::System(SystemEvent::Connected);
            
            while let Some(block) = block_stream.next().await {
                debug!("Received new block: {}", block.number.unwrap_or_default());
                
                yield Event::NewBlock {
                    block_number: block.number.unwrap_or_default().as_u64(),
                    block_hash: format!("{:?}", block.hash.unwrap_or_default()),
                    timestamp: block.timestamp.as_u64(),
                };
            }
            
            yield Event::System(SystemEvent::Disconnected);
        };
        
        Ok(Box::pin(stream))
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("BlockCollector started for chain {}", self.chain_id);
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("BlockCollector stopped");
        Ok(())
    }
}
