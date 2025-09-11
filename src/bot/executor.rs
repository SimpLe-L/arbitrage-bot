use async_trait::async_trait;
use crate::types::Executor;
use eyre::Result;
use ethers::{
    types::{transaction::eip2718::TypedTransaction, TransactionReceipt, H256, Address, U256},
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
};
use std::sync::Arc;
use tracing::info;

use crate::contract_executor::{ContractArbExecutor, ArbParamsBuilder};
use crate::bindings::avaxarbexecutor::ArbParams;

/// 套利执行动作类型
#[derive(Debug, Clone)]
pub enum ArbAction {
    /// 直接交易执行
    DirectTx(TypedTransaction),
    /// 合约套利执行（自有资金）
    ContractArb {
        token_in: Address,
        amount_in: U256,
        swap_path: Vec<(Address, U256, U256)>, // (pair, amount0_out, amount1_out)
        profit_token: Address,
        min_profit: U256,
        use_flash: bool,
    },
}

pub struct PublicTxExecutor {
    client: SignerMiddleware<Provider<Http>, LocalWallet>,
}

impl PublicTxExecutor {
    pub async fn new(rpc_url: &str, private_key: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let wallet: LocalWallet = private_key.parse()?;
        let client = SignerMiddleware::new(provider, wallet);
        
        Ok(Self { client })
    }

    pub async fn execute_tx(&self, tx: TypedTransaction) -> Result<TransactionReceipt> {
        let pending_tx = self.client.send_transaction(tx, None).await?;
        let receipt = pending_tx.await?;
        
        match receipt {
            Some(receipt) => Ok(receipt),
            None => eyre::bail!("Transaction failed to get receipt"),
        }
    }
}

#[async_trait]
impl Executor<TypedTransaction> for PublicTxExecutor {
    fn name(&self) -> &str {
        "AvaxPublicTxExecutor"
    }

    async fn execute(&self, action: TypedTransaction) -> Result<()> {
        let receipt = self.execute_tx(action).await?;
        let tx_hash = receipt.transaction_hash;

        info!(
            tx_hash = ?tx_hash,
            status = ?receipt.status,
            gas_used = ?receipt.gas_used,
            "Executed AVAX transaction"
        );
        
        Ok(())
    }
}

/// 增强的套利执行器，支持合约和直接交易
pub struct EnhancedArbExecutor {
    client: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    contract_executor: Option<ContractArbExecutor<SignerMiddleware<Provider<Http>, LocalWallet>>>,
}

impl EnhancedArbExecutor {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: Option<Address>) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let wallet: LocalWallet = private_key.parse()?;
        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        
        let contract_executor = match contract_address {
            Some(addr) => Some(ContractArbExecutor::new(addr, client.clone())),
            None => None,
        };
        
        Ok(Self { client, contract_executor })
    }
    
    /// 执行套利动作
    pub async fn execute_arb_action(&self, action: ArbAction) -> Result<TransactionReceipt> {
        match action {
            ArbAction::DirectTx(tx) => {
                let pending_tx = self.client.send_transaction(tx, None).await?;
                let receipt = pending_tx.await?;
                receipt.ok_or_else(|| eyre::eyre!("交易执行失败"))
            },
            ArbAction::ContractArb {
                token_in,
                amount_in,
                swap_path,
                profit_token,
                min_profit,
                use_flash,
            } => {
                let contract_executor = self.contract_executor
                    .as_ref()
                    .ok_or_else(|| eyre::eyre!("合约执行器未初始化"))?;
                
                let mut builder = ArbParamsBuilder::new(token_in, amount_in, profit_token)
                    .min_profit(min_profit);
                
                // 构建交换路径
                for (pair, amount0_out, amount1_out) in swap_path {
                    builder = builder.add_v2_swap(pair, amount0_out, amount1_out);
                }
                
                let params = builder.build();
                
                if use_flash {
                    contract_executor.execute_arb_with_flash(params).await
                } else {
                    contract_executor.execute_arb(params).await
                }
            }
        }
    }
}

#[async_trait]
impl Executor<ArbAction> for EnhancedArbExecutor {
    fn name(&self) -> &str {
        "EnhancedAvaxArbExecutor"
    }

    async fn execute(&self, action: ArbAction) -> Result<()> {
        let receipt = self.execute_arb_action(action.clone()).await?;
        let tx_hash = receipt.transaction_hash;

        match action {
            ArbAction::DirectTx(_) => {
                info!(
                    tx_hash = ?tx_hash,
                    gas_used = ?receipt.gas_used,
                    "执行直接交易"
                );
            },
            ArbAction::ContractArb { profit_token, min_profit, use_flash, .. } => {
                info!(
                    tx_hash = ?tx_hash,
                    gas_used = ?receipt.gas_used,
                    profit_token = ?profit_token,
                    min_profit = %min_profit,
                    use_flash = use_flash,
                    "执行合约套利"
                );
            }
        }
        
        Ok(())
    }
}
