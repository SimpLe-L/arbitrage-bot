//! 流动性池管理器
//! 
//! 负责管理所有DEX池的状态，提供高效的查询和更新接口

use super::types::*;
use crate::core::types::{Result, BotError};
use ethers::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// 池管理器
pub struct PoolManager {
    /// 池状态缓存
    pools: Arc<RwLock<PoolCache>>,
    /// 代币信息缓存
    tokens: Arc<RwLock<TokenCache>>,
    /// 代币对到池地址的映射
    token_pair_pools: Arc<RwLock<TokenPairPools>>,
    /// DEX到池地址的映射
    dex_pools: Arc<RwLock<DexPools>>,
    /// 统计信息
    stats: Arc<RwLock<SyncStats>>,
    /// 过期时间阈值(秒)
    stale_threshold_secs: u64,
    /// HTTP客户端
    rpc_client: Arc<Provider<Http>>,
}

impl PoolManager {
    /// 创建新的池管理器
    pub fn new(rpc_client: Arc<Provider<Http>>) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
            token_pair_pools: Arc::new(RwLock::new(HashMap::new())),
            dex_pools: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SyncStats::default())),
            stale_threshold_secs: 300, // 5分钟过期
            rpc_client,
        }
    }
    
    /// 设置过期时间阈值
    pub fn with_stale_threshold(mut self, threshold_secs: u64) -> Self {
        self.stale_threshold_secs = threshold_secs;
        self
    }
    
    /// 添加池到管理器
    pub fn add_pool(&self, pool: Pool, block_number: u64) -> Result<()> {
        let pool_address = pool.address;
        let token_pair = pool.get_token_pair();
        let dex = pool.dex;
        
        // 添加代币信息到缓存
        {
            let mut tokens = self.tokens.write().unwrap();
            tokens.insert(pool.token0.address, pool.token0.clone());
            tokens.insert(pool.token1.address, pool.token1.clone());
        }
        
        // 添加池状态
        {
            let mut pools = self.pools.write().unwrap();
            pools.insert(pool_address, PoolState::new(pool, block_number));
        }
        
        // 更新代币对映射
        {
            let mut token_pair_pools = self.token_pair_pools.write().unwrap();
            token_pair_pools
                .entry(token_pair)
                .or_insert_with(Vec::new)
                .push(pool_address);
        }
        
        // 更新DEX映射
        {
            let mut dex_pools = self.dex_pools.write().unwrap();
            dex_pools
                .entry(dex)
                .or_insert_with(Vec::new)
                .push(pool_address);
        }
        
        // 更新统计信息
        {
            let mut stats = self.stats.write().unwrap();
            stats.pools_tracked = self.pools.read().unwrap().len();
            stats.record_pool_update();
        }
        
        debug!("添加池 {} (DEX: {})", pool_address, dex.name());
        Ok(())
    }
    
    /// 更新池储备量
    pub fn update_pool_reserves(&self, pool_address: Address, reserve0: U256, reserve1: U256, block_number: u64) -> Result<()> {
        let mut pools = self.pools.write().unwrap();
        if let Some(pool_state) = pools.get_mut(&pool_address) {
            pool_state.pool.reserve0 = reserve0;
            pool_state.pool.reserve1 = reserve1;
            pool_state.last_updated = Instant::now();
            pool_state.block_number = block_number;
            
            // 更新统计信息
            drop(pools);
            let mut stats = self.stats.write().unwrap();
            stats.record_pool_update();
            
            debug!("更新池 {} 储备量: {}/{}", pool_address, reserve0, reserve1);
            Ok(())
        } else {
            Err(BotError::NotFound(format!("Pool {} not found", pool_address)))
        }
    }
    
    /// 处理Swap事件
    pub fn handle_swap_event(&self, event: SwapEvent) -> Result<()> {
        let (delta0, delta1) = event.calculate_reserve_changes();
        
        let mut pools = self.pools.write().unwrap();
        if let Some(pool_state) = pools.get_mut(&event.pool_address) {
            // 更新储备量
            let new_reserve0 = if delta0 >= 0 {
                pool_state.pool.reserve0 + U256::from(delta0 as u128)
            } else {
                pool_state.pool.reserve0 - U256::from((-delta0) as u128)
            };
            
            let new_reserve1 = if delta1 >= 0 {
                pool_state.pool.reserve1 + U256::from(delta1 as u128)
            } else {
                pool_state.pool.reserve1 - U256::from((-delta1) as u128)
            };
            
            pool_state.pool.reserve0 = new_reserve0;
            pool_state.pool.reserve1 = new_reserve1;
            pool_state.last_updated = Instant::now();
            pool_state.block_number = event.block_number;
            
            // 更新统计信息
            drop(pools);
            let mut stats = self.stats.write().unwrap();
            stats.record_swap_event();
            
            debug!("处理Swap事件，池 {} 储备量变化: {}/{}", 
                event.pool_address, delta0, delta1);
            Ok(())
        } else {
            warn!("Swap事件中的池 {} 不存在", event.pool_address);
            Err(BotError::NotFound(format!("Pool {} not found", event.pool_address)))
        }
    }
    
    /// 处理Sync事件
    pub fn handle_sync_event(&self, event: SyncEvent) -> Result<()> {
        self.update_pool_reserves(event.pool_address, event.reserve0, event.reserve1, event.block_number)?;
        
        // 更新统计信息
        let mut stats = self.stats.write().unwrap();
        stats.record_sync_event();
        
        Ok(())
    }
    
    /// 获取池状态
    pub fn get_pool(&self, pool_address: Address) -> Option<PoolState> {
        self.pools.read().unwrap().get(&pool_address).cloned()
    }
    
    /// 获取有效的池状态(未过期)
    pub fn get_fresh_pool(&self, pool_address: Address) -> Option<PoolState> {
        let pools = self.pools.read().unwrap();
        if let Some(pool_state) = pools.get(&pool_address) {
            if !pool_state.is_stale(self.stale_threshold_secs) {
                Some(pool_state.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// 根据代币对获取所有相关池
    pub fn get_pools_for_token_pair(&self, token0: Address, token1: Address) -> Vec<PoolState> {
        let token_pair_pools = self.token_pair_pools.read().unwrap();
        let pools = self.pools.read().unwrap();
        
        let mut result = Vec::new();
        
        // 检查两种顺序的代币对
        for pair in [(token0, token1), (token1, token0)] {
            if let Some(pool_addresses) = token_pair_pools.get(&pair) {
                for &pool_address in pool_addresses {
                    if let Some(pool_state) = pools.get(&pool_address) {
                        if !pool_state.is_stale(self.stale_threshold_secs) && pool_state.pool.is_valid() {
                            result.push(pool_state.clone());
                        }
                    }
                }
            }
        }
        
        result
    }
    
    /// 获取包含指定代币的所有池
    pub fn get_pools_containing_token(&self, token: Address) -> Vec<PoolState> {
        let pools = self.pools.read().unwrap();
        let mut result = Vec::new();
        
        for pool_state in pools.values() {
            if !pool_state.is_stale(self.stale_threshold_secs) 
                && pool_state.pool.is_valid() 
                && pool_state.pool.contains_token(token) {
                result.push(pool_state.clone());
            }
        }
        
        result
    }
    
    /// 获取指定DEX的所有池
    pub fn get_pools_by_dex(&self, dex: DexType) -> Vec<PoolState> {
        let dex_pools = self.dex_pools.read().unwrap();
        let pools = self.pools.read().unwrap();
        
        let mut result = Vec::new();
        
        if let Some(pool_addresses) = dex_pools.get(&dex) {
            for &pool_address in pool_addresses {
                if let Some(pool_state) = pools.get(&pool_address) {
                    if !pool_state.is_stale(self.stale_threshold_secs) && pool_state.pool.is_valid() {
                        result.push(pool_state.clone());
                    }
                }
            }
        }
        
        result
    }
    
    /// 获取所有有效池
    pub fn get_all_fresh_pools(&self) -> Vec<PoolState> {
        let pools = self.pools.read().unwrap();
        pools
            .values()
            .filter(|pool_state| {
                !pool_state.is_stale(self.stale_threshold_secs) && pool_state.pool.is_valid()
            })
            .cloned()
            .collect()
    }
    
    /// 清理过期池状态
    pub fn cleanup_stale_pools(&self) -> usize {
        let mut pools = self.pools.write().unwrap();
        let initial_count = pools.len();
        
        pools.retain(|_, pool_state| !pool_state.is_stale(self.stale_threshold_secs));
        
        let removed_count = initial_count - pools.len();
        if removed_count > 0 {
            info!("清理了 {} 个过期池状态", removed_count);
            
            // 更新统计信息
            let mut stats = self.stats.write().unwrap();
            stats.pools_tracked = pools.len();
        }
        
        removed_count
    }
    
    /// 获取代币信息
    pub fn get_token(&self, token_address: Address) -> Option<Token> {
        self.tokens.read().unwrap().get(&token_address).cloned()
    }
    
    /// 添加代币信息
    pub fn add_token(&self, token: Token) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(token.address, token);
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> SyncStats {
        self.stats.read().unwrap().clone()
    }
    
    /// 启动定期清理任务
    pub async fn start_cleanup_task(&self) -> Result<()> {
        let pools = self.pools.clone();
        let stats = self.stats.clone();
        let stale_threshold = self.stale_threshold_secs;
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(300)); // 每5分钟清理一次
            
            loop {
                cleanup_interval.tick().await;
                
                let mut pools_guard = pools.write().unwrap();
                let initial_count = pools_guard.len();
                
                pools_guard.retain(|_, pool_state| !pool_state.is_stale(stale_threshold));
                
                let removed_count = initial_count - pools_guard.len();
                if removed_count > 0 {
                    info!("定期清理了 {} 个过期池状态", removed_count);
                    
                    // 更新统计信息
                    let mut stats_guard = stats.write().unwrap();
                    stats_guard.pools_tracked = pools_guard.len();
                }
                
                drop(pools_guard);
            }
        });
        
        info!("启动池管理器定期清理任务");
        Ok(())
    }
    
    /// 刷新所有池的状态
    pub async fn refresh_all_pools(&self) -> Result<usize> {
        let pool_addresses: Vec<Address> = {
            self.pools.read().unwrap().keys().cloned().collect()
        };
        
        let mut refreshed_count = 0;
        let current_block = self.rpc_client.get_block_number().await?;
        
        for pool_address in pool_addresses {
            if let Err(e) = self.refresh_pool(pool_address, current_block.as_u64()).await {
                warn!("刷新池 {} 失败: {}", pool_address, e);
                let mut stats = self.stats.write().unwrap();
                stats.record_error();
            } else {
                refreshed_count += 1;
            }
        }
        
        info!("刷新了 {} 个池状态", refreshed_count);
        Ok(refreshed_count)
    }
    
    /// 刷新单个池的状态
    pub async fn refresh_pool(&self, pool_address: Address, block_number: u64) -> Result<()> {
        // 这里需要调用池合约的getReserves方法
        // 暂时实现一个基础版本，具体实现需要根据池合约ABI
        
        // TODO: 实现真实的池状态查询
        debug!("刷新池 {} 状态 (区块: {})", pool_address, block_number);
        Ok(())
    }
    
    /// 获取池总数
    pub fn get_pool_count(&self) -> usize {
        self.pools.read().unwrap().len()
    }
    
    /// 获取有效池总数
    pub fn get_fresh_pool_count(&self) -> usize {
        let pools = self.pools.read().unwrap();
        pools
            .values()
            .filter(|pool_state| {
                !pool_state.is_stale(self.stale_threshold_secs) && pool_state.pool.is_valid()
            })
            .count()
    }
    
    /// 获取代币总数
    pub fn get_token_count(&self) -> usize {
        self.tokens.read().unwrap().len()
    }
    
    /// 打印管理器状态
    pub fn print_status(&self) {
        let stats = self.get_stats();
        let pool_count = self.get_pool_count();
        let fresh_count = self.get_fresh_pool_count();
        let token_count = self.get_token_count();
        
        info!("池管理器状态:");
        info!("  池总数: {} (有效: {})", pool_count, fresh_count);
        info!("  代币总数: {}", token_count);
        info!("  更新次数: {}", stats.pools_updated);
        info!("  Sync事件: {}", stats.sync_events_processed);
        info!("  Swap事件: {}", stats.swap_events_processed);
        info!("  错误次数: {}", stats.errors_count);
        if let Some(last_sync) = stats.last_sync_time {
            info!("  最后同步: {:.2}秒前", last_sync.elapsed().as_secs_f64());
        }
    }
}

impl Clone for PoolManager {
    fn clone(&self) -> Self {
        Self {
            pools: self.pools.clone(),
            tokens: self.tokens.clone(),
            token_pair_pools: self.token_pair_pools.clone(),
            dex_pools: self.dex_pools.clone(),
            stats: self.stats.clone(),
            stale_threshold_secs: self.stale_threshold_secs,
            rpc_client: self.rpc_client.clone(),
        }
    }
}
