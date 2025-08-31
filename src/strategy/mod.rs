//! MEV策略模块
//! 
//! 这个模块包含了MEV机器人的各种策略实现，包括套利策略、配置管理等

// 子模块声明
pub mod arb;
pub mod config;
pub mod traits;
pub mod stats;
pub mod simulator;

// 重新导出主要的公共接口
pub use arb::{ArbitrageHandler, ArbitragePathFinder};
pub use config::{AppConfig, BotConfig, AvaxConfig, DexConfig, AllDexConfig, NotificationConfig, ConfigManager};
pub use traits::*;
pub use stats::*;
pub use simulator::{Simulator, FoundrySimulator, SimpleSimulator};
