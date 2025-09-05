//! DEX数据同步相关类型定义

use ethers::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// DEX类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexType {
    TraderJoe,
    Pangolin,
    SushiSwap,
    Unknown,
}

impl DexType {
    /// 从路由器地址获取DEX类型
    pub fn from_router_address(address: Address) -> Self {
        match format!("{:?}", address).to_lowercase().as_str() {
            "0x60ae616a2155ee3d9a68541ba4544862310933d4" => Self::TraderJoe,
            "0xe54ca86531e17ef3616d22ca28b0d458b6c89106" => Self::Pangolin,
            "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506" => Self::SushiSwap,
            _ => Self::Unknown,
        }
    }
    
    /// 获取DEX名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::TraderJoe => "Trader Joe",
            Self::Pangolin => "Pangolin",
            Self::SushiSwap => "SushiSwap",
            Self::Unknown => "Unknown",
        }
    }
    
    /// 获取默认手续费(基点)
    pub fn default_fee_bps(&self) -> u16 {
        match self {
            Self::TraderJoe => 30,   // 0.3%
            Self::Pangolin => 30,    // 0.3%
            Self::SushiSwap => 30,   // 0.3%
            Self::Unknown => 30,     // 默认0.3%
        }
    }
}

/// 代币信息
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

impl Token {
    pub fn new(address: Address, symbol: String, name: String, decimals: u8) -> Self {
        Self {
            address,
            symbol,
            name,
            decimals,
        }
    }
    
    /// 创建WAVAX代币
    pub fn wavax() -> Self {
        Self {
            address: "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7".parse().unwrap(),
            symbol: "WAVAX".to_string(),
            name: "Wrapped AVAX".to_string(),
            decimals: 18,
        }
    }
    
    /// 检查是否为WAVAX
    pub fn is_wavax(&self) -> bool {
        self.address == "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7".parse().unwrap()
    }
}

/// 流动性池信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub address: Address,
    pub dex: DexType,
    pub token0: Token,
    pub token1: Token,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee_bps: u16,
    pub block_timestamp_last: u64,
}

impl Pool {
    pub fn new(
        address: Address,
        dex: DexType,
        token0: Token,
        token1: Token,
        reserve0: U256,
        reserve1: U256,
        fee_bps: u16,
        block_timestamp_last: u64,
    ) -> Self {
        Self {
            address,
            dex,
            token0,
            token1,
            reserve0,
            reserve1,
            fee_bps,
            block_timestamp_last,
        }
    }
    
    /// 获取代币对
    pub fn get_token_pair(&self) -> (Address, Address) {
        (self.token0.address, self.token1.address)
    }
    
    /// 检查池子是否包含指定代币
    pub fn contains_token(&self, token: Address) -> bool {
        self.token0.address == token || self.token1.address == token
    }
    
    /// 获取另一个代币地址
    pub fn get_other_token(&self, token: Address) -> Option<Address> {
        if self.token0.address == token {
            Some(self.token1.address)
        } else if self.token1.address == token {
            Some(self.token0.address)
        } else {
            None
        }
    }
    
    /// 根据输入代币获取储备量
    pub fn get_reserves(&self, token_in: Address) -> Option<(U256, U256)> {
        if self.token0.address == token_in {
            Some((self.reserve0, self.reserve1))
        } else if self.token1.address == token_in {
            Some((self.reserve1, self.reserve0))
        } else {
            None
        }
    }
    
    /// 检查池子是否有效(储备量大于0)
    pub fn is_valid(&self) -> bool {
        self.reserve0 > U256::zero() && self.reserve1 > U256::zero()
    }
    
    /// 计算价格(token0/token1)
    pub fn get_price(&self) -> f64 {
        if self.reserve1 == U256::zero() {
            return 0.0;
        }
        
        let reserve0_f64 = self.reserve0.as_u128() as f64 / 10_f64.powi(self.token0.decimals as i32);
        let reserve1_f64 = self.reserve1.as_u128() as f64 / 10_f64.powi(self.token1.decimals as i32);
        
        reserve1_f64 / reserve0_f64
    }
}

/// 池状态(带时间戳)
#[derive(Debug, Clone)]
pub struct PoolState {
    pub pool: Pool,
    pub last_updated: Instant,
    pub block_number: u64,
}

impl PoolState {
    pub fn new(pool: Pool, block_number: u64) -> Self {
        Self {
            pool,
            last_updated: Instant::now(),
            block_number,
        }
    }
    
    /// 检查状态是否过期
    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        self.last_updated.elapsed().as_secs() > max_age_secs
    }
    
    /// 更新池状态
    pub fn update(&mut self, new_pool: Pool, block_number: u64) {
        self.pool = new_pool;
        self.last_updated = Instant::now();
        self.block_number = block_number;
    }
}

/// Swap事件数据
#[derive(Debug, Clone)]
pub struct SwapEvent {
    pub pool_address: Address,
    pub sender: Address,
    pub amount0_in: U256,
    pub amount1_in: U256,
    pub amount0_out: U256,
    pub amount1_out: U256,
    pub to: Address,
    pub block_number: u64,
    pub transaction_hash: H256,
}

impl SwapEvent {
    /// 计算此次交换对储备量的影响
    pub fn calculate_reserve_changes(&self) -> (i128, i128) {
        let delta0 = self.amount0_in.as_u128() as i128 - self.amount0_out.as_u128() as i128;
        let delta1 = self.amount1_in.as_u128() as i128 - self.amount1_out.as_u128() as i128;
        (delta0, delta1)
    }
}

/// 同步事件
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub pool_address: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub block_number: u64,
    pub transaction_hash: H256,
}

/// DEX配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub dex_type: DexType,
    pub router_address: Address,
    pub factory_address: Address,
    pub fee_bps: u16,
    pub enabled: bool,
}

impl DexConfig {
    /// 获取AVAX主要DEX配置
    pub fn avax_dexes() -> Vec<Self> {
        vec![
            Self {
                dex_type: DexType::TraderJoe,
                router_address: "0x60aE616a2155Ee3d9A68541Ba4544862310933d4".parse().unwrap(),
                factory_address: "0x9Ad6C38BE94206cA50bb0d90783181662f0Cfa10".parse().unwrap(),
                fee_bps: 30,
                enabled: true,
            },
            Self {
                dex_type: DexType::Pangolin,
                router_address: "0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106".parse().unwrap(),
                factory_address: "0xefa94DE7a4656D787667C749f7E1223D71E9FD88".parse().unwrap(),
                fee_bps: 30,
                enabled: true,
            },
            Self {
                dex_type: DexType::SushiSwap,
                router_address: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".parse().unwrap(),
                factory_address: "0xc35DADB65012eC5796536bD9864eD8773aBc74C4".parse().unwrap(),
                fee_bps: 30,
                enabled: true,
            },
        ]
    }
}

/// 数据同步统计信息
#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    pub pools_tracked: usize,
    pub pools_updated: u64,
    pub sync_events_processed: u64,
    pub swap_events_processed: u64,
    pub last_sync_time: Option<Instant>,
    pub errors_count: u64,
}

impl SyncStats {
    /// 记录池更新
    pub fn record_pool_update(&mut self) {
        self.pools_updated += 1;
        self.last_sync_time = Some(Instant::now());
    }
    
    /// 记录同步事件
    pub fn record_sync_event(&mut self) {
        self.sync_events_processed += 1;
    }
    
    /// 记录交换事件
    pub fn record_swap_event(&mut self) {
        self.swap_events_processed += 1;
    }
    
    /// 记录错误
    pub fn record_error(&mut self) {
        self.errors_count += 1;
    }
    
    /// 获取同步频率(每秒更新数)
    pub fn get_sync_rate(&self) -> f64 {
        if let Some(last_sync) = self.last_sync_time {
            let duration = last_sync.elapsed().as_secs_f64();
            if duration > 0.0 {
                return self.pools_updated as f64 / duration;
            }
        }
        0.0
    }
}

/// 代币对到池地址的映射
pub type TokenPairPools = HashMap<(Address, Address), Vec<Address>>;

/// DEX到池地址的映射  
pub type DexPools = HashMap<DexType, Vec<Address>>;

/// 代币信息缓存
pub type TokenCache = HashMap<Address, Token>;

/// 池状态缓存
pub type PoolCache = HashMap<Address, PoolState>;
