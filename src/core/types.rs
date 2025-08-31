use ethers::types::{Address, Bytes, H256, U256, U64};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 交易数据
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: H256,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub data: Bytes,
    pub nonce: U256,
    pub block_number: Option<U64>,
    pub timestamp: Option<u64>,
}

/// 区块数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub number: U64,
    pub hash: H256,
    pub parent_hash: H256,
    pub timestamp: U256,
    pub transactions: Vec<Transaction>,
    pub gas_limit: U256,
    pub gas_used: U256,
    pub base_fee_per_gas: Option<U256>,
}

/// DEX 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexType {
    TraderJoe,
    Pangolin,
    Sushiswap,
    Uniswap,
}

impl fmt::Display for DexType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DexType::TraderJoe => write!(f, "Trader Joe"),
            DexType::Pangolin => write!(f, "Pangolin"),
            DexType::Sushiswap => write!(f, "Sushiswap"),
            DexType::Uniswap => write!(f, "Uniswap"),
        }
    }
}

/// 代币信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

/// 交易对池信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub address: Address,
    pub token0: Token,
    pub token1: Token,
    pub dex: DexType,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee: U256, // 费用基点 (比如 30 表示 0.3%)
}

/// 套利路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitragePath {
    pub input_token: Token,
    pub output_token: Token,
    pub pools: Vec<Pool>,
    pub amounts_in: Vec<U256>,
    pub amounts_out: Vec<U256>,
    pub expected_profit: U256,
    pub gas_estimate: U256,
    pub net_profit: U256,
}

impl fmt::Display for ArbitragePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "套利路径: {} → ", self.input_token.symbol)?;
        for (i, pool) in self.pools.iter().enumerate() {
            if i > 0 {
                write!(f, " → ")?;
            }
            let intermediate_token = if pool.token0.address == self.input_token.address {
                &pool.token1
            } else {
                &pool.token0
            };
            write!(f, "{} ({})", intermediate_token.symbol, pool.dex)?;
        }
        write!(f, " → {}", self.output_token.symbol)?;
        write!(f, "\n预期利润: {} wei", self.expected_profit)?;
        write!(f, "\nGas估算: {} wei", self.gas_estimate)?;
        write!(f, "\n净利润: {} wei", self.net_profit)
    }
}

/// 套利机会
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub trigger_tx: Transaction,
    pub paths: Vec<ArbitragePath>,
    pub best_path: ArbitragePath,
    pub confidence: f64, // 0.0 - 1.0
    pub timestamp: u64,
}

/// 模拟结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: U256,
    pub profit: U256,
    pub error_message: Option<String>,
}

/// MEV 机器人状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BotStatus {
    Starting,
    Running,
    Paused,
    Stopped,
    Error(String),
}

/// 统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotStatistics {
    pub total_opportunities_found: u64,
    pub successful_arbitrages: u64,
    pub failed_arbitrages: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub uptime_seconds: u64,
}

/// 配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub ws_url: String,
    pub private_key: String,
    pub min_profit_threshold: U256,
    pub max_gas_price: U256,
    pub slippage_tolerance: f64,
    pub max_hops: u8,
    pub simulation_enabled: bool,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
}

/// 错误类型
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("WebSocket connection failed: {0}")]
    WebSocketError(String),
    
    #[error("RPC call failed: {0}")]
    RpcError(String),
    
    #[error("Transaction failed: {0}")]
    TransactionError(String),
    
    #[error("Simulation failed: {0}")]
    SimulationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Path finding error: {0}")]
    PathFindingError(String),
    
    #[error("Insufficient profit: expected {expected}, got {actual}")]
    InsufficientProfit { expected: U256, actual: U256 },
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, BotError>;
