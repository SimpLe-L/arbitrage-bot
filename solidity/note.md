1) Lotus Router 的定位与接口

最佳实践：把所有 DEX 的细节交给 Router 处理；你们只维护 executePlan(ExecuteParams) 这一“统一接口”。

常见坑：

Router 不是你们自己写的，要白名单路由地址，避免被替换成恶意路由。

plan 的 calldata 编码必须与 Router 规范完全一致；链下构造器要有充分的单元测试。

建议在 Router 与本合约双层 minOut（Router 内部一步步保护、本合约做全局保护）。

2) 授权 approve

最佳实践：只对白名单 Router给 uint256.max 授权，并在部署后不轻易更换 Router 地址。

常见坑：很多代币要求先清零再授权；用 safeApprove(spender,0) 再设大额，避免“非标准 ERC20”失败。

3) 原子性与回滚

所有步骤放在 同一笔交易。任何一步失败直接 revert，天然原子。

常见坑：有些 DEX 的 supportingFeeOnTransfer 版本与常规模型返回值不同，Router 内部要处理好。

4) Aave V3 Flashloan 回调校验

最佳实践：

在 executeOperation 里验证 msg.sender == aavePool、initiator == address(this)；

回调参数中带上你要执行的 plan、expectedOutToken 等，不要从外部读可被 front-run 的状态。

常见坑：

归还资产时余额不够（路径产出不是同一种资产）；要么在路径尾部换回 asset，要么合约里预留少量 buffer。

忘记把 premium 算入归还额。

5) Uniswap V2 Flash Swap

最佳实践：

在 uniswapV2Call 中验证 回调 Pair 白名单，或根据 Factory + token0/token1 计算 pair 地址核对。

常见坑：

费率不是固定 0.3% 的 fork，需要从 Pair/Factory 读取，或者预估不足导致还款失败。

回调里再外部调用不当可能触发重入；使用 nonReentrant 并审计 Router 的调用路径。

6) 利润结算

最佳实践：统一用 expectedOutToken 计利润，只把增量部分转给接收者；留存或小额 dust 归集到 treasury（可扩展）。

常见坑：

利润为 0 仍然放行，会浪费 gas；严格 require(gained > 0)。

多币种利润（例如多跳后产出多币），需要清算为一个基准币再判断利润。

7) 安全与可升级

最佳实践：

强烈建议接入 Permit2 或签名授权，减少无限额授权暴露面。

如果计划频繁升级，考虑Proxy + UUPS；但注意初始化与权限治理。

常见坑：

误设权限导致任何人能改 Router / Pair 白名单。

忘了限制 sweep 只能由 owner，或把关键参数暴露给外部。

8) Off-chain 策略协同（什么时候交互？）

时机：当 off-chain 通过价格流/报价器/自建 indexer 确认存在可观无风险（或容忍风险）价差，并且估算 gas + MEV 成本 后仍为正；

流程：

链下构造 plan（Lotus 指令字节串）与 minTotalOut；

选择自有资金还是 Flashloan；

通过私有交易通道（如 Flashbots）提交 executeWithFunds 或 executeWithAaveFlash；

监听 PlanExecuted/FlashExecuted 事件，记录利润并迭代策略。

可以直接落地的下一步

把上面的 ILotusRouter 改成你们 Router 的真实 ABI（或再包一层适配器 Adapter）。

实现链下 plan 构造器（把路径、fee、minOut、动作序列编码成 bytes）。

接入 OZ 正式库（SafeERC20, ReentrancyGuard, Ownable）。

按你的目标链（Base/Arb/OP/ETH）配置 Aave Pool 地址与白名单 Router/Pair。

做一套主流代币与路由的 回归测试（含转账即收税代币、不同精度、失败场景）