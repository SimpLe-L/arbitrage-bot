pub mod notification;
pub mod search;

use eyre::Result;
use ethers::{providers::{Http, Provider, Middleware}, types::{BlockId, BlockNumber}};
use std::sync::Arc;
use crate::bot::simulator::SimEpoch;

pub async fn get_latest_epoch(provider: &Arc<Provider<Http>>) -> Result<SimEpoch> {
    let latest_block = provider.get_block(BlockId::latest()).await?.ok_or_else(|| {
        eyre::eyre!("Failed to get latest block")
    })?;
    
    Ok(SimEpoch::from_block(&latest_block))
}

pub async fn get_latest_block(rpc_url: &str) -> Result<BlockNumber> {
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let latest_block = provider.get_block_number().await?;
    Ok(latest_block)
}
