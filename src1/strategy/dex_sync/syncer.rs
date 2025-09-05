//! DEX数据同步器
//! 
//! 负责从链上同步DEX池数据，监听事件并保持状态最新

use super::{pool_manager::PoolManager, types::*};
use crate::core::{
    collectors::{Event, EventHandler},
    types::{Result, BotError},
};
use async_trait::async_trait;
use ethers::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

/// DEX数据同步器
pub struct DexDataSyncer {
    /// HTTP RPC客户端
    rpc_client: Arc<Provider<Http>>,
    /// 池管理器
    pool_manager: Arc<PoolManager>,
    /// DEX配置
    dex_configs: Vec<DexConfig>,
    /// 同步间隔
    sync_interval: Duration,
    /// 是否运行中
    running: Arc<std::sync::atomic::AtomicBool>,
    /// 已发现的池地址集合
    discovered_pools: Arc<std::sync::RwLock<HashSet<Address>>>,
    /// 批量RPC调用大小
    batch_size: usize,
}

impl DexDataSyncer {
    /// 创建新的数据同步器
    pub fn new(
        rpc_client: Arc<Provider<Http>>,
        pool_manager: Arc<PoolManager>,
        dex_configs: Vec<DexConfig>,
    ) -> Self {
        Self {
            rpc_client,
            pool_manager,
            dex_configs,
            sync_interval: Duration::from_secs(30), // 默认30秒同步一次
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            discovered_pools: Arc::new(std::sync::RwLock::new(HashSet::new())),
            batch_size: 50, // 批量处理50个池
        }
    }
    
    /// 设置同步间隔
    pub fn with_sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = interval;
        self
    }
    
    /// 设置批量处理大小
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }
    
    /// 启动数据同步器
    pub async fn start(&self) -> Result<()> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            warn!("数据同步器已经在运行中");
            return Ok(());
        }
        
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("启动DEX数据同步器");
        
        // 初始化：发现所有池
        self.discover_all_pools().await?;
        
        // 启动池管理器清理任务
        self.pool_manager.start_cleanup_task().await?;
        
        // 启动定期同步任务
        self.start_periodic_sync().await?;
        
        Ok(())
    }
    
    /// 停止数据同步器
    pub fn stop(&self) {
        info!("停止DEX数据同步器");
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// 发现所有DEX的池
    async fn discover_all_pools(&self) -> Result<()> {
        info!("开始发现所有DEX池...");
        let mut total_discovered = 0;
        
        for dex_config in &self.dex_configs {
            if !dex_config.enabled {
                debug!("跳过已禁用的DEX: {}", dex_config.dex_type.name());
                continue;
            }
            
            match self.discover_dex_pools(dex_config).await {
                Ok(count) => {
                    info!("从 {} 发现 {} 个池", dex_config.dex_type.name(), count);
                    total_discovered += count;
                }
                Err(e) => {
                    error!("从 {} 发现池失败: {}", dex_config.dex_type.name(), e);
                }
            }
        }
        
        info!("池发现完成，总计发现 {} 个池", total_discovered);
        Ok(())
    }
    
    /// 发现单个DEX的池
    async fn discover_dex_pools(&self, dex_config: &DexConfig) -> Result<usize> {
        debug!("开始发现 {} 的池", dex_config.dex_type.name());
        
        // TODO: 实现真实的池发现逻辑
        // 这里需要调用工厂合约的 allPairsLength 和 allPairs 方法
        // 然后对每个池调用 getReserves 等方法获取详细信息
        
        // 暂时创建一些示例池用于测试
        let example_pools = self.create_example_pools(dex_config).await?;
        let mut discovered_count = 0;
        
        let current_block = self.rpc_client.get_block_number().await?.as_u64();
        
        for pool in example_pools {
            if let Err(e) = self.pool_manager.add_pool(pool, current_block) {
                warn!("添加池失败: {}", e);
            } else {
                discovered_count += 1;
            }
        }
        
        Ok(discovered_count)
    }
    
    /// 创建示例池(临时实现)
    async fn create_example_pools(&self, dex_config: &DexConfig) -> Result<Vec<Pool>> {
        let mut pools = Vec::new();
        
        // 创建WAVAX/USDC池示例
        let wavax = Token::wavax();
        let usdc = Token::new(
            "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E".parse().unwrap(),
            "USDC".to_string(),
            "USD Coin".to_string(),
            6,
        );
        
        // 构造池地址(实际应该从工厂合约查询)
        let pool_address = match dex_config.dex_type {
            DexType::TraderJoe => "0x1111111111111111111111111111111111111111".parse().unwrap(),
            DexType::Pangolin => "0x2222222222222222222222222222222222222222".parse().unwrap(),
            DexType::SushiSwap => "0x3333333333333333333333333333333333333333".parse().unwrap(),
            DexType::Unknown => return Err(BotError::Configuration("Unknown DEX type".to_string())),
        };
        
        let pool = Pool::new(
            pool_address,
            dex_config.dex_type,
            wavax,
            usdc,
            U256::from_dec_str("1000000000000000000000").map_err(|e| BotError::Configuration(format!("Parse error: {}", e)))?, // 1000 WAVAX
            U256::from_dec_str("50000000000").map_err(|e| BotError::Configuration(format!("Parse error: {}", e)))?,           // 50000 USDC
            dex_config.fee_bps,
            0,
        );
        
        pools.push(pool);
        Ok(pools)
    }
    
    /// 启动定期同步任务
    async fn start_periodic_sync(&self) -> Result<()> {
        let pool_manager = self.pool_manager.clone();
        let running = self.running.clone();
        let sync_interval = self.sync_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = interval(sync_interval);
            
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                interval_timer.tick().await;
                
                // 刷新所有池状态
                match pool_manager.refresh_all_pools().await {
                    Ok(refreshed) => {
                        debug!("定期同步完成，刷新了 {} 个池", refreshed);
                    }
                    Err(e) => {
                        error!("定期同步失败: {}", e);
                    }
                }
                
                // 清理过期状态
                let cleaned = pool_manager.cleanup_stale_pools();
                if cleaned > 0 {
                    debug!("清理了 {} 个过期池状态", cleaned);
                }
                
                // 打印状态信息
                pool_manager.print_status();
            }
            
            info!("定期同步任务已停止");
        });
        
        info!("启动定期同步任务，间隔: {:?}", sync_interval);
        Ok(())
    }
    
    /// 处理新区块事件
    pub async fn handle_new_block(&self, block_number: u64, _timestamp: u64) -> Result<()> {
        debug!("处理新区块: {}", block_number);
        
        // 这里可以添加基于区块的同步逻辑
        // 比如定期刷新池状态，监听事件等
        
        Ok(())
    }
    
    /// 处理新交易事件 
    pub async fn handle_new_transaction(&self, tx_hash: &str, to: Option<&str>, data: Option<&str>) -> Result<()> {
        // 检查是否为DEX相关交易
        if let Some(to_addr) = to {
            if let Ok(address) = to_addr.parse::<Address>() {
                // 检查是否为已知的DEX路由器
                for dex_config in &self.dex_configs {
                    if dex_config.router_address == address {
                        debug!("发现DEX交易: {} -> {}", tx_hash, dex_config.dex_type.name());
                        
                        // 分析交易数据
                        if let Some(call_data) = data {
                            self.analyze_dex_transaction(tx_hash, dex_config, call_data).await?;
                        }
                        
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 分析DEX交易
    async fn analyze_dex_transaction(&self, _tx_hash: &str, _dex_config: &DexConfig, _call_data: &str) -> Result<()> {
        // TODO: 实现交易数据解析
        // 1. 解析函数调用
        // 2. 提取交换参数
        // 3. 预测对池状态的影响
        // 4. 更新相关池的状态
        
        debug!("分析DEX交易数据");
        Ok(())
    }
    
    /// 批量更新池储备量
    pub async fn batch_update_reserves(&self, pool_addresses: &[Address]) -> Result<usize> {
        if pool_addresses.is_empty() {
            return Ok(0);
        }
        
        let current_block = self.rpc_client.get_block_number().await?.as_u64();
        let mut updated_count = 0;
        
        // 分批处理
        for chunk in pool_addresses.chunks(self.batch_size) {
            match timeout(
                Duration::from_secs(30),
                self.update_pools_batch(chunk, current_block)
            ).await {
                Ok(Ok(count)) => {
                    updated_count += count;
                }
                Ok(Err(e)) => {
                    error!("批量更新池失败: {}", e);
                }
                Err(_) => {
                    error!("批量更新池超时");
                }
            }
        }
        
        info!("批量更新完成，更新了 {} 个池", updated_count);
        Ok(updated_count)
    }
    
    /// 更新单批池
    async fn update_pools_batch(&self, pool_addresses: &[Address], block_number: u64) -> Result<usize> {
        // TODO: 实现真实的批量池状态查询
        // 使用 Multicall 合约可以在单个调用中查询多个池的状态
        
        let mut updated_count = 0;
        
        for &pool_address in pool_addresses {
            // 模拟池状态更新
            if let Err(e) = self.pool_manager.update_pool_reserves(
                pool_address,
                U256::from(1000000), // 示例储备量
                U256::from(2000000), // 示例储备量
                block_number,
            ) {
                debug!("更新池 {} 失败: {}", pool_address, e);
            } else {
                updated_count += 1;
            }
        }
        
        Ok(updated_count)
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> SyncStats {
        self.pool_manager.get_stats()
    }
    
    /// 检查是否运行中
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// 获取已发现的池总数
    pub fn get_discovered_pool_count(&self) -> usize {
        self.discovered_pools.read().unwrap().len()
    }
}

/// 事件处理器实现
#[async_trait]
impl EventHandler for DexDataSyncer {
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::NewBlock { block_number, timestamp, .. } => {
                self.handle_new_block(block_number, timestamp).await?;
            }
            Event::NewTransaction { hash, to, data, .. } => {
                let to_ref = to.as_deref();
                let data_ref = data.as_deref();
                self.handle_new_transaction(&hash, to_ref, data_ref).await?;
            }
            Event::System(system_event) => {
                debug!("收到系统事件: {:?}", system_event);
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "DexDataSyncer"
    }
}

impl Clone for DexDataSyncer {
    fn clone(&self) -> Self {
        Self {
            rpc_client: self.rpc_client.clone(),
            pool_manager: self.pool_manager.clone(),
            dex_configs: self.dex_configs.clone(),
            sync_interval: self.sync_interval,
            running: self.running.clone(),
            discovered_pools: self.discovered_pools.clone(),
            batch_size: self.batch_size,
        }
    }
}
