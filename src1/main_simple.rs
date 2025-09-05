//! 精简版MEV套利机器人 - 参考sui-mev架构设计
//! 移除复杂的引擎抽象，专注核心套利功能

use clap::Parser;
use eyre::Result;
use std::str::FromStr;
use tracing::{info, warn};
use ethers::{
    prelude::*,
    types::{Address, U256},
};
use tokio::time::{Duration, Instant};

mod core;
mod strategy;
mod utils;

use core::types::{Token, Pool, DexType};
use strategy::arbitrage::SimpleArbitrage;

#[derive(Parser)]
#[command(about = "简化版AVAX MEV套利机器人")]
pub struct Args {
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: String,
    
    #[arg(long, env = "RPC_URL", default_value = "https://api.avax.network/ext/bc/C/rpc")]
    pub rpc_url: String,
    
    #[arg(long, env = "WS_URL", default_value = "wss://api.avax.network/ext/bc/C/ws")]
    pub ws_url: String,
    
    #[arg(long, default_value = "1000000000000000000")] // 1 AVAX minimum profit
    pub min_profit_wei: u64,
    
    #[arg(long, default_value = "50000000000")] // 50 gwei
    pub max_gas_price_wei: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 简单的日志初始化
    tracing_subscriber::fmt()
        .with_env_filter("info,simple_mev=debug")
        .init();
    
    let args = Args::parse();
    info!("🚀 启动精简版MEV套利机器人");
    
    // 验证私钥格式
    if args.private_key.len() != 66 || !args.private_key.starts_with("0x") {
        warn!("⚠️ 私钥格式可能不正确，请确保是0x开头的64字符十六进制");
    }
    
    // 创建简化的套利引擎
    let arbitrage = SimpleArbitrage::new(
        &args.rpc_url,
        &args.ws_url,
        &args.private_key,
        args.min_profit_wei,
        args.max_gas_price_wei,
    ).await?;
    
    info!("✅ 套利引擎初始化完成");
    info!("📈 开始监听套利机会...");
    info!("💰 最小利润阈值: {} wei", args.min_profit_wei);
    info!("⛽ 最大Gas价格: {} gwei", args.max_gas_price_wei / 1_000_000_000);
    
    // 运行套利循环 - 简单直接，无复杂状态管理
    arbitrage.run().await
}
