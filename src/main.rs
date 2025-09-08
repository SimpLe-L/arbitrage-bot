mod bot;
mod common;
mod dex;
mod engine;
mod indexer;
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

    #[arg(long, help = "deprecated")]
    pub ipc_path: Option<String>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    StartBot(bot::start_bot::Args),
    Run(strategy::arb::Args),
    /// Generate a file with contract addresses of all pools and their underlying contracts
    PoolIds(tools::pool_ids::Args),
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::StartBot(args) => bot::start_bot::run(args).await,
        Command::Run(args) => strategy::arb::run(args).await,
        Command::PoolIds(args) => tools::pool_ids::run(args).await,
    }
}
