//! 策略trait定义

use crate::core::types::{ArbitrageOpportunity, Transaction, BotError};
use async_trait::async_trait;
use super::stats::StrategyStats;

/// 策略特征，定义了所有策略必须实现的接口
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 策略名称
    fn name(&self) -> &str;
    
    /// 处理新的交易，寻找机会
    async fn process_transaction(&mut self, tx: &Transaction) -> Result<Vec<ArbitrageOpportunity>, BotError>;
    
    /// 启动策略
    async fn start(&mut self) -> Result<(), BotError>;
    
    /// 停止策略
    async fn stop(&mut self) -> Result<(), BotError>;
    
    /// 获取策略统计信息
    fn get_stats(&self) -> StrategyStats;
}
