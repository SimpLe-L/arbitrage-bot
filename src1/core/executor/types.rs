//! 执行器相关类型定义

use ethers::types::{H256, U256};
use serde::{Deserialize, Serialize};

/// 交易执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 执行是否成功
    pub success: bool,
    /// 交易哈希（如果成功）
    pub transaction_hash: Option<H256>,
    /// 实际消耗的gas
    pub gas_used: Option<U256>,
    /// 实际获得的利润
    pub actual_profit: Option<U256>,
    /// 错误消息（如果失败）
    pub error_message: Option<String>,
    /// 执行时间戳
    pub timestamp: u64,
    /// 执行器名称
    pub executor_name: String,
}

impl ExecutionResult {
    /// 创建成功的执行结果
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
    
    /// 创建失败的执行结果
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

/// 执行器状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorStatus {
    /// 执行器名称
    pub name: String,
    /// 是否可用
    pub available: bool,
    /// 最后执行时间
    pub last_execution: Option<u64>,
    /// 成功执行次数
    pub success_count: u64,
    /// 失败执行次数
    pub failure_count: u64,
    /// 总消耗gas
    pub total_gas_used: U256,
    /// 总利润
    pub total_profit: U256,
}

impl ExecutorStatus {
    pub fn new(name: String) -> Self {
        Self {
            name,
            available: true,
            last_execution: None,
            success_count: 0,
            failure_count: 0,
            total_gas_used: U256::zero(),
            total_profit: U256::zero(),
        }
    }
    
    /// 更新执行结果统计
    pub fn update_with_result(&mut self, result: &ExecutionResult) {
        self.last_execution = Some(result.timestamp);
        
        if result.success {
            self.success_count += 1;
            if let Some(gas) = result.gas_used {
                self.total_gas_used += gas;
            }
            if let Some(profit) = result.actual_profit {
                self.total_profit += profit;
            }
        } else {
            self.failure_count += 1;
        }
    }
    
    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            self.success_count as f64 / total as f64
        }
    }
}
