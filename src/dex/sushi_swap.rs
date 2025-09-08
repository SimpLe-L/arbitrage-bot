use std::sync::Arc;

use dex_indexer::types::Protocol;
use ethers::types::{Address, U256};
use eyre::Result;
use simulator::Simulator;

use super::{Dex, FlashResult, TradeCtx};

#[derive(Debug, Clone)]
pub struct SushiSwapDex {
    pub pool: Address,
    pub token_in: String,
    pub token_out: String,
    pub liquidity: u128,
    pub fee_rate: u64,
}

impl SushiSwapDex {
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
impl Dex for SushiSwapDex {
    fn support_flashloan(&self) -> bool {
        false
    }

    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        eyre::bail!("flashloan not supported")
    }

    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _coin: ethers::types::Bytes, _flash_res: FlashResult) -> Result<ethers::types::Bytes> {
        eyre::bail!("flashloan not supported")
    }

    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: Address,
        coin_in: ethers::types::Bytes,
        amount_in: Option<u64>,
    ) -> Result<ethers::types::Bytes> {
        // SushiSwap swap implementation would go here
        todo!("SushiSwap swap not implemented yet")
    }

    fn coin_in_type(&self) -> String {
        self.token_in.clone()
    }

    fn coin_out_type(&self) -> String {
        self.token_out.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::SushiSwap
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
        self.token_in < self.token_out
    }

    async fn swap_tx(&self, sender: Address, recipient: Address, amount_in: u64) -> Result<ethers::types::TransactionRequest> {
        // SushiSwap swap transaction building would go here
        todo!("SushiSwap swap_tx not implemented yet")
    }
}

pub async fn sushi_swap_related_contract_addresses() -> Vec<String> {
    vec![
        "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string(), // SushiSwap Router
        "0xc35DADB65012eC5796536bD9864eD8773aBc74C4".to_string(), // SushiSwap Factory
    ]
}
