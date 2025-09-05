//! MEV策略模块
//! 
//! 这个模块包含了MEV机器人的各种策略实现，包括套利策略、配置管理等

// 子模块声明
pub mod arb;
pub mod arbitrage; // 精简版套利引擎
pub mod config;
pub mod traits;
pub mod stats;
pub mod simulator;
pub mod dex_sync;
pub mod amm;

// 重新导出主要的公共接口
pub use arb::{ArbitrageHandler, ArbitragePathFinder};
pub use config::{AppConfig, BotConfig, AvaxConfig, DexConfig, AllDexConfig, NotificationConfig, ConfigManager};
pub use traits::*;
pub use stats::*;
pub use simulator::{Simulator, FoundrySimulator, SimpleSimulator};

// 导出DEX数据同步相关
pub use dex_sync::{
    DexDataSyncer, PoolManager, 
    DexType, Pool, PoolState, Token, SwapEvent, SyncEvent, DexConfig as DexSyncConfig,
    SyncStats, TokenPairPools, DexPools, TokenCache, PoolCache
};

// 导出AMM计算相关
pub use amm::{
    AmmCalculator, AmmCalculatorManager, UniswapV2Calculator,
    SwapParams, SwapInput, SwapOutput, ArbitrageStep, ArbitragePath, PriceInfo,
    AmmError, AmmResult, AmmProtocol, SlippageConfig,
    create_avax_calculator_manager, quick_swap_calculation, calculate_min_amount_out
};
