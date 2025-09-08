use std::sync::Arc;

use dex_indexer::types::Protocol;
use ethers::types::{Address, U256};
use eyre::Result;
use simulator::Simulator;

use super::{Dex, FlashResult, TradeCtx};

#[derive(Debug, Clone)]
pub struct UniswapV3Dex {
    pub pool: Address,
    pub token_in: String,
    pub token_out: String,
    pub liquidity: u128,
    pub fee_rate: u64,
    pub tick_spacing: i32,
}

impl UniswapV3Dex {
    pub fn new(
        pool: Address,
        token_in: String,
        token_out: String,
        liquidity: u128,
        fee_rate: u64,
        tick_spacing: i32,
    ) -> Self {
        Self {
            pool,
            token_in,
            token_out,
            liquidity,
            fee_rate,
            tick_spacing,
        }
    }
}

#[async_trait::async_trait]
impl Dex for UniswapV3Dex {
    fn support_flashloan(&self) -> bool {
        true
    }

    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        // UniswapV3 flashloan implementation would go here
        todo!("UniswapV3 flashloan not implemented yet")
    }

    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _coin: ethers::types::Bytes, _flash_res: FlashResult) -> Result<ethers::types::Bytes> {
        // UniswapV3 repay implementation would go here
        todo!("UniswapV3 repay not implemented yet")
    }

    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: Address,
        coin_in: ethers::types::Bytes,
        amount_in: Option<u64>,
    ) -> Result<ethers::types::Bytes> {
        // UniswapV3 swap implementation would go here
        todo!("UniswapV3 swap not implemented yet")
    }

    fn coin_in_type(&self) -> String {
        self.token_in.clone()
    }

    fn coin_out_type(&self) -> String {
        self.token_out.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::UniswapV3
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
        // UniswapV3 swap transaction building would go here
        todo!("UniswapV3 swap_tx not implemented yet")
    }
}

pub async fn uniswap_v3_related_contract_addresses() -> Vec<String> {
    vec![
        "0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE".to_string(), // UniswapV3 Router
        "0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD".to_string(), // UniswapV3 Factory
    ]
}
