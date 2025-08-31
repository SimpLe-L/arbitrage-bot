use crate::core::types::{Config, BotError, Result};
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use std::env;
use std::str::FromStr;

/// AVAX网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvaxConfig {
    /// 主网RPC URL
    pub mainnet_rpc_url: String,
    /// 主网WebSocket URL
    pub mainnet_ws_url: String,
    /// 链ID (AVAX主网: 43114, Fuji测试网: 43113)
    pub chain_id: u64,
    /// 是否使用测试网
    pub use_testnet: bool,
}

impl Default for AvaxConfig {
    fn default() -> Self {
        Self {
            mainnet_rpc_url: "https://api.avax.network/ext/bc/C/rpc".to_string(),
            mainnet_ws_url: "wss://api.avax.network/ext/bc/C/ws".to_string(),
            chain_id: 43114, // AVAX主网
            use_testnet: false,
        }
    }
}

/// DEX配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    /// 路由合约地址
    pub router_address: Address,
    /// 工厂合约地址
    pub factory_address: Address,
    /// 交易手续费 (基点，如30表示0.3%)
    pub fee_bps: u16,
    /// 是否启用
    pub enabled: bool,
}

/// 所有DEX的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllDexConfig {
    pub trader_joe: DexConfig,
    pub pangolin: DexConfig,
    pub sushiswap: DexConfig,
}

impl Default for AllDexConfig {
    fn default() -> Self {
        Self {
            trader_joe: DexConfig {
                router_address: Address::from_str("0x60aE616a2155Ee3d9A68541Ba4544862310933d4").unwrap(),
                factory_address: Address::from_str("0x9Ad6C38BE94206cA50bb0d90783181662f0Cfa10").unwrap(),
                fee_bps: 30,
                enabled: true,
            },
            pangolin: DexConfig {
                router_address: Address::from_str("0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106").unwrap(),
                factory_address: Address::from_str("0xefa94DE7a4656D787667C749f7E1223D71E9FD88").unwrap(),
                fee_bps: 30,
                enabled: true,
            },
            sushiswap: DexConfig {
                router_address: Address::from_str("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506").unwrap(),
                factory_address: Address::from_str("0xc35DADB65012eC5796536bD9864eD8773aBc74C4").unwrap(),
                fee_bps: 30,
                enabled: true,
            },
        }
    }
}

/// 机器人运行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// 最小利润阈值 (wei)
    pub min_profit_threshold: U256,
    /// 最大gas价格 (gwei)
    pub max_gas_price_gwei: u64,
    /// 滑点容忍度 (基点，如500表示5%)
    pub slippage_tolerance_bps: u16,
    /// 最大跳数 (1-3)
    pub max_hops: u8,
    /// 是否启用模拟
    pub simulation_enabled: bool,
    /// 模拟失败时是否继续
    pub continue_on_simulation_failure: bool,
    /// mempool监听延迟 (毫秒)
    pub mempool_delay_ms: u64,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: U256::from(10u64.pow(16)), // 0.01 AVAX
            max_gas_price_gwei: 50,
            slippage_tolerance_bps: 100, // 1%
            max_hops: 3,
            simulation_enabled: true,
            continue_on_simulation_failure: false,
            mempool_delay_ms: 100,
        }
    }
}

/// 通知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Telegram Bot Token
    pub telegram_bot_token: Option<String>,
    /// Telegram Chat ID
    pub telegram_chat_id: Option<i64>,
    /// 是否启用Telegram通知
    pub telegram_enabled: bool,
    /// 是否通知所有交易
    pub notify_all_transactions: bool,
    /// 只通知成功的套利
    pub notify_successful_only: bool,
    /// 最小通知利润阈值
    pub min_notify_profit: U256,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            telegram_bot_token: None,
            telegram_chat_id: None,
            telegram_enabled: false,
            notify_all_transactions: false,
            notify_successful_only: true,
            min_notify_profit: U256::from(10u64.pow(17)), // 0.1 AVAX
        }
    }
}

/// 完整的应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 网络配置
    pub network: AvaxConfig,
    /// DEX配置
    pub dex: AllDexConfig,
    /// 机器人配置
    pub bot: BotConfig,
    /// 通知配置
    pub notification: NotificationConfig,
    /// 私钥 (从环境变量读取)
    #[serde(skip)]
    pub private_key: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            network: AvaxConfig::default(),
            dex: AllDexConfig::default(),
            bot: BotConfig::default(),
            notification: NotificationConfig::default(),
            private_key: String::new(),
        }
    }
}

/// 配置管理器
pub struct ConfigManager;

impl ConfigManager {
    /// 从环境变量加载配置
    pub fn load_from_env() -> Result<AppConfig> {
        dotenv::dotenv().ok(); // 加载.env文件，如果存在的话
        
        let mut config = AppConfig::default();
        
        // 加载必需的环境变量
        config.private_key = env::var("PRIVATE_KEY")
            .map_err(|_| BotError::ConfigError("PRIVATE_KEY environment variable is required".to_string()))?;
        
        // 可选的网络配置
        if let Ok(rpc_url) = env::var("RPC_URL") {
            config.network.mainnet_rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = env::var("WS_URL") {
            config.network.mainnet_ws_url = ws_url;
        }
        
        if let Ok(chain_id) = env::var("CHAIN_ID") {
            config.network.chain_id = chain_id.parse()
                .map_err(|_| BotError::ConfigError("Invalid CHAIN_ID".to_string()))?;
        }
        
        if let Ok(use_testnet) = env::var("USE_TESTNET") {
            config.network.use_testnet = use_testnet.parse()
                .map_err(|_| BotError::ConfigError("Invalid USE_TESTNET".to_string()))?;
        }
        
        // 可选的机器人配置
        if let Ok(min_profit) = env::var("MIN_PROFIT_THRESHOLD") {
            config.bot.min_profit_threshold = U256::from_dec_str(&min_profit)
                .map_err(|_| BotError::ConfigError("Invalid MIN_PROFIT_THRESHOLD".to_string()))?;
        }
        
        if let Ok(max_gas) = env::var("MAX_GAS_PRICE_GWEI") {
            config.bot.max_gas_price_gwei = max_gas.parse()
                .map_err(|_| BotError::ConfigError("Invalid MAX_GAS_PRICE_GWEI".to_string()))?;
        }
        
        if let Ok(slippage) = env::var("SLIPPAGE_TOLERANCE_BPS") {
            config.bot.slippage_tolerance_bps = slippage.parse()
                .map_err(|_| BotError::ConfigError("Invalid SLIPPAGE_TOLERANCE_BPS".to_string()))?;
        }
        
        if let Ok(max_hops) = env::var("MAX_HOPS") {
            let hops = max_hops.parse::<u8>()
                .map_err(|_| BotError::ConfigError("Invalid MAX_HOPS".to_string()))?;
            if hops < 1 || hops > 5 {
                return Err(BotError::ConfigError("MAX_HOPS must be between 1 and 5".to_string()));
            }
            config.bot.max_hops = hops;
        }
        
        if let Ok(simulation_enabled) = env::var("SIMULATION_ENABLED") {
            config.bot.simulation_enabled = simulation_enabled.parse()
                .map_err(|_| BotError::ConfigError("Invalid SIMULATION_ENABLED".to_string()))?;
        }
        
        // 可选的通知配置
        if let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") {
            config.notification.telegram_bot_token = Some(token);
            config.notification.telegram_enabled = true;
        }
        
        if let Ok(chat_id) = env::var("TELEGRAM_CHAT_ID") {
            config.notification.telegram_chat_id = Some(chat_id.parse()
                .map_err(|_| BotError::ConfigError("Invalid TELEGRAM_CHAT_ID".to_string()))?);
        }
        
        if let Ok(notify_all) = env::var("NOTIFY_ALL_TRANSACTIONS") {
            config.notification.notify_all_transactions = notify_all.parse()
                .map_err(|_| BotError::ConfigError("Invalid NOTIFY_ALL_TRANSACTIONS".to_string()))?;
        }
        
        // 验证配置
        Self::validate_config(&config)?;
        
        Ok(config)
    }
    
    /// 验证配置的有效性
    fn validate_config(config: &AppConfig) -> Result<()> {
        // 验证私钥格式
        if config.private_key.len() != 64 && !config.private_key.starts_with("0x") {
            return Err(BotError::ConfigError("Invalid private key format".to_string()));
        }
        
        // 验证网络配置
        if config.network.mainnet_rpc_url.is_empty() {
            return Err(BotError::ConfigError("RPC URL cannot be empty".to_string()));
        }
        
        if config.network.mainnet_ws_url.is_empty() {
            return Err(BotError::ConfigError("WebSocket URL cannot be empty".to_string()));
        }
        
        // 验证机器人配置
        if config.bot.max_hops < 1 || config.bot.max_hops > 5 {
            return Err(BotError::ConfigError("max_hops must be between 1 and 5".to_string()));
        }
        
        if config.bot.slippage_tolerance_bps > 1000 { // 最大10%
            return Err(BotError::ConfigError("slippage_tolerance_bps cannot exceed 1000 (10%)".to_string()));
        }
        
        // 验证通知配置
        if config.notification.telegram_enabled {
            if config.notification.telegram_bot_token.is_none() {
                return Err(BotError::ConfigError("Telegram bot token is required when telegram is enabled".to_string()));
            }
            
            if config.notification.telegram_chat_id.is_none() {
                return Err(BotError::ConfigError("Telegram chat ID is required when telegram is enabled".to_string()));
            }
        }
        
        Ok(())
    }
    
    /// 转换为核心Config类型
    pub fn to_core_config(app_config: &AppConfig) -> Config {
        Config {
            rpc_url: app_config.network.mainnet_rpc_url.clone(),
            ws_url: app_config.network.mainnet_ws_url.clone(),
            private_key: app_config.private_key.clone(),
            min_profit_threshold: app_config.bot.min_profit_threshold,
            max_gas_price: U256::from(app_config.bot.max_gas_price_gwei) * U256::from(10u64.pow(9)), // 转换为wei
            slippage_tolerance: app_config.bot.slippage_tolerance_bps as f64 / 10000.0, // 转换为小数
            max_hops: app_config.bot.max_hops,
            simulation_enabled: app_config.bot.simulation_enabled,
            telegram_bot_token: app_config.notification.telegram_bot_token.clone(),
            telegram_chat_id: app_config.notification.telegram_chat_id.map(|id| id.to_string()),
        }
    }
    
    /// 打印配置摘要 (不包含敏感信息)
    pub fn print_config_summary(config: &AppConfig) {
        log::info!("=== 配置摘要 ===");
        log::info!("网络: AVAX {} (链ID: {})", 
            if config.network.use_testnet { "Fuji测试网" } else { "主网" },
            config.network.chain_id
        );
        log::info!("RPC URL: {}", config.network.mainnet_rpc_url);
        log::info!("WebSocket URL: {}", config.network.mainnet_ws_url);
        log::info!("最小利润阈值: {} wei", config.bot.min_profit_threshold);
        log::info!("最大gas价格: {} gwei", config.bot.max_gas_price_gwei);
        log::info!("滑点容忍度: {}%", config.bot.slippage_tolerance_bps as f64 / 100.0);
        log::info!("最大跳数: {}", config.bot.max_hops);
        log::info!("模拟启用: {}", config.bot.simulation_enabled);
        
        log::info!("启用的DEX:");
        if config.dex.trader_joe.enabled {
            log::info!("  - Trader Joe (手续费: {}%)", config.dex.trader_joe.fee_bps as f64 / 100.0);
        }
        if config.dex.pangolin.enabled {
            log::info!("  - Pangolin (手续费: {}%)", config.dex.pangolin.fee_bps as f64 / 100.0);
        }
        if config.dex.sushiswap.enabled {
            log::info!("  - SushiSwap (手续费: {}%)", config.dex.sushiswap.fee_bps as f64 / 100.0);
        }
        
        if config.notification.telegram_enabled {
            log::info!("Telegram通知: 启用");
        } else {
            log::info!("Telegram通知: 禁用");
        }
        log::info!("==================");
    }
}
