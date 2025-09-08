use std::sync::Arc;

use dex_indexer::types::Protocol;
use ethers::types::{Address, U256};
use eyre::Result;
use simulator::Simulator;

use super::{Dex, FlashResult, TradeCtx};

#[derive(Debug, Clone)]
pub struct TraderJoeDex {
    pub pool: Address,
    pub token_in: String,
    pub token_out: String, 
    pub liquidity: u128,
    pub fee_rate: u64,
}

impl TraderJoeDex {
    pub fn new(
        pool: Address,
        token_in: String,
        token_out: String,
        liquidity: u128,
        fee_rate: u64,
    ) -> Self {
        Self {
            pool,
            token_in,
            token_out,
            liquidity,
            fee_rate,
        }
    }
}

#[async_trait::async_trait]
impl Dex for TraderJoeDex {
    fn support_flashloan(&self) -> bool {
        true
    }

    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        // TraderJoe flashloan implementation would go here
        todo!("TraderJoe flashloan not implemented yet")
    }

    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _coin: ethers::types::Bytes, _flash_res: FlashResult) -> Result<ethers::types::Bytes> {
        // TraderJoe repay implementation would go here
        todo!("TraderJoe repay not implemented yet")  
    }

    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: Address,
        coin_in: ethers::types::Bytes,
        amount_in: Option<u64>,
    ) -> Result<ethers::types::Bytes> {
        // TraderJoe swap implementation would go here
        todo!("TraderJoe swap not implemented yet")
    }

    fn coin_in_type(&self) -> String {
        self.token_in.clone()
    }

    fn coin_out_type(&self) -> String {
        self.token_out.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::TraderJoe
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn pool_address(&self) -> Address {
        self.pool
    }

    fn flip(&mut self) {
        std::mem::swap(&mut self.token_in, &mut self.token_out);
    }

    fn is_a2b(&self) -> bool {
        // Determine direction based on token addresses
        self.token_in < self.token_out
    }

    async fn swap_tx(&self, sender: Address, recipient: Address, amount_in: u64) -> Result<ethers::types::TransactionRequest> {
        // TraderJoe swap transaction building would go here
        todo!("TraderJoe swap_tx not implemented yet")
    }
}

pub async fn trader_joe_related_contract_addresses() -> Vec<String> {
    vec![
        "0x60aE616a2155Ee3d9A68541Ba4544862310933d4".to_string(), // TraderJoe Router
        "0x9Ad6C38BE94206cA50bb0d90783181662f0Cfa10".to_string(), // TraderJoe Factory  
    ]
}
