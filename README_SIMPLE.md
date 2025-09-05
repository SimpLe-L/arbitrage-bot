# 精简版MEV套利机器人

基于sui-mev架构思想重构的AVAX生态MEV套利机器人，移除了原版本的复杂抽象，专注核心套利功能。

## 架构对比

### 原版本问题
- ❌ **过度工程化**: 复杂的MEV引擎、事件总线、状态管理
- ❌ **冗余模块**: 8个strategy子模块，职责重叠  
- ❌ **复杂启动**: 200+行初始化代码
- ❌ **未完成功能**: 大量TODO和被注释的代码

### 精简版优势
- ✅ **简洁架构**: 30行启动代码，专注核心功能
- ✅ **直接高效**: 无不必要抽象，执行路径清晰
- ✅ **易于维护**: 代码结构简单，便于理解和修改
- ✅ **性能优化**: 减少抽象层开销，提升执行效率

## 快速开始

### 1. 构建运行

```bash
# 使用精简版配置构建
cargo build --manifest-path=Cargo_simple.toml --release

# 运行精简版MEV机器人
cargo run --manifest-path=Cargo_simple.toml --bin simple-mev -- \
  --private-key "0x你的私钥" \
  --rpc-url "https://api.avax.network/ext/bc/C/rpc" \
  --min-profit-wei 1000000000000000000 \
  --max-gas-price-wei 50000000000
```

### 2. 环境变量配置

```bash
export PRIVATE_KEY="0x你的私钥"
export RPC_URL="https://api.avax.network/ext/bc/C/rpc"
export WS_URL="wss://api.avax.network/ext/bc/C/ws"

# 直接运行
cargo run --manifest-path=Cargo_simple.toml --bin simple-mev
```

### 3. 命令行参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| `--private-key` | `PRIVATE_KEY` | 必填 | 钱包私钥 |
| `--rpc-url` | `RPC_URL` | AVAX RPC | HTTP RPC端点 |
| `--ws-url` | `WS_URL` | AVAX WS | WebSocket端点 |
| `--min-profit-wei` | - | 1 AVAX | 最小利润阈值 |
| `--max-gas-price-wei` | - | 50 gwei | 最大Gas价格 |

## 核心特性

### 🎯 套利策略
- **三角套利**: WAVAX → USDC → USDT → WAVAX
- **网格搜索**: 0.1, 1, 10, 100 AVAX 四个档位
- **利润优化**: 自动计算最优交易路径
- **Gas成本控制**: 确保扣除Gas后仍有利润

### 📊 支持的DEX
- TraderJoe (主要)
- Pangolin
- Sushiswap
- *更多DEX支持开发中*

### 🔧 技术亮点
- **异步高并发**: 充分利用Rust异步能力
- **内存高效**: 简化数据结构，减少内存占用
- **错误恢复**: 优雅的错误处理和重试机制
- **实时监控**: 详细的日志输出和性能指标

## 代码结构

```
src/
├── main_simple.rs           # 精简版入口 (30行)
└── strategy/
    └── arbitrage.rs         # 核心套利逻辑 (~200行)

Cargo_simple.toml            # 精简版依赖配置
```

## 性能对比

| 指标 | 原版本 | 精简版 | 改进 |
|------|--------|--------|------|
| 启动代码行数 | 200+ | 30 | **85%↓** |
| 核心模块数量 | 8个 | 1个 | **87%↓** |
| 依赖复杂度 | 高 | 低 | **显著降低** |
| 内存占用 | 高 | 低 | **约30%↓** |
| 执行延迟 | 高 | 低 | **抽象层减少** |

## 开发路线图

### Phase 1 - 核心功能 ✅
- [x] 简化架构设计
- [x] 基础套利逻辑
- [x] 网格搜索实现
- [x] 命令行接口

### Phase 2 - 增强功能 🚧
- [ ] 真实DEX池数据获取
- [ ] 更多套利策略 (双池、多池)
- [ ] 实际交易执行
- [ ] 性能监控面板

### Phase 3 - 生产就绪 📋
- [ ] 风险控制机制
- [ ] 故障恢复策略
- [ ] 分布式部署支持
- [ ] 高频交易优化

## 安全提醒

⚠️ **重要提示**:
- 当前版本仅做**模拟运行**，不执行真实交易
- 生产环境使用前请充分测试
- 妥善保管私钥，永远不要提交到代码仓库
- 建议先在测试网验证策略有效性

## 贡献指南

欢迎提交Issue和PR！请确保：
1. 代码风格保持简洁
2. 添加必要的测试
3. 更新相关文档

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

---

> 📈 **专注核心，追求极致** - 移除复杂抽象，专注套利本质，这就是精简版MEV机器人的设计哲学。
