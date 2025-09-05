use std::sync::Arc;

use dex_indexer::types::Protocol;
use ethers::types::{Address, U256};
use eyre::Result;
use simulator::Simulator;

use super::{Dex, FlashResult, TradeCtx};

#[derive(Debug, Clone)]
pub struct CurveDex {
    pub pool: Address,
    pub token_in: String,
    pub token_out: String,
    pub liquidity: u128,
    pub fee_rate: u64,
    pub amplification: u64,
}

impl CurveDex {
    pub fn new(
        pool: Address,
        token_in: String,
        token_out: String,
        liquidity: u128,
        fee_rate: u64,
        amplification: u64,
    ) -> Self {
        Self {
            pool,
            token_in,
            token_out,
            liquidity,
            fee_rate,
            amplification,
        }
    }
}

#[async_trait::async_trait]
impl Dex for CurveDex {
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
        // Curve swap implementation would go here
        todo!("Curve swap not implemented yet")
    }

    fn coin_in_type(&self) -> String {
        self.token_in.clone()
    }

    fn coin_out_type(&self) -> String {
        self.token_out.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::Curve
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
        // Curve swap transaction building would go here
        todo!("Curve swap_tx not implemented yet")
    }
}

pub async fn curve_related_contract_addresses() -> Vec<String> {
    vec![
        "0x7f90122BF0700F9E7e1F688fe926940E8839F353".to_string(), // Curve Pool Registry
        "0x8474DdbE98F5aA3179B3B3F5942D724aFcdec9f6".to_string(), // Curve Address Provider
    ]
}
