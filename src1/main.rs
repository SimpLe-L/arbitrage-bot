//! AVAX MEV套利机器人主程序
//! 
//! 这是整个MEV机器人的启动入口，负责初始化所有组件并启动套利流程

use tokio;
use std::sync::Arc;
use std::time::Duration;
use log::{info, warn, error, debug};
use env_logger;
use tokio::sync::RwLock;
use ethers::types::{Address, U256};

mod core;
mod strategy;
mod utils;

use core::{
    engine::MevEngine,
    executor::{ExecutorManager, PrintExecutor, MockExecutor, MempoolExecutor, FlashbotExecutor},
    collectors::{EventBus, BlockCollector, MempoolCollector},
    types::{Token, Pool, DexType, BotError, Result}
};
use strategy::{
    config::ConfigManager,
    arb::{ArbitrageHandler, ArbitragePathFinder}
};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    info!("🚀 启动AVAX MEV套利机器人...");
    
    // 加载配置
    let config = match load_config().await {
        Ok(config) => {
            info!("✅ 配置加载成功");
            ConfigManager::print_config_summary(&config);
            config
        }
        Err(e) => {
            error!("❌ 配置加载失败: {}", e);
            return Err(e.into());
        }
    };
    
    // 创建MEV引擎
    let mut engine = MevEngine::new();
    info!("✅ MEV引擎创建成功");
    
    // 设置执行器管理器
    let executor_manager = setup_executors(&config).await?;
    info!("✅ 执行器设置完成");
    
    // 设置收集器
    setup_collectors(&mut engine, &config).await?;
    info!("✅ 收集器设置完成");
    
    // 设置套利处理器
    setup_arbitrage_handler(&mut engine, executor_manager, &config).await?;
    info!("✅ 套利处理器设置完成");
    
    // 添加示例代币和池数据（实际应用中应该从链上获取）
    setup_sample_data(&engine).await?;
    info!("✅ 示例数据设置完成");
    
    info!("🎯 MEV套利机器人初始化完成！");
    info!("📊 开始监听区块链事件并寻找套利机会...");
    
    // 启动引擎
    match engine.run_until_stopped().await {
        Ok(_) => {
            info!("🏁 MEV套利机器人正常退出");
        }
        Err(e) => {
            error!("💥 MEV套利机器人运行出错: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// 从环境变量加载配置
async fn load_config() -> Result<strategy::config::AppConfig> {
    match ConfigManager::load_from_env() {
        Ok(config) => Ok(config),
        Err(e) => {
            warn!("从环境变量加载配置失败: {}", e);
            info!("使用默认配置...");
            
            // 创建默认配置
            let mut config = strategy::config::AppConfig::default();
            
            // 设置一个示例私钥（实际使用中必须从环境变量获取）
            config.private_key = "0x1234567890123456789012345678901234567890123456789012345678901234".to_string();
            
            warn!("⚠️ 警告：正在使用示例私钥，请在生产环境中设置正确的PRIVATE_KEY环境变量");
            warn!("⚠️ 警告：正在使用默认RPC和WS URL，请根据需要在环境变量中设置");
            
            Ok(config)
        }
    }
}

/// 设置执行器
async fn setup_executors(config: &strategy::config::AppConfig) -> Result<Arc<RwLock<ExecutorManager>>> {
    let mut executor_manager = ExecutorManager::new();
    
    // 添加打印执行器（用于显示套利机会）
    executor_manager.add_executor(Box::new(PrintExecutor::new("PrintExecutor".to_string())));
    
    // 添加模拟执行器
    executor_manager.add_executor(Box::new(MockExecutor::new("MockExecutor".to_string())));
    
    // 尝试添加真实执行器（如果配置允许）
    if !config.private_key.is_empty() && config.private_key != "0x1234567890123456789012345678901234567890123456789012345678901234" {
        info!("🔑 检测到有效私钥，添加真实执行器");
        
        // 添加内存池执行器
        match MempoolExecutor::new(
            &config.network.mainnet_rpc_url,
            &config.private_key,
            config.network.chain_id,
        ).await {
            Ok(mempool_executor) => {
                executor_manager.add_executor(Box::new(mempool_executor));
                info!("✅ 内存池执行器添加成功");
            }
            Err(e) => {
                warn!("⚠️ 内存池执行器添加失败: {}", e);
            }
        }
        
        // 添加Flashbot执行器
        match FlashbotExecutor::new(
            &config.network.mainnet_rpc_url,
            &config.private_key,
            config.network.chain_id,
            None,
        ).await {
            Ok(flashbot_executor) => {
                executor_manager.add_executor(Box::new(flashbot_executor));
                info!("✅ Flashbot执行器添加成功");
            }
            Err(e) => {
                warn!("⚠️ Flashbot执行器添加失败: {}", e);
            }
        }
    } else {
        warn!("⚠️ 未检测到有效私钥，仅使用模拟执行器");
    }
    
    info!("执行器总数: {}", executor_manager.executor_count());
    Ok(Arc::new(RwLock::new(executor_manager)))
}

/// 设置收集器
async fn setup_collectors(engine: &mut MevEngine, config: &strategy::config::AppConfig) -> Result<()> {
    // 添加区块收集器
    match BlockCollector::new(&config.network.mainnet_ws_url, config.network.chain_id).await {
        Ok(block_collector) => {
            engine.add_collector(Box::new(block_collector));
            info!("✅ 区块收集器添加成功");
        }
        Err(e) => {
            warn!("⚠️ 区块收集器添加失败: {}", e);
        }
    }
    
    // 添加内存池收集器
    match MempoolCollector::new(&config.network.mainnet_ws_url, config.network.chain_id).await {
        Ok(mempool_collector) => {
            let mempool_collector = mempool_collector
                .with_min_gas_price(U256::from(config.bot.max_gas_price_gwei) * U256::from(10u64.pow(9)) / U256::from(2))
                .contracts_only();
            engine.add_collector(Box::new(mempool_collector));
            info!("✅ 内存池收集器添加成功");
        }
        Err(e) => {
            warn!("⚠️ 内存池收集器添加失败: {}", e);
        }
    }
    
    Ok(())
}

/// 设置套利处理器
async fn setup_arbitrage_handler(
    engine: &mut MevEngine,
    executor_manager: Arc<RwLock<ExecutorManager>>,
    config: &strategy::config::AppConfig,
) -> Result<()> {
    let arbitrage_handler = ArbitrageHandler::new(
        executor_manager,
        config.bot.min_profit_threshold,
        config.bot.max_hops,
    );
    
    // TODO: 在这里添加handler到引擎
    // engine.add_handler(Box::new(arbitrage_handler));
    info!("套利处理器创建成功（事件处理暂时禁用）");
    
    Ok(())
}

/// 设置示例数据（代币和池信息）
async fn setup_sample_data(engine: &MevEngine) -> Result<()> {
    info!("🔧 设置示例代币和池数据...");
    
    // 示例AVAX代币
    let wavax = Token {
        address: Address::from_low_u64_be(1), // WAVAX地址示例
        symbol: "WAVAX".to_string(),
        name: "Wrapped AVAX".to_string(),
        decimals: 18,
    };
    
    let usdc = Token {
        address: Address::from_low_u64_be(2), // USDC地址示例
        symbol: "USDC".to_string(),
        name: "USD Coin".to_string(),
        decimals: 6,
    };
    
    let usdt = Token {
        address: Address::from_low_u64_be(3), // USDT地址示例
        symbol: "USDT".to_string(),
        name: "Tether USD".to_string(),
        decimals: 6,
    };
    
    // 示例池
    let pool1 = Pool {
        address: Address::from_low_u64_be(101),
        token0: wavax.clone(),
        token1: usdc.clone(),
        dex: DexType::TraderJoe,
        reserve0: U256::from(1000u64) * U256::from(10u64.pow(18)), // 1000 WAVAX
        reserve1: U256::from(25000u64) * U256::from(10u64.pow(6)),  // 25000 USDC
        fee: U256::from(30), // 0.3%
    };
    
    let pool2 = Pool {
        address: Address::from_low_u64_be(102),
        token0: usdc.clone(),
        token1: usdt.clone(),
        dex: DexType::Pangolin,
        reserve0: U256::from(50000u64) * U256::from(10u64.pow(6)), // 50000 USDC
        reserve1: U256::from(50100u64) * U256::from(10u64.pow(6)), // 50100 USDT (略高于USDC，可能存在套利机会)
        fee: U256::from(25), // 0.25%
    };
    
    let pool3 = Pool {
        address: Address::from_low_u64_be(103),
        token0: usdt.clone(),
        token1: wavax.clone(),
        dex: DexType::Sushiswap,
        reserve0: U256::from(25200u64) * U256::from(10u64.pow(6)), // 25200 USDT
        reserve1: U256::from(1000u64) * U256::from(10u64.pow(18)), // 1000 WAVAX
        fee: U256::from(30), // 0.3%
    };
    
    info!("示例代币和池数据设置完成");
    info!("- 代币: WAVAX, USDC, USDT");
    info!("- 池: TraderJoe (WAVAX/USDC), Pangolin (USDC/USDT), Sushiswap (USDT/WAVAX)");
    info!("- 可能的套利路径: WAVAX -> USDC -> USDT -> WAVAX");
    
    Ok(())
}

/// 优雅关闭处理
async fn handle_shutdown() {
    info!("正在关闭MEV套利机器人...");
    // TODO: 实现优雅关闭逻辑
    info!("MEV套利机器人已关闭");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        // 测试配置加载
        tokio_test::block_on(async {
            let config = load_config().await;
            assert!(config.is_ok());
        });
    }
    
    #[test]
    fn test_executor_setup() {
        // 测试执行器设置
        tokio_test::block_on(async {
            let config = strategy::config::AppConfig::default();
            let executor_manager = setup_executors(&config).await;
            assert!(executor_manager.is_ok());
            let manager = executor_manager.unwrap();
            let manager_lock = manager.read().await;
            assert!(manager_lock.executor_count() > 0);
        });
    }
}
