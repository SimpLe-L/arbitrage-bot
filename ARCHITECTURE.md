# AVAX MEV 套利机器人项目架构文档

## 项目概述

这是一个基于Rust构建的AVAX链MEV（最大可提取价值）套利机器人，专门用于监听内存池交易并识别多跳套利机会。机器人采用事件驱动架构，支持实时监听、路径计算、本地模拟和执行管理。

### 核心特性

- **实时监听**: 通过WebSocket监听AVAX链的区块和内存池交易
- **多跳套利**: 支持最多3跳的套利路径发现
- **本地模拟**: 集成Foundry进行交易模拟
- **风险管理**: 支持滑点保护、最小利润阈值等风险控制
- **模块化设计**: 清晰的模块分离，易于维护和扩展

## 项目结构

```
arbitrage-bot/
├── Cargo.toml                 # 项目配置和依赖
├── .env                       # 环境变量配置（敏感信息）
├── note.md                    # 项目需求和说明
├── ARCHITECTURE.md            # 项目架构文档（本文件）
└── src/                       # 源代码目录
    ├── main.rs                # 程序入口点
    ├── core/                  # 核心模块
    │   ├── mod.rs            # 核心模块导出
    │   ├── types.rs          # 核心数据结构定义
    │   ├── engine.rs         # MEV引擎核心逻辑
    │   ├── messages/         # 消息系统
    │   │   ├── mod.rs       # 事件总线和处理器
    │   │   ├── block.rs     # 区块监听器
    │   │   ├── mempool.rs   # 内存池监听器
    │   │   └── log_filter.rs # DEX日志过滤器
    │   ├── executor/         # 执行器模块
    │   │   ├── mod.rs       # 执行器管理
    │   │   ├── flashbot.rs  # Flashbot执行器
    │   │   └── mempool.rs   # 内存池执行器
    │   └── utilities/        # 核心工具
    │       ├── mod.rs       
    │       └── state_override.rs
    ├── strategy/             # 策略模块
    │   ├── mod.rs           # 策略接口定义
    │   ├── config/          # 配置管理
    │   │   └── mod.rs       # AVAX和DEX配置
    │   ├── arb/             # 套利策略
    │   │   └── mod.rs       # 套利路径发现和处理
    │   ├── bot/             # 机器人控制（预留）
    │   ├── contract/        # 合约相关（预留）
    │   ├── simulator/       # 模拟器（预留）
    │   └── utils/           # 策略工具（预留）
    └── utils/               # 通用工具模块
        └── mod.rs           # 工具函数集合
```

## 核心架构

### 1. 事件驱动架构

机器人基于事件驱动架构设计，主要组件通过事件总线进行通信：

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   区块监听器     │    │   内存池监听器   │    │  DEX日志过滤器  │
│ BlockCollector  │    │MempoolCollector │    │LogFilterCollector│
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼──────────────┐
                    │        事件总线            │
                    │       EventBus           │
                    └─────────────┬──────────────┘
                                  │
                    ┌─────────────▼──────────────┐
                    │      套利处理器            │
                    │   ArbitrageHandler       │
                    └─────────────┬──────────────┘
                                  │
                    ┌─────────────▼──────────────┐
                    │      执行器管理器          │
                    │    ExecutorManager       │
                    └──────────────────────────────┘
```

### 2. 数据流

```
WebSocket(AVAX节点) → 消息收集器 → 事件总线 → 套利处理器 → 路径计算 → 模拟验证 → 执行管理器 → 区块链
                    ↓
                过滤和解析
                    ↓
              交易/区块/日志事件
                    ↓
               套利机会识别
                    ↓
              多跳路径搜索(BFS)
                    ↓
               利润计算和验证
                    ↓
              Foundry本地模拟
                    ↓
            打印结果/提交交易
```

## 模块详细说明

### Core模块 (`src/core/`)

#### 1. 类型定义 (`types.rs`)

定义了机器人使用的核心数据结构：

- **Transaction**: 交易信息结构
- **Block**: 区块信息结构  
- **Token**: 代币信息结构
- **Pool**: 流动性池结构
- **ArbitragePath**: 套利路径结构
- **ArbitrageOpportunity**: 套利机会结构
- **BotError**: 统一错误类型

#### 2. MEV引擎 (`engine.rs`)

MEV引擎是整个系统的核心协调器：

**主要功能**:
- 组件生命周期管理
- 系统状态监控
- 统计信息收集
- 错误处理和恢复

**关键接口**:
```rust
impl MEVEngine {
    pub async fn new(config, event_bus, executor) -> Result<Self>
    pub async fn start(&mut self) -> Result<()>
    pub async fn stop(&mut self) -> Result<()>
    pub async fn restart(&mut self) -> Result<()>
    pub async fn get_status(&self) -> BotStatus
    pub async fn get_statistics(&self) -> BotStatistics
}
```

#### 3. 消息系统 (`messages/`)

**事件总线** (`mod.rs`):
- 异步事件分发机制
- 支持多个事件处理器注册
- 线程安全的事件传递

**区块收集器** (`block.rs`):
- 监听新区块事件
- 区块数据解析和验证
- 支持区块重组处理

**内存池收集器** (`mempool.rs`):
- 实时监听mempool交易
- 交易过滤（gas价格、地址、合约调用）
- 交易分类和路由

**DEX日志过滤器** (`log_filter.rs`):
- 监听DEX相关事件日志
- 支持Uniswap V2/V3、ERC20转账事件
- 事件解析和结构化

#### 4. 执行器 (`executor/`)

**执行器管理器** (`mod.rs`):
- 统一的交易执行接口
- 支持多种执行策略
- 执行结果跟踪

**执行器类型**:
- **MockExecutor**: 模拟执行器，用于测试
- **PrintExecutor**: 打印执行器，输出交易信息
- **FlashbotExecutor**: Flashbot执行器（预留）
- **MempoolExecutor**: 内存池执行器（预留）

### Strategy模块 (`src/strategy/`)

#### 1. 配置管理 (`config/mod.rs`)

**BotConfig结构**:
```rust
pub struct BotConfig {
    pub avax: AvaxConfig,           // AVAX网络配置
    pub dexes: HashMap<String, DexConfig>, // DEX配置
    pub bot: BotParameters,         // 机器人参数
    pub notifications: NotificationConfig, // 通知配置
}
```

**支持的DEX**:
- Trader Joe (AVAX生态主要DEX)
- Pangolin (AVAX原生DEX)
- SushiSwap (多链DEX)

#### 2. 套利策略 (`arb/mod.rs`)

**套利路径发现器** (`ArbitragePathFinder`):
- 使用BFS算法搜索多跳路径
- 支持最多3跳的复杂路径
- 路径优化和去重

**套利处理器** (`ArbitrageHandler`):
- 监听交易事件
- 套利机会识别
- 利润计算和验证
- 执行决策

**核心算法**:
```rust
// 路径搜索伪代码
fn find_arbitrage_paths(start_token, end_token, max_hops) -> Vec<ArbitragePath> {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    
    // BFS遍历所有可能路径
    // 计算每条路径的预期利润
    // 返回利润超过阈值的路径
}
```

### Utils模块 (`src/utils/`)

提供各种通用工具函数：

#### 1. 地址工具 (`address_utils`)
- 地址格式验证
- 安全地址解析
- 零地址检查

#### 2. 数值计算 (`math_utils`)
- 滑点计算
- 百分比计算
- Wei/Ether转换
- 精度处理

#### 3. 时间工具 (`time_utils`)
- 时间戳处理
- 时间范围验证
- 格式化显示

#### 4. 验证工具 (`validation`)
- 交易哈希验证
- 私钥格式验证
- URL格式验证

#### 5. 性能监控 (`performance`)
- 执行时间统计
- 性能日志记录

## 配置说明

### 环境变量配置 (`.env`)

```bash
# AVAX网络配置
AVAX_WS_URL=wss://api.avax.network/ext/bc/C/ws
AVAX_RPC_URL=https://api.avax.network/ext/bc/C/rpc

# 账户配置
PRIVATE_KEY=0x...

# Flashbot配置
FLASHBOT_RELAY_URL=https://relay.flashbots.net

# 日志级别
RUST_LOG=info
```

### 机器人参数配置

```rust
pub struct BotParameters {
    pub max_hops: usize,                    // 最大跳数 (默认: 3)
    pub min_profit_threshold: f64,          // 最小利润阈值 (默认: 0.01 AVAX)
    pub max_slippage_bps: u64,             // 最大滑点 (默认: 100 = 1%)
    pub simulation_mode: bool,              // 模拟模式 (默认: true)
    pub gas_limit: u64,                    // Gas限制 (默认: 500000)
    pub max_gas_price_gwei: u64,           // 最大gas价格 (默认: 30)
}
```

## 部署和运行

### 1. 环境准备

```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装Foundry (用于本地模拟)
curl -L https://foundry.paradigm.xyz | bash
foundryup

# 克隆项目
git clone <repository_url>
cd arbitrage-bot
```

### 2. 配置设置

```bash
# 复制环境变量模板
cp .env.example .env

# 编辑配置文件，填入必要信息
vim .env
```

### 3. 编译和运行

```bash
# 编译项目
cargo build --release

# 运行测试
cargo test

# 启动机器人 (模拟模式)
RUST_LOG=info cargo run

# 启动机器人 (实际模式)
SIMULATION_MODE=false RUST_LOG=info cargo run
```

## 监控和日志

### 1. 日志输出

机器人提供详细的运行日志：

```bash
2024-08-30 20:30:00 INFO  启动AVAX MEV套利机器人...
2024-08-30 20:30:01 INFO  配置加载完成: AvaxConfig { chain_id: 43114, ... }
2024-08-30 20:30:01 INFO  事件总线创建完成
2024-08-30 20:30:02 INFO  MEV引擎启动完成
2024-08-30 20:30:02 INFO  AVAX MEV套利机器人启动完成!
2024-08-30 20:30:02 INFO  支持的DEX: ["trader_joe", "pangolin", "sushiswap"]
2024-08-30 20:30:02 INFO  最大跳数: 3
2024-08-30 20:30:02 INFO  最小利润阈值: 0.01 AVAX
```

### 2. 性能指标

系统每分钟输出性能统计：

```bash
2024-08-30 20:31:00 INFO  机器人状态: Running | 处理交易: 1254 | 发现机会: 12 | 执行成功: 8 | 总利润: 0.2450 AVAX
```

### 3. 错误处理

- 自动重连机制
- 错误恢复和重试
- 详细错误日志记录

## 扩展指南

### 1. 添加新的DEX

1. 在`DexConfig`中添加新的DEX配置
2. 在`LogFilterCollector`中添加DEX特定的事件过滤器
3. 在`ArbitragePathFinder`中添加新的池查询逻辑

### 2. 实现闪电贷

1. 创建`FlashloanExecutor`执行器
2. 实现闪电贷合约接口
3. 在套利策略中集成闪电贷逻辑

### 3. 添加新的执行策略

1. 实现`Executor` trait
2. 在`ExecutorManager`中注册新执行器
3. 在配置中添加执行策略选择

## 风险提示

### 1. 技术风险
- 网络延迟可能影响套利成功率
- 区块重组可能导致交易失败
- 智能合约风险

### 2. 经济风险
- 滑点风险
- 前置交易（Front-running）
- Gas费用波动

### 3. 监管风险
- MEV相关的监管政策变化
- 交易所政策调整

## 测试指南

### 1. 单元测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test core::types
cargo test strategy::arb
```

### 2. 集成测试

```bash
# 启动本地测试网络
anvil --chain-id 43114

# 运行集成测试
cargo test --test integration
```

### 3. 性能测试

```bash
# 压力测试
cargo test --release stress_test

# 内存泄漏测试
valgrind --leak-check=full cargo run
```

## 贡献指南

### 1. 代码规范
- 遵循Rust官方代码风格
- 使用`cargo fmt`格式化代码
- 使用`cargo clippy`检查代码质量

### 2. 提交流程
- Fork项目到个人仓库
- 创建功能分支
- 提交Pull Request
- 通过代码审查和测试

### 3. 文档更新
- 更新相关文档
- 添加必要的注释
- 更新CHANGELOG

---

**版本**: v0.1.0  
**最后更新**: 2024-08-30  
**维护者**: MEV Bot Development Team
