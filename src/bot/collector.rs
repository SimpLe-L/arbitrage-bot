use crate::engine::{async_trait, Collector, CollectorStream};
use eyre::Result;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use ethers::types::Transaction;
use tokio::pin;
use tracing::{debug, error};

use crate::types::Event;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TxMessage {
    pub result: Option<Transaction>,
}

pub struct AvaxMempoolCollector {
    ws_url: String,
}

impl AvaxMempoolCollector {
    pub fn new(ws_url: &str) -> Self {
        Self {
            ws_url: ws_url.to_string(),
        }
    }
}

#[async_trait]
impl Collector<Event> for AvaxMempoolCollector {
    fn name(&self) -> &str {
        "AvaxMempoolCollector"
    }

    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.ws_url)
            .await
            .expect("Failed to connect to AVAX WebSocket");

        // 订阅pending交易
        let (mut sink, read) = ws_stream.split();
        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newPendingTransactions", true]
        });
        
        futures::SinkExt::send(&mut sink, Message::Text(subscribe_msg.to_string())).await
            .expect("Failed to send subscription");

        let stream = async_stream::stream! {
            pin!(read);
            while let Some(message) = read.next().await {
                let message = match message {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("WebSocket error: {:?}", e);
                        continue;
                    }
                };

                if let Ok(text) = message.to_text() {
                    if let Ok(value) = serde_json::from_str::<Value>(text) {
                        // 解析订阅通知
                        if let Some(params) = value.get("params") {
                            if let Some(result) = params.get("result") {
                                if let Ok(tx) = serde_json::from_value::<Transaction>(result.clone()) {
                                    yield Event::PendingTx(tx);
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
