use crate::core::executor::{TransactionExecutor, ExecutionResult};
use crate::core::types::{ArbitragePath, BotError, Result};
use async_trait::async_trait;
use ethers::types::U256;

/// Mempool执行器 (简化实现)
pub struct MempoolExecutor {
    name: String,
    enabled: bool,
}

impl MempoolExecutor {
    pub fn new() -> Self {
        Self {
            name: "MempoolExecutor".to_string(),
            enabled: false, // 默认禁用，需要实际的RPC连接
        }
    }
}

#[async_trait]
impl TransactionExecutor for MempoolExecutor {
    async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult> {
        if !self.enabled {
            return Err(BotError::Unknown("Mempool执行器未启用".to_string()));
        }
        
        log::info!("通过Mempool执行套利交易 (未实现)");
        
        // TODO: 实现实际的mempool交易提交
        Err(BotError::Unknown("Mempool执行器未完全实现".to_string()))
    }
    
    async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256> {
        Ok(path.gas_estimate)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn is_available(&self) -> bool {
        self.enabled
    }
}
