//! 数据收集器模块
//! 
//! 参考sui-mev的设计，提供简洁统一的数据收集接口

use async_trait::async_trait;
use crate::core::types::{Result, BotError};
use futures::Stream;
use std::pin::Pin;

pub mod block;
pub mod mempool;

pub use block::*;
pub use mempool::*;

/// 事件类型
#[derive(Debug, Clone)]
pub enum Event {
    /// 新区块事件
    NewBlock {
        block_number: u64,
        block_hash: String,
        timestamp: u64,
    },
    /// 新交易事件
    NewTransaction {
        hash: String,
        from: String,
        to: Option<String>,
        value: String,
        gas_price: String,
        data: Option<String>,
    },
    /// 系统事件
    System(SystemEvent),
}

/// 系统事件
#[derive(Debug, Clone)]
pub enum SystemEvent {
    Connected,
    Disconnected,
    Error(String),
    Shutdown,
}

/// 事件流类型
pub type EventStream = Pin<Box<dyn Stream<Item = Event> + Send>>;

/// 收集器trait - 参考sui-mev的Collector设计
#[async_trait]
pub trait Collector: Send + Sync {
    /// 收集器名称
    fn name(&self) -> &str;
    
    /// 获取事件流
    async fn get_event_stream(&self) -> Result<EventStream>;
    
    /// 启动收集器
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }
    
    /// 停止收集器
    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

/// 简单的事件总线
pub struct EventBus {
    collectors: Vec<Box<dyn Collector>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
        }
    }
    
    pub fn add_collector(&mut self, collector: Box<dyn Collector>) {
        self.collectors.push(collector);
    }
    
    pub async fn start(&mut self) -> Result<()> {
        for collector in &mut self.collectors {
            collector.start().await?;
        }
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<()> {
        for collector in &mut self.collectors {
            collector.stop().await?;
        }
        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
