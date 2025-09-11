# AVAX MEV套利机器人

## MEV套利流程说明

经过分析和优化，正确的MEV套利流程如下：

### 1. 监听阶段 (Off-chain)
- 通过WebSocket连接监听AVAX mempool中的pending交易
- 分析交易是否可能创造套利机会（如大额交易可能影响DEX价格）
- 提取相关的代币地址和交易参数

### 2. 机会识别 (Off-chain)
- 基于pending交易分析潜在的价格变化
- 寻找跨DEX的价格差异套利路径
- 计算预期利润和所需的交易参数

### 3. 本地模拟 (Off-chain)
- 使用Foundry或HTTP模拟器克隆AVAX主网状态
- 模拟整个套利交易序列
- 验证利润计算和gas成本
- 确保交易可行性

### 4. 交易构建 (Off-chain)
- 构建原子套利交易
- 包含闪电贷逻辑（如果需要）
- 编码swap路径和参数
- 设置适当的gas和滑点保护

### 5. 交易提交 (On-chain)
- **关键澄清**: 合约中的逻辑只是交易的"计划"，需要通过交易提交到区块链才会真正执行
- 将构建好的交易提交到mempool或Flashbots
- 区块链矿工/验证者执行交易，此时合约中的swap才真正发生
- 获取执行结果和实际利润

### 流程图
```
Mempool监听 → 机会识别 → 本地模拟 → 交易构建 → 提交执行
     ↓            ↓          ↓          ↓          ↓
  PendingTx   → 套利路径  → 利润验证  → 交易数据  → 链上执行
```

## 项目架构

### 核心模块
- `collector/`: Mempool交易收集器
- `strategy/`: 套利策略和机会分析
- `simulator/`: 本地交易模拟
- `executor/`: 交易执行器
- `contract/`: 套利合约代码

### 数据流
```
AvaxMempoolCollector → ArbStrategy → HttpSimulator → EnhancedArbExecutor
```

## 配置说明

### 环境变量配置
复制 `.env.example` 为 `.env` 并填入实际值：

```bash
# 必需配置
AVAX_PRIVATE_KEY=your_private_key_here
AVAX_RPC_URL=https://api.avax.network/ext/bc/C/rpc
ARB_CONTRACT_ADDRESS=your_contract_address

# 可选配置
MAX_HOPS=2
MIN_PROFIT=10000000000000000  # 0.01 AVAX
WORKER_THREADS=8
```

### 使用方法

#### 启动套利机器人
```bash
cargo run --bin arbitrage-bot start-bot
```

#### 测试特定代币套利
```bash
cargo run --bin arbitrage-bot run --token-address 0xA7D7079b0FEaD91F3e65f86E8915Cb59c1a4C664
```

#### 使用合约套利
```bash
cargo run --bin arbitrage-bot contract-arb --contract-address YOUR_CONTRACT --token-address TOKEN_ADDR
```

## 安全注意事项

1. **私钥保护**: 私钥存储在 `.env` 文件中，确保不要提交到版本控制
2. **资金管理**: 设置合理的最大交易金额和最小利润阈值
3. **Gas优化**: 监控gas价格，避免在高gas时期执行小额套利
4. **合约安全**: 使用紧急提取功能，定期检查合约余额

## 常见问题

### Q: 为什么合约执行了还要提交交易？
A: 合约代码只是定义了执行逻辑，必须通过交易提交到区块链网络，由矿工执行后才会真正产生状态变化。

### Q: 如何提高套利成功率？
A: 1) 优化gas设置  2) 选择流动性好的代币对  3) 设置合理的滑点保护  4) 使用Flashbots避免抢跑

### Q: 支持哪些DEX？
A: 目前支持TraderJoe、Pangolin、SushiSwap等主流AVAX DEX

## 开发说明

### 添加新的DEX支持
1. 在 `dex/` 目录添加新的DEX适配器
2. 实现统一的交易接口
3. 更新路径发现逻辑

### 优化套利算法
1. 调整 `strategy/arb.rs` 中的搜索算法
2. 优化gas估算和利润计算
3. 改进风险控制机制

## 免责声明

本项目仅用于学习和研究目的。使用者需要：
- 充分理解MEV套利的风险
- 遵守相关法律法规
- 承担使用过程中的所有风险
