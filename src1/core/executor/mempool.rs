//! 内存池执行器实现

use super::{traits::TransactionExecutor, types::ExecutionResult};
use crate::core::types::{ArbitragePath, BotError, Result};
use async_trait::async_trait;
use ethers::{
    prelude::*,
    types::{H256, U256, TransactionRequest},
};
use std::sync::Arc;
use log::{info, error, debug, warn};

/// 内存池执行器 - 直接提交交易到公共内存池
pub struct MempoolExecutor {
    name: String,
    provider: Arc<Provider<Http>>,
    wallet: LocalWallet,
    chain_id: u64,
    gas_price_strategy: GasPriceStrategy,
}

/// Gas价格策略
#[derive(Debug, Clone)]
pub enum GasPriceStrategy {
    /// 固定Gas价格
    Fixed(U256),
    /// 动态Gas价格（基于网络当前价格的倍数）
    Dynamic(f64),
    /// 竞价模式（比最高交易高出指定百分比）
    Competitive(f64),
}

impl MempoolExecutor {
    /// 创建新的内存池执行器
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        chain_id: u64,
    ) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| BotError::Connection(format!("Failed to connect to RPC: {}", e)))?;
        
        let wallet = private_key.parse::<LocalWallet>()
            .map_err(|e| BotError::ConfigError(format!("Invalid private key: {}", e)))?
            .with_chain_id(chain_id);

        Ok(Self {
            name: format!("MempoolExecutor-{}", chain_id),
            provider: Arc::new(provider),
            wallet,
            chain_id,
            gas_price_strategy: GasPriceStrategy::Dynamic(1.1), // 默认比当前价格高10%
        })
    }
    
    /// 设置Gas价格策略
    pub fn with_gas_price_strategy(mut self, strategy: GasPriceStrategy) -> Self {
        self.gas_price_strategy = strategy;
        self
    }
    
    /// 获取当前Gas价格
    async fn get_gas_price(&self) -> Result<U256> {
        match &self.gas_price_strategy {
            GasPriceStrategy::Fixed(price) => Ok(*price),
            GasPriceStrategy::Dynamic(multiplier) => {
                let base_price = self.provider.get_gas_price().await
                    .map_err(|e| BotError::RpcError(format!("Failed to get gas price: {}", e)))?;
                let adjusted_price = (base_price.as_u128() as f64 * multiplier) as u128;
                Ok(U256::from(adjusted_price))
            }
            GasPriceStrategy::Competitive(percentage) => {
                // 这里应该获取内存池中相关交易的最高Gas价格
                // 简化实现：使用当前价格 + 百分比
                let base_price = self.provider.get_gas_price().await
                    .map_err(|e| BotError::RpcError(format!("Failed to get gas price: {}", e)))?;
                let competitive_price = (base_price.as_u128() as f64 * (1.0 + percentage)) as u128;
                Ok(U256::from(competitive_price))
            }
        }
    }
    
    /// 构建套利交易
    async fn build_arbitrage_transaction(&self, path: &ArbitragePath) -> Result<TransactionRequest> {
        // 这里需要构建实际的套利合约调用
        // 简化实现：创建一个占位符交易
        warn!("⚠️ 套利合约调用尚未实现，使用占位符交易");
        
        let gas_price = self.get_gas_price().await?;
        let nonce = self.provider.get_transaction_count(self.wallet.address(), None).await
            .map_err(|e| BotError::RpcError(format!("Failed to get nonce: {}", e)))?;
        
        // 创建占位符交易（实际应该调用套利合约）
        let tx = TransactionRequest::new()
            .to(self.wallet.address()) // 占位符：发送给自己
            .value(U256::zero())
            .gas(path.gas_estimate)
            .gas_price(gas_price)
            .nonce(nonce)
            .chain_id(self.chain_id);
        
        info!("构建套利交易:");
        info!("  Gas限制: {}", path.gas_estimate);
        info!("  Gas价格: {} gwei", gas_price.as_u128() / 1_000_000_000);
        info!("  Nonce: {}", nonce);
        info!("  预期利润: {} wei", path.net_profit);
        
        Ok(tx)
    }
}

#[async_trait]
impl TransactionExecutor for MempoolExecutor {
    async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult> {
        info!("🚀 开始执行套利交易到公共内存池");
        info!("执行器: {}", self.name);
        
        // 构建交易
        let tx = self.build_arbitrage_transaction(path).await?;
        
        // 模拟执行（如果需要）
        debug!("模拟交易执行...");
        // let call_result = self.provider.call(&tx, None).await
        //     .map_err(|e| BotError::SimulationError(format!("Transaction simulation failed: {}", e)))?;
        
        // 签名并发送交易
        info!("签名并发送交易...");
        let signed_tx = self.wallet.sign_transaction(&tx.into()).await
            .map_err(|e| BotError::TransactionError(format!("Failed to sign transaction: {}", e)))?;
        
        info!("⚠️ 注意：为了安全起见，实际发送交易的代码被注释");
        info!("如需真实执行，请取消注释以下代码并确保充分测试");
        
        // 实际发送交易（当前被注释以防止意外执行）
        /*
        let pending_tx = self.provider.send_raw_transaction(signed_tx.rlp()).await
            .map_err(|e| BotError::TransactionError(format!("Failed to send transaction: {}", e)))?;
        
        info!("✅ 交易已发送，等待确认...");
        info!("交易哈希: {:?}", pending_tx.tx_hash());
        
        // 等待交易确认
        let receipt = pending_tx.await
            .map_err(|e| BotError::TransactionError(format!("Transaction failed: {}", e)))?;
        
        if let Some(receipt) = receipt {
            let actual_gas = receipt.gas_used.unwrap_or_default();
            let gas_price = tx.gas_price().unwrap_or_default();
            let gas_cost = actual_gas * gas_price;
            let actual_profit = if path.expected_profit > gas_cost {
                path.expected_profit - gas_cost
            } else {
                U256::zero()
            };
            
            info!("🎉 套利交易执行成功!");
            info!("交易哈希: {:?}", receipt.transaction_hash);
            info!("Gas使用: {} / {}", actual_gas, path.gas_estimate);
            info!("实际利润: {} wei", actual_profit);
            
            Ok(ExecutionResult::success(
                receipt.transaction_hash,
                actual_gas,
                actual_profit,
                self.name.clone(),
            ))
        } else {
            Err(BotError::TransactionError("交易收据为空".to_string()))
        }
        */
        
        // 模拟成功结果（用于测试）
        let mock_tx_hash = H256::random();
        info!("🔍 模拟模式：交易未实际发送");
        info!("模拟交易哈希: {:?}", mock_tx_hash);
        
        Ok(ExecutionResult::success(
            mock_tx_hash,
            path.gas_estimate,
            path.net_profit,
            self.name.clone(),
        ))
    }
    
    async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256> {
        debug!("估算套利交易Gas消耗");
        
        // 构建交易用于估算
        let tx = self.build_arbitrage_transaction(path).await?;
        
        // 实际Gas估算（当前被注释）
        /*
        let estimated_gas = self.provider.estimate_gas(&tx, None).await
            .map_err(|e| BotError::RpcError(format!("Gas estimation failed: {}", e)))?;
        */
        
        // 使用路径中的估算值（简化实现）
        let estimated_gas = path.gas_estimate;
        
        debug!("Gas估算结果: {} gas", estimated_gas);
        Ok(estimated_gas)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        // 检查RPC连接是否正常
        match self.provider.get_block_number().await {
            Ok(_) => {
                debug!("内存池执行器可用");
                true
            }
            Err(e) => {
                warn!("内存池执行器不可用: {}", e);
                false
            }
        }
    }
}

impl MempoolExecutor {
    /// 获取账户余额
    pub async fn get_balance(&self) -> Result<U256> {
        self.provider.get_balance(self.wallet.address(), None).await
            .map_err(|e| BotError::RpcError(format!("Failed to get balance: {}", e)))
    }
    
    /// 获取账户地址
    pub fn get_address(&self) -> Address {
        self.wallet.address()
    }
    
    /// 获取当前nonce
    pub async fn get_nonce(&self) -> Result<U256> {
        self.provider.get_transaction_count(self.wallet.address(), None).await
            .map_err(|e| BotError::RpcError(format!("Failed to get nonce: {}", e)))
    }
}
