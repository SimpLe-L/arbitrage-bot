//! 区块收集器
//! 
//! 监听新区块事件，具备重连和错误处理机制

use super::{Collector, Event, EventStream, SystemEvent};
use crate::core::types::{Result, BotError};
use async_trait::async_trait;
use ethers::prelude::*;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// 区块收集器
pub struct BlockCollector {
    ws_url: String,
    chain_id: u64,
    provider: Arc<Mutex<Option<Provider<Ws>>>>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
}

impl BlockCollector {
    /// 创建新的区块收集器
    pub async fn new(ws_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Ws>::connect(ws_url).await
            .map_err(|e| BotError::Connection(format!("Failed to connect to WebSocket: {}", e)))?;
        
        Ok(Self {
            ws_url: ws_url.to_string(),
            chain_id,
            provider: Arc::new(Mutex::new(Some(provider))),
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(5),
        })
    }
    
    /// 设置最大重连次数
    pub fn with_max_reconnect_attempts(mut self, max_attempts: u32) -> Self {
        self.max_reconnect_attempts = max_attempts;
        self
    }
    
    /// 设置重连延迟
    pub fn with_reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }
    
    /// 尝试重连
    async fn try_reconnect(&mut self) -> Result<()> {
        if self.reconnect_attempts >= self.max_reconnect_attempts {
            return Err(BotError::Connection(format!(
                "Maximum reconnect attempts ({}) exceeded",
                self.max_reconnect_attempts
            )));
        }
        
        self.reconnect_attempts += 1;
        warn!("尝试重连 ({}/{})", self.reconnect_attempts, self.max_reconnect_attempts);
        
        sleep(self.reconnect_delay).await;
        
        match Provider::<Ws>::connect(&self.ws_url).await {
            Ok(provider) => {
                *self.provider.lock().await = Some(provider);
                self.reconnect_attempts = 0;
                info!("重连成功");
                Ok(())
            }
            Err(e) => {
                error!("重连失败: {}", e);
                Err(BotError::Connection(format!("Reconnection failed: {}", e)))
            }
        }
    }
    
    /// 获取提供者连接
    async fn get_provider(&self) -> Option<Provider<Ws>> {
        self.provider.lock().await.clone()
    }
}

#[async_trait]
impl Collector for BlockCollector {
    fn name(&self) -> &str {
        "BlockCollector"
    }
    
    async fn get_event_stream(&self) -> Result<EventStream> {
        let provider_arc = self.provider.clone();
        let ws_url = self.ws_url.clone();
        let chain_id = self.chain_id;
        let max_attempts = self.max_reconnect_attempts;
        let reconnect_delay = self.reconnect_delay;
        
        let stream = async_stream::stream! {
            info!("开始监听链 {} 的区块事件", chain_id);
            let mut reconnect_attempts = 0u32;
            
            loop {
                // 获取当前连接
                let provider = match provider_arc.lock().await.clone() {
                    Some(p) => p,
                    None => {
                        error!("No provider available");
                        yield Event::System(SystemEvent::Error("Provider not available".to_string()));
                        break;
                    }
                };
                
                // 订阅新区块
                let mut block_stream = match provider.subscribe_blocks().await {
                    Ok(stream) => {
                        info!("成功订阅区块事件");
                        reconnect_attempts = 0; // 重置重连计数
                        yield Event::System(SystemEvent::Connected);
                        stream
                    }
                    Err(e) => {
                        error!("订阅区块失败: {}", e);
                        yield Event::System(SystemEvent::Error(format!("Block subscription failed: {}", e)));
                        
                        // 尝试重连
                        if reconnect_attempts < max_attempts {
                            reconnect_attempts += 1;
                            warn!("尝试重连 ({}/{})", reconnect_attempts, max_attempts);
                            sleep(reconnect_delay).await;
                            
                            match Provider::<Ws>::connect(&ws_url).await {
                                Ok(new_provider) => {
                                    *provider_arc.lock().await = Some(new_provider);
                                    info!("重连成功");
                                    continue;
                                }
                                Err(e) => {
                                    error!("重连失败: {}", e);
                                    continue;
                                }
                            }
                        } else {
                            error!("超过最大重连次数，停止尝试");
                            break;
                        }
                    }
                };
                
                // 处理区块事件
                while let Some(block) = block_stream.next().await {
                    let block_number = block.number.unwrap_or_default().as_u64();
                    debug!("收到新区块: {}", block_number);
                    
                    yield Event::NewBlock {
                        block_number,
                        block_hash: format!("{:?}", block.hash.unwrap_or_default()),
                        timestamp: block.timestamp.as_u64(),
                    };
                }
                
                warn!("区块流断开，尝试重连");
                yield Event::System(SystemEvent::Disconnected);
                
                // 重连逻辑
                if reconnect_attempts < max_attempts {
                    reconnect_attempts += 1;
                    sleep(reconnect_delay).await;
                    
                    match Provider::<Ws>::connect(&ws_url).await {
                        Ok(new_provider) => {
                            *provider_arc.lock().await = Some(new_provider);
                            info!("重连成功");
                            continue;
                        }
                        Err(e) => {
                            error!("重连失败: {}", e);
                        }
                    }
                } else {
                    error!("超过最大重连次数");
                    break;
                }
            }
            
            yield Event::System(SystemEvent::Shutdown);
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
