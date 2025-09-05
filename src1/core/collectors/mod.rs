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

/// 事件处理器trait
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理事件
    async fn handle_event(&mut self, event: Event) -> Result<()>;
    
    /// 获取处理器名称
    fn name(&self) -> &str;
}

/// 简单的事件总线
pub struct EventBus {
    collectors: Vec<Box<dyn Collector>>,
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
            handlers: Vec::new(),
        }
    }
    
    pub fn add_collector(&mut self, collector: Box<dyn Collector>) {
        self.collectors.push(collector);
    }
    
    pub fn add_handler(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
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
    
    /// 发送事件给所有处理器
    pub async fn send_event(&mut self, event: Event) -> Result<()> {
        for handler in &mut self.handlers {
            if let Err(e) = handler.handle_event(event.clone()).await {
                log::warn!("处理器 {} 处理事件失败: {}", handler.name(), e);
            }
        }
        Ok(())
    }
    
    /// 启动事件循环
    pub async fn run_event_loop(&mut self) -> Result<()> {
        use futures::StreamExt;
        
        log::info!("启动事件循环，收集器数量: {}, 处理器数量: {}", 
            self.collectors.len(), self.handlers.len());
        
        // 收集所有事件流
        let mut streams = Vec::new();
        for collector in &self.collectors {
            let stream = collector.get_event_stream().await?;
            streams.push(stream);
        }
        
        // 合并所有事件流
        let mut merged_stream = futures::stream::select_all(streams);
        
        // 处理事件
        while let Some(event) = merged_stream.next().await {
            self.send_event(event).await?;
        }
        
        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
