//! 内存池收集器
//! 
//! 监听mempool交易事件，具备智能过滤和重连机制

use super::{Collector, Event, EventStream, SystemEvent};
use crate::core::types::{Result, BotError};
use async_trait::async_trait;
use ethers::prelude::*;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// 内存池收集器
pub struct MempoolCollector {
    ws_url: String,
    chain_id: u64,
    provider: Arc<Mutex<Option<Provider<Ws>>>>,
    min_gas_price: Option<U256>,
    filter_contracts_only: bool,
    target_contracts: HashSet<Address>,
    dex_routers: HashSet<Address>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    min_value_wei: Option<U256>,
}

impl MempoolCollector {
    /// 创建新的内存池收集器
    pub async fn new(ws_url: &str, chain_id: u64) -> Result<Self> {
        let provider = Provider::<Ws>::connect(ws_url).await
            .map_err(|e| BotError::Connection(format!("Failed to connect to WebSocket: {}", e)))?;
        
        // AVAX主要DEX路由器地址
        let mut dex_routers = HashSet::new();
        // Trader Joe
        dex_routers.insert("0x60aE616a2155Ee3d9A68541Ba4544862310933d4".parse().unwrap());
        // Pangolin
        dex_routers.insert("0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106".parse().unwrap());
        // SushiSwap
        dex_routers.insert("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".parse().unwrap());
        
        Ok(Self {
            ws_url: ws_url.to_string(),
            chain_id,
            provider: Arc::new(Mutex::new(Some(provider))),
            min_gas_price: None,
            filter_contracts_only: false,
            target_contracts: HashSet::new(),
            dex_routers,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(5),
            min_value_wei: None,
        })
    }
    
    /// 设置最小gas价格过滤
    pub fn with_min_gas_price(mut self, min_gas_price: U256) -> Self {
        self.min_gas_price = Some(min_gas_price);
        self
    }
    
    /// 设置最小交易价值过滤
    pub fn with_min_value(mut self, min_value: U256) -> Self {
        self.min_value_wei = Some(min_value);
        self
    }
    
    /// 只监听合约调用交易
    pub fn contracts_only(mut self) -> Self {
        self.filter_contracts_only = true;
        self
    }
    
    /// 添加目标合约地址
    pub fn add_target_contract(mut self, address: Address) -> Self {
        self.target_contracts.insert(address);
        self
    }
    
    /// 添加DEX路由器地址
    pub fn add_dex_router(mut self, address: Address) -> Self {
        self.dex_routers.insert(address);
        self
    }
    
    /// 设置重连配置
    pub fn with_reconnect_config(mut self, max_attempts: u32, delay: Duration) -> Self {
        self.max_reconnect_attempts = max_attempts;
        self.reconnect_delay = delay;
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
        
        // 交易价值过滤
        if let Some(min_value) = self.min_value_wei {
            if tx.value < min_value {
                return false;
            }
        }
        
        // 合约调用过滤
        if self.filter_contracts_only {
            if tx.to.is_none() || tx.input.is_empty() {
                return false;
            }
        }
        
        // 目标合约过滤
        if !self.target_contracts.is_empty() {
            if let Some(to) = tx.to {
                if !self.target_contracts.contains(&to) {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        true
    }
    
    /// 检查是否为DEX相关交易
    fn is_dex_transaction(&self, tx: &Transaction) -> bool {
        if let Some(to) = tx.to {
            // 检查是否为已知的DEX路由器
            if self.dex_routers.contains(&to) {
                return true;
            }
            
            // 检查输入数据是否包含swap函数签名
            if !tx.input.is_empty() && tx.input.len() >= 4 {
                let function_selector = &tx.input[0..4];
                // 常见的swap函数选择器
                let swap_selectors = [
                    [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                    [0x8a, 0x04, 0xc5, 0x70], // swapExactTokensForETH
                    [0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
                    [0x18, 0xcb, 0xaf, 0xe5], // swapExactTokensForTokensSupportingFeeOnTransferTokens
                ];
                
                return swap_selectors.iter().any(|&selector| selector == function_selector);
            }
        }
        false
    }
    
    /// 分析交易类型
    fn analyze_transaction_type(&self, tx: &Transaction) -> String {
        if self.is_dex_transaction(tx) {
            return "DEX_SWAP".to_string();
        }
        
        if tx.to.is_none() {
            return "CONTRACT_CREATION".to_string();
        }
        
        if tx.input.is_empty() {
            return "ETH_TRANSFER".to_string();
        }
        
        "CONTRACT_CALL".to_string()
    }
}

#[async_trait]
impl Collector for MempoolCollector {
    fn name(&self) -> &str {
        "MempoolCollector"
    }
    
    async fn get_event_stream(&self) -> Result<EventStream> {
        let provider_arc = self.provider.clone();
        let ws_url = self.ws_url.clone();
        let chain_id = self.chain_id;
        let min_gas_price = self.min_gas_price;
        let min_value_wei = self.min_value_wei;
        let filter_contracts_only = self.filter_contracts_only;
        let target_contracts = self.target_contracts.clone();
        let dex_routers = self.dex_routers.clone();
        let max_attempts = self.max_reconnect_attempts;
        let reconnect_delay = self.reconnect_delay;
        
        let stream = async_stream::stream! {
            info!("开始监听链 {} 的内存池事件", chain_id);
            info!("过滤配置: contracts_only={}, min_gas_price={:?}, target_contracts={}", 
                filter_contracts_only, min_gas_price, target_contracts.len());
            
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
                
                // 订阅pending交易
                let mut tx_stream = match provider.subscribe_pending_txs().await {
                    Ok(stream) => {
                        info!("成功订阅内存池事件");
                        reconnect_attempts = 0; // 重置重连计数
                        yield Event::System(SystemEvent::Connected);
                        stream
                    }
                    Err(e) => {
                        error!("订阅内存池失败: {}", e);
                        yield Event::System(SystemEvent::Error(format!("Mempool subscription failed: {}", e)));
                        
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
                
                // 处理交易事件
                while let Some(tx_hash) = tx_stream.next().await {
                    // 获取完整交易信息
                    let tx = match provider.get_transaction(tx_hash).await {
                        Ok(Some(tx)) => tx,
                        Ok(None) => {
                            debug!("交易 {} 未找到", tx_hash);
                            continue;
                        }
                        Err(e) => {
                            warn!("获取交易 {} 失败: {}", tx_hash, e);
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
                        // 交易价值过滤
                        if let Some(min_value) = min_value_wei {
                            tx.value >= min_value
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
                    } && {
                        // 目标合约过滤
                        if !target_contracts.is_empty() {
                            if let Some(to) = tx.to {
                                target_contracts.contains(&to)
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    };
                    
                    if !passes_filter {
                        continue;
                    }
                    
                    // 分析交易类型
                    let tx_type = {
                        if let Some(to) = tx.to {
                            if dex_routers.contains(&to) {
                                "DEX_SWAP"
                            } else if tx.input.is_empty() {
                                "ETH_TRANSFER"
                            } else {
                                "CONTRACT_CALL"
                            }
                        } else {
                            "CONTRACT_CREATION"
                        }
                    };
                    
                    debug!("收到新内存池交易: {} (类型: {})", tx_hash, tx_type);
                    
                    // 构建事件数据，包含更多信息
                    let mut data_info = if tx.input.is_empty() { 
                        None 
                    } else { 
                        Some(format!("0x{}", hex::encode(&tx.input))) 
                    };
                    
                    // 对DEX交易添加额外分析
                    if tx_type == "DEX_SWAP" && !tx.input.is_empty() && tx.input.len() >= 4 {
                        let function_selector = hex::encode(&tx.input[0..4]);
                        data_info = Some(format!("0x{}|selector:{}", hex::encode(&tx.input), function_selector));
                    }
                    
                    yield Event::NewTransaction {
                        hash: format!("{:?}", tx_hash),
                        from: format!("{:?}", tx.from),
                        to: tx.to.map(|addr| format!("{:?}", addr)),
                        value: tx.value.to_string(),
                        gas_price: tx.gas_price.unwrap_or_default().to_string(),
                        data: data_info,
                    };
                }
                
                warn!("内存池流断开，尝试重连");
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
