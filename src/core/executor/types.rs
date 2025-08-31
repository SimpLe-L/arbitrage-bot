//! 执行器相关类型定义

use ethers::types::{H256, U256};
use serde::{Deserialize, Serialize};
use crate::core::types::BotError;

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 交易哈希（如果成功提交）
    pub transaction_hash: Option<H256>,
    /// 实际使用的gas
    pub gas_used: Option<U256>,
    /// 实际利润
    pub actual_profit: Option<U256>,
    /// 错误信息（如果失败）
    pub error_message: Option<String>,
    /// 执行时间戳
    pub timestamp: u64,
    /// 执行器名称
    pub executor_name: String,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(
        transaction_hash: H256,
        gas_used: U256,
        actual_profit: U256,
        executor_name: String,
    ) -> Self {
        Self {
            success: true,
            transaction_hash: Some(transaction_hash),
            gas_used: Some(gas_used),
            actual_profit: Some(actual_profit),
            error_message: None,
            timestamp: chrono::Utc::now().timestamp() as u64,
            executor_name,
        }
    }
    
    /// 创建失败结果
    pub fn failure(error_message: String, executor_name: String) -> Self {
        Self {
            success: false,
            transaction_hash: None,
            gas_used: None,
            actual_profit: None,
            error_message: Some(error_message),
            timestamp: chrono::Utc::now().timestamp() as u64,
            executor_name,
        }
    }
}

/// 执行策略
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionStrategy {
    /// 直接发送到mempool
    Mempool,
    /// 通过Flashbots发送
    Flashbots,
    /// 仅模拟，不实际执行
    SimulationOnly,
    /// 仅打印信息
    PrintOnly,
}
