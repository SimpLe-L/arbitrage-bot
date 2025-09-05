mod foundry_simulator;
mod http_simulator;

use async_trait::async_trait;
use eyre::Result;
use ethers::types::{Address, Block, Transaction, TransactionReceipt, U256, H256};
use serde::{Deserialize, Serialize};

pub use foundry_simulator::FoundrySimulator;
pub use http_simulator::HttpSimulator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateResult {
    pub transaction_hash: H256,
    pub receipt: TransactionReceipt,
    pub gas_used: U256,
    pub gas_price: U256,
    pub balance_changes: Vec<BalanceChange>,
    pub logs: Vec<ethers::types::Log>,
    pub cache_misses: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    pub address: Address,
    pub token: Address, // 0x0 for native AVAX
    pub amount: i128,   // positive for incoming, negative for outgoing
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimEpoch {
    pub block_number: u64,
    pub block_timestamp: u64,
    pub base_fee: U256,
    pub gas_limit: U256,
}

impl SimEpoch {
    pub fn from_block(block: &Block<H256>) -> Self {
        Self {
            block_number: block.number.unwrap_or_default().as_u64(),
            block_timestamp: block.timestamp.as_u64(),
            base_fee: block.base_fee_per_gas.unwrap_or_default(),
            gas_limit: block.gas_limit,
        }
    }

    pub fn is_stale(&self, max_age_seconds: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now.saturating_sub(self.block_timestamp) > max_age_seconds
    }
}

#[derive(Debug, Clone, Default)]
pub struct SimulateCtx {
    pub epoch: SimEpoch,
    pub override_balances: Vec<(Address, Address, U256)>, // (account, token, balance)
    pub flashloan_amount: Option<(Address, U256)>, // (token, amount)
    pub fork_block: Option<u64>,
}

impl SimulateCtx {
    pub fn new(epoch: SimEpoch) -> Self {
        Self {
            epoch,
            override_balances: Vec::new(),
            flashloan_amount: None,
            fork_block: None,
        }
    }

    pub fn with_override_balance(&mut self, account: Address, token: Address, balance: U256) -> &mut Self {
        self.override_balances.push((account, token, balance));
        self
    }

    pub fn with_flashloan(&mut self, token: Address, amount: U256) -> &mut Self {
        self.flashloan_amount = Some((token, amount));
        self
    }

    pub fn with_fork_block(&mut self, block_number: u64) -> &mut Self {
        self.fork_block = Some(block_number);
        self
    }

    pub fn with_base_fee(&mut self, base_fee: U256) -> &mut Self {
        self.epoch.base_fee = base_fee;
        self
    }
}

#[async_trait]
pub trait Simulator: Sync + Send {
    async fn simulate(&self, tx: Transaction, ctx: SimulateCtx) -> Result<SimulateResult>;
    
    async fn get_balance(&self, account: Address, token: Address) -> Option<U256>;
    
    async fn get_block(&self, block_number: Option<u64>) -> Option<Block<H256>>;
    
    fn name(&self) -> &str;

    /// Get the maximum gas limit for transactions
    fn max_gas_limit(&self) -> U256 {
        U256::from(30_000_000) // Default AVAX C-Chain block gas limit
    }

    /// Estimate gas for a transaction
    async fn estimate_gas(&self, tx: &Transaction) -> Result<U256>;
}
