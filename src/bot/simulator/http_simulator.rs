use async_trait::async_trait;
use eyre::Result;
use ethers::{
    providers::{Http, Provider, Middleware},
    types::{Address, Block, Transaction, TransactionReceipt, U256, H256, BlockId},
    utils::parse_ether,
};
use std::sync::Arc;
use tracing::warn;

use super::{BalanceChange, SimulateCtx, SimulateResult, Simulator};

#[derive(Clone)]
pub struct HttpSimulator {
    pub provider: Arc<Provider<Http>>,
    pub chain_id: u64,
}

impl HttpSimulator {
    pub async fn new(rpc_url: impl AsRef<str>, chain_id: Option<u64>) -> Result<Self> {
        warn!("HTTP simulator may not provide accurate balance change calculations for complex MEV operations");

        let provider = Provider::<Http>::try_from(rpc_url.as_ref())?;
        let provider = Arc::new(provider);

        let chain_id = if let Some(chain_id) = chain_id {
            chain_id
        } else {
            provider.get_chainid().await?.as_u64()
        };

        Ok(Self { provider, chain_id })
    }

    pub async fn new_avalanche_mainnet(rpc_url: impl AsRef<str>) -> Result<Self> {
        Self::new(rpc_url, Some(43114)).await // Avalanche C-Chain mainnet
    }

    pub async fn new_avalanche_fuji(rpc_url: impl AsRef<str>) -> Result<Self> {
        Self::new(rpc_url, Some(43113)).await // Avalanche C-Chain testnet (Fuji)
    }

    pub async fn max_budget(&self) -> U256 {
        // Get latest block to determine gas limit
        if let Ok(Some(block)) = self.provider.get_block(BlockId::latest()).await {
            block.gas_limit
        } else {
            // Default AVAX C-Chain block gas limit
            U256::from(15_000_000)
        }
    }

    pub async fn get_gas_price(&self) -> Result<U256> {
        self.provider.get_gas_price().await.map_err(Into::into)
    }

    async fn calculate_balance_changes(
        &self,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        ctx: &SimulateCtx,
    ) -> Result<Vec<BalanceChange>> {
        let mut balance_changes = Vec::new();

        // Calculate gas cost
        let gas_cost = receipt.gas_used.unwrap_or_default() * receipt.effective_gas_price.unwrap_or(tx.gas_price.unwrap_or_default());
        
        // Gas cost for sender (negative)
        if gas_cost > U256::zero() {
            balance_changes.push(BalanceChange {
                address: tx.from,
                token: Address::zero(), // Native AVAX
                amount: -(gas_cost.as_u128() as i128),
            });
        }

        // Value transfer (if any)
        if let Some(value) = tx.value {
            if value > U256::zero() {
                // Sender loses value
                balance_changes.push(BalanceChange {
                    address: tx.from,
                    token: Address::zero(),
                    amount: -(value.as_u128() as i128),
                });

                // Recipient gains value (if not a contract creation)
                if let Some(to) = tx.to {
                    balance_changes.push(BalanceChange {
                        address: to,
                        token: Address::zero(),
                        amount: value.as_u128() as i128,
                    });
                }
            }
        }

        // Handle flashloan repayment if applicable
        if let Some((token, amount)) = &ctx.flashloan_amount {
            balance_changes.push(BalanceChange {
                address: tx.from,
                token: *token,
                amount: -(amount.as_u128() as i128),
            });
        }

        // TODO: Parse logs for ERC20 transfers and other token movements
        // This would require more sophisticated log parsing based on known token contracts

        Ok(balance_changes)
    }
}

#[async_trait]
impl Simulator for HttpSimulator {
    async fn simulate(&self, tx: Transaction, ctx: SimulateCtx) -> Result<SimulateResult> {
        // Note: This is a simplified simulation using call/estimateGas
        // For more accurate simulation, consider using anvil fork mode
        
        let block_id = if let Some(fork_block) = ctx.fork_block {
            BlockId::Number(fork_block.into())
        } else {
            BlockId::Number(ctx.epoch.block_number.into())
        };

        // Estimate gas
        let gas_estimate = self.provider
            .estimate_gas(&tx.into(), Some(block_id))
            .await?;

        // Get current gas price or use provided one
        let gas_price = if tx.gas_price.is_some() {
            tx.gas_price.unwrap()
        } else {
            self.get_gas_price().await.unwrap_or(ctx.epoch.base_fee)
        };

        // Create a mock receipt (since we can't actually execute without sending)
        let receipt = TransactionReceipt {
            transaction_hash: tx.hash,
            transaction_index: Some(0u64.into()),
            block_hash: Some(H256::random()),
            block_number: Some(ctx.epoch.block_number.into()),
            from: tx.from,
            to: tx.to,
            cumulative_gas_used: gas_estimate,
            gas_used: Some(gas_estimate),
            contract_address: None,
            logs: Vec::new(), // Would need to parse from call result
            status: Some(1u64.into()), // Assume success
            root: None,
            logs_bloom: Default::default(),
            transaction_type: tx.transaction_type,
            effective_gas_price: Some(gas_price),
        };

        let balance_changes = self.calculate_balance_changes(&tx, &receipt, &ctx).await?;

        Ok(SimulateResult {
            transaction_hash: tx.hash,
            receipt,
            gas_used: gas_estimate,
            gas_price,
            balance_changes,
            logs: Vec::new(),
            cache_misses: 0,
        })
    }

    fn name(&self) -> &str {
        "HttpSimulator"
    }

    async fn get_balance(&self, account: Address, token: Address) -> Option<U256> {
        if token == Address::zero() {
            // Native AVAX balance
            self.provider.get_balance(account, None).await.ok()
        } else {
            // ERC20 token balance - would need to call balanceOf
            // This requires the ERC20 contract interface
            None // TODO: Implement ERC20 balance checking
        }
    }

    async fn get_block(&self, block_number: Option<u64>) -> Option<Block<H256>> {
        let block_id = block_number
            .map(|n| BlockId::Number(n.into()))
            .unwrap_or(BlockId::latest());
            
        self.provider.get_block(block_id).await.ok().flatten()
    }

    async fn estimate_gas(&self, tx: &Transaction) -> Result<U256> {
        self.provider
            .estimate_gas(&tx.clone().into(), None)
            .await
            .map_err(Into::into)
    }
}
