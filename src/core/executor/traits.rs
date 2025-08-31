//! 执行器trait定义

use crate::core::types::{ArbitragePath, Result};
use super::types::ExecutionResult;
use async_trait::async_trait;
use ethers::types::U256;

/// 交易执行器trait
#[async_trait]
pub trait TransactionExecutor: Send + Sync {
    /// 执行套利交易
    async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult>;
    
    /// 估算交易gas费用
    async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256>;
    
    /// 获取执行器名称
    fn name(&self) -> &str;
    
    /// 检查执行器是否可用
    async fn is_available(&self) -> bool;
}
