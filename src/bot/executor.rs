use async_trait::async_trait;
use infra::Executor;
use eyre::Result;
use ethers::{
    infra::types::{transaction::eip2718::TypedTransaction, TransactionReceipt, H256},
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
};
use tracing::info;

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
