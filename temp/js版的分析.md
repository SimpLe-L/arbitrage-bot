## pools
1. 从链上历史区块查询 Uniswap V2 工厂的 PairCreated 事件（即新交易对的创建事件）；

2. 获取每个交易对中两个 token 的地址和精度（decimals）信息；

3. 将这些信息格式化为 Pool 实例并缓存到本地文件中（CSV 格式），下次运行可以跳过重复同步；

4. 提高套利机器人后续路径搜索的效率和准确性（如确定 token 对是否存在、计算金额精度等）。

----核心实现-------
```js
class Pool {  constructor(address, version, token0, token1, decimals0, decimals1, fee)}

// 表示一个交易池（Uniswap V2 中为 token0/token1 的交易对）。包含：地址、DEX 版本（V2/V3）、两个 token 地址、它们的 decimals 精度，以及手续费（V2 默认 0.3% 即 300）。
```

```js
loadCachedPools() 和 cacheSyncedPools()

// 读取或写入缓存池数据的本地 CSV 文件。
// 避免重复扫描链上历史区块，加快二次启动速度。
```

```js
const range = (start, stop, step) => {...}

// 将一个大的区块范围分段，用于分批调用 queryFilter 防止一次性获取过多事件
```

```js
loadAllPoolsFromV2(httpsUrl, factoryAddresses, fromBlocks, chunk)
// httpsUrl: RPC 链节点 URL；
// factoryAddresses: 一个或多个 UniswapV2 工厂合约地址；
// fromBlocks: 每个工厂合约的起始 block（部署时间）；
// chunk: 每次调用 queryFilter 的区块区间大小（比如 1000 块）

const events = await v2Factory.queryFilter(filter, params[0], params[1]);
// 从每个工厂的起始区块开始，分段扫描链上 PairCreated 事件；
// 拿到 token0/token1 和 pair address；
// 查询 token0/token1 合约的 decimals() 值；
// 存储为 Pool 实例；
// 最后写入本地缓存文件。

```

优化：
```js
// queryFilter 是线性的，不支持并发处理多个 factory → 可以用 Promise.all 或 p-limit 提升并发效率。
// 未处理 Uniswap V3 的池子，虽然 DexVariant 中有 UniswapV3，但此代码只处理了 V2。
// token decimals 获取是串行的→ 可以在 token 不重复的情况下批量并发请求。
// fee 值写死为 300，对 V3 来说费率是可变的，这里不是很适配。
```

## strategy
池子同步 → 路径构建 → 实时更新 reserve → 模拟交易路径 → 判断是否有正收益 → 输出套利路径与收益率

核心解析看代码注释

优化：
```js
// --加 gas 成本考虑：当前只判断了 spread，但没考虑：

// --gas 消耗；
//   套利金额大小（是否值得）；
//   滑点风险。

// --接入 Flashbots / MEV protection：
//   Flashbots 支持可以避免抢跑（frontrun）或 sandwich 攻击。

// --设置交易阈值：
//   例如 spread > 0.5% 时才尝试发交易。

// --并发模拟：
//   当前所有路径是串行模拟的，for (let idx = 0; idx < ... 可替换为并发 Promise.all()。

// --动态调整路径：
//   如果套利失败或频繁无利润，可以动态重建路径（或 prune 无效路径）
```

## paths
该模块从所有池中构建三角套利路径，并封装成可模拟、可优化输入金额的对象，为机器人后续模拟交易与套利判断提供支持。

优化
```js
// --模拟金额不应始终为 1，建议传入策略性金额
// --使用 gas-aware 优化， 加入 gas 成本计算
// let costInUSDC = gasUsed * gasPrice * ETH/USDC rate
// if (spread - costInUSDC > threshold) {
//     executeArbitrage()
// }
// --并行构造路径/模拟，当池子数量很多时，O(n^3) 可能非常慢
//   使用并发任务（如 Promise.allSettled()）；
//   分批次构建；
//   构建路径时提前剪枝（比如不考虑 TVL < $10K 的池子）
// --扩展为 n-hop
// generatePaths(pools, tokenIn, maxHops)
```

## bundler
围绕 Flashbots 实现的以太坊交易打包发送系统的核心模块
