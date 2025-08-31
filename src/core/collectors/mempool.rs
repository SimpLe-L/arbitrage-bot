//! 内存池收集器
//! 
//! 监听mempool交易事件，参考sui-mev的简洁设计

use super::{Collector, Event, EventStream, SystemEvent};
use crate::core::types::{Result, BotError};
use async_trait::async_trait;
use ethers::prelude::*;
use futures::StreamExt;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// 内存池收集器
pub struct MempoolCollector {
    provider: Arc<Provider<Ws>>,
    chain_id: u64,
    min_gas_price: Option<U256>,
    filter_contracts_only: bool,
}

impl MempoolCollector {
    /// 创建新的内存池收集器
    pub async fn new(ws_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Ws>::connect(ws_url).await
            .map_err(|e| BotError::Connection(format!("Failed to connect to WebSocket: {}", e)))?;
        
        Ok(Self {
            provider: Arc::new(provider),
            chain_id,
            min_gas_price: None,
            filter_contracts_only: false,
        })
    }
    
    /// 设置最小gas价格过滤
    pub fn with_min_gas_price(mut self, min_gas_price: U256) -> Self {
        self.min_gas_price = Some(min_gas_price);
        self
    }
    
    /// 只监听合约调用交易
    pub fn contracts_only(mut self) -> Self {
        self.filter_contracts_only = true;
        self
    }
    
    /// 检查交易是否通过过滤器
    fn passes_filter(&self, tx: &Transaction) -> bool {
        // Gas价格过滤
        if let Some(min_gas_price) = self.min_gas_price {
            if tx.gas_price.unwrap_or_default() < min_gas_price {
                return false;
            }
        }
        
        // 合约调用过滤
        if self.filter_contracts_only {
            if tx.to.is_none() || tx.input.is_empty() {
                return false;
            }
        }
        
        true
    }
}

#[async_trait]
impl Collector for MempoolCollector {
    fn name(&self) -> &str {
        "MempoolCollector"
    }
    
    async fn get_event_stream(&self) -> Result<EventStream> {
        let provider = self.provider.clone();
        let chain_id = self.chain_id;
        let min_gas_price = self.min_gas_price;
        let filter_contracts_only = self.filter_contracts_only;
        
        let stream = async_stream::stream! {
            info!("Starting mempool collection for chain {}", chain_id);
            
            // 订阅pending交易
            let mut tx_stream = match provider.subscribe_pending_txs().await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Failed to subscribe to pending transactions: {}", e);
                    yield Event::System(SystemEvent::Error(format!("Mempool subscription failed: {}", e)));
                    return;
                }
            };
            
            yield Event::System(SystemEvent::Connected);
            
            while let Some(tx_hash) = tx_stream.next().await {
                // 获取完整交易信息
                let tx = match provider.get_transaction(tx_hash).await {
                    Ok(Some(tx)) => tx,
                    Ok(None) => {
                        debug!("Transaction {} not found", tx_hash);
                        continue;
                    }
                    Err(e) => {
                        warn!("Failed to get transaction {}: {}", tx_hash, e);
                        continue;
                    }
                };
                
                // 应用过滤器
                let passes_filter = {
                    // Gas价格过滤
                    if let Some(min_gas_price) = min_gas_price {
                        if tx.gas_price.unwrap_or_default() < min_gas_price {
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } && {
                    // 合约调用过滤
                    if filter_contracts_only {
                        tx.to.is_some() && !tx.input.is_empty()
                    } else {
                        true
                    }
                };
                
                if !passes_filter {
                    continue;
                }
                
                debug!("Received new mempool transaction: {}", tx_hash);
                
                yield Event::NewTransaction {
                    hash: format!("{:?}", tx_hash),
                    from: format!("{:?}", tx.from),
                    to: tx.to.map(|addr| format!("{:?}", addr)),
                    value: tx.value.to_string(),
                    gas_price: tx.gas_price.unwrap_or_default().to_string(),
                    data: if tx.input.is_empty() { None } else { Some(format!("0x{}", hex::encode(&tx.input))) },
                };
            }
            
            yield Event::System(SystemEvent::Disconnected);
        };
        
        Ok(Box::pin(stream))
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("MempoolCollector started for chain {}", self.chain_id);
        if let Some(min_gas_price) = self.min_gas_price {
            info!("Minimum gas price filter: {} wei", min_gas_price);
        }
        if self.filter_contracts_only {
            info!("Contract calls only filter enabled");
        }
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("MempoolCollector stopped");
        Ok(())
    }
}
