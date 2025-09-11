mod bindings;
mod bot;
mod common;
mod contract_executor;
mod dex;
mod engine;
mod simulator;
mod strategy;
mod tools;
mod types;
mod utils;

use clap::Parser;
use eyre::Result;

pub const BUILD_VERSION: &str = version::build_version!();

#[derive(clap::Parser)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, Parser)]
#[command(about = "Common configuration")]
pub struct HttpConfig {
    #[arg(long, env = "AVAX_RPC_URL", default_value = "https://api.avax.network/ext/bc/C/rpc")]
    pub rpc_url: String,

    #[arg(long, env = "AVAX_WS_URL", default_value = "wss://api.avax.network/ext/bc/C/ws")]
    pub ws_url: String,

    #[arg(long, help = "deprecated")]
    pub ipc_path: Option<String>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    StartBot(bot::start_bot::Args),
    Run(strategy::arb::Args),
    // ContractArb功能与StartBot重复，已删除
    // ContractArb(strategy::contract_arb::ContractArbArgs),
    // PoolIds工具命令，用不到，已删除
    // PoolIds(tools::pool_ids::Args),
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::StartBot(args) => bot::start_bot::run(args).await,
        Command::Run(args) => strategy::arb::run(args).await,
    }
}
