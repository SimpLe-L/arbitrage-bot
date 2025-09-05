//! 执行器管理器

use super::{traits::TransactionExecutor, types::ExecutionResult};
use crate::core::types::{ArbitragePath, BotError, Result};
use ethers::types::U256;
use log::info;

/// 执行器管理器
pub struct ExecutorManager {
    executors: Vec<Box<dyn TransactionExecutor>>,
    current_executor_index: usize,
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            executors: Vec::new(),
            current_executor_index: 0,
        }
    }
    
    /// 添加执行器
    pub fn add_executor(&mut self, executor: Box<dyn TransactionExecutor>) {
        info!("添加执行器: {}", executor.name());
        self.executors.push(executor);
    }
    
    /// 获取当前执行器
    pub fn get_current_executor(&self) -> Option<&dyn TransactionExecutor> {
        self.executors.get(self.current_executor_index)
            .map(|e| e.as_ref())
    }
    
    /// 切换到下一个可用的执行器
    pub async fn switch_to_next_available(&mut self) -> Result<()> {
        for i in 0..self.executors.len() {
            let index = (self.current_executor_index + i + 1) % self.executors.len();
            if self.executors[index].is_available().await {
                self.current_executor_index = index;
                info!("切换到执行器: {}", self.executors[index].name());
                return Ok(());
            }
        }
        
        Err(BotError::Unknown("没有可用的执行器".to_string()))
    }
    
    /// 执行套利交易
    pub async fn execute_arbitrage(&self, path: &ArbitragePath) -> Result<ExecutionResult> {
        if let Some(executor) = self.get_current_executor() {
            if !executor.is_available().await {
                return Err(BotError::Unknown(
                    format!("执行器 {} 不可用", executor.name())
                ));
            }
            
            executor.execute_arbitrage(path).await
        } else {
            Err(BotError::Unknown("没有可用的执行器".to_string()))
        }
    }
    
    /// 估算gas费用
    pub async fn estimate_gas(&self, path: &ArbitragePath) -> Result<U256> {
        if let Some(executor) = self.get_current_executor() {
            executor.estimate_gas(path).await
        } else {
            Err(BotError::Unknown("没有可用的执行器".to_string()))
        }
    }
    
    /// 获取所有执行器的状态
    pub async fn get_status(&self) -> Vec<(String, bool)> {
        let mut status = Vec::new();
        for executor in &self.executors {
            let available = executor.is_available().await;
            status.push((executor.name().to_string(), available));
        }
        status
    }
    
    /// 获取执行器数量
    pub fn executor_count(&self) -> usize {
        self.executors.len()
    }
    
    /// 获取当前执行器索引
    pub fn current_index(&self) -> usize {
        self.current_executor_index
    }
}

impl Default for ExecutorManager {
    fn default() -> Self {
        Self::new()
    }
}
