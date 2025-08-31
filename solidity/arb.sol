// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * ✅ 设计目标
 * - 用 Lotus Router 作为统一 swap 执行层（跨 V2/V3/Curve/Balancer…）
 * - 既支持“自有资金执行”，也支持“Flashloan 执行”（示例：Aave V3 / Uniswap V2 flash swap）
 * - 强约束：原子性、滑点保护、路由白名单、回调校验与利润结算
 *
 * ⚠️ 注意
 * - 下面对 Aave V3 / Lotus Router 仅保留必要的接口与回调签名，细节以你们真实依赖为准。
 * - 实产出前建议把 OZ SafeERC20 / ReentrancyGuard / Ownable 引自官方库，而不是简化实现。
 */

/* ============ Minimal ERC20 & SafeTransfer ============ */

interface IERC20 {
    function approve(address, uint256) external returns (bool);
    function transfer(address, uint256) external returns (bool);
    function transferFrom(address, address, uint256) external returns (bool);
    function balanceOf(address) external view returns (uint256);
    function allowance(address, address) external view returns (uint256);
    function decimals() external view returns (uint8);
}

library SafeTransfer {
    function safeApprove(IERC20 t, address to, uint256 v) internal {
        (bool ok, bytes memory d) = address(t).call(
            abi.encodeWithSelector(t.approve.selector, to, v)
        );
        require(ok && (d.length == 0 || abi.decode(d, (bool))), "APPROVE_FAIL");
    }
    function safeTransfer(IERC20 t, address to, uint256 v) internal {
        (bool ok, bytes memory d) = address(t).call(
            abi.encodeWithSelector(t.transfer.selector, to, v)
        );
        require(
            ok && (d.length == 0 || abi.decode(d, (bool))),
            "TRANSFER_FAIL"
        );
    }
    function safeTransferFrom(
        IERC20 t,
        address f,
        address to,
        uint256 v
    ) internal {
        (bool ok, bytes memory d) = address(t).call(
            abi.encodeWithSelector(t.transferFrom.selector, f, to, v)
        );
        require(ok && (d.length == 0 || abi.decode(d, (bool))), "TF_FROM_FAIL");
    }
}

/* ============ ReentrancyGuard (minimal) ============ */
abstract contract ReentrancyGuard {
    uint256 private _status = 1;
    modifier nonReentrant() {
        require(_status == 1, "REENTRANCY");
        _status = 2;
        _;
        _status = 1;
    }
}

/* ============ Ownable (minimal) ============ */
abstract contract Ownable {
    address public owner;
    error ONLY_OWNER();
    constructor() {
        owner = msg.sender;
    }
    modifier onlyOwner() {
        if (msg.sender != owner) revert ONLY_OWNER();
        _;
    }
    function transferOwnership(address n) external onlyOwner {
        owner = n;
    }
}

/* ============ Lotus Router (占位接口，根据你们版本替换) ============ */
/**
 * 你们的 Lotus Router 可能是“指令虚拟机”风格：
 *   - 输入一串 bytes 指令（path / action / minOut…）
 *   - Router 内部解析并逐步执行，最终把 token 留在 recipient
 *
 * 注意：这里定义了一个“通用 executePlan”接口，按你们实际 Router 改函数名与参数。
 */
interface ILotusRouter {
    struct ExecuteParams {
        address inputToken; // 起始 token
        uint256 amountIn; // 输入数量
        bytes plan; // 路径/指令字节串（off-chain 构造）
        address recipient; // 产出 token 的接收地址（一般本合约）
        uint256 minAmountOut; // 总体最小产出（再叠加本合约的风控也可以）
    }

    /// @notice 执行整条指令路径；返回最终产出数量（例如最终 token 的数量）
    function executePlan(
        ExecuteParams calldata p
    ) external payable returns (uint256 amountOut);
}

/* ============ Aave V3 Flashloan (最简接口，占位) ============ */
interface IAaveV3Pool {
    function flashLoanSimple(
        address receiver,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

interface IAaveV3FlashReceiver {
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool);
}

/* ============ Uniswap V2 Pair flash swap (可选) ============ */
interface IUniswapV2Pair {
    function token0() external view returns (address);
    function token1() external view returns (address);
    /// data 非空会回调 uniswapV2Call
    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
}

/* ============ 主合约 ============ */

contract ArbExecutorWithLotus is
    Ownable,
    ReentrancyGuard,
    IAaveV3FlashReceiver
{
    using SafeTransfer for IERC20;

    /* ---------- Errors ---------- */
    error ZeroAddress();
    error NotProfitable();
    error RouterNotWhitelisted();
    error BadCallback();
    error InvalidToken();
    error InvalidAmount();

    /* ---------- Events ---------- */
    event PlanExecuted(
        address indexed caller,
        address indexed profitToken,
        uint256 profit,
        bytes32 tag
    );
    event FlashExecuted(
        address indexed source,
        address asset,
        uint256 amount,
        uint256 premium,
        bool ok,
        bytes32 tag
    );

    /* ---------- State ---------- */
    ILotusRouter public router; // Lotus Router 地址
    mapping(address => bool) public routerWhitelist; // 可选：多 router 支持
    mapping(address => bool) public allowedPairs; // 可选：允许的 V2 Pair （防伪回调）
    IAaveV3Pool public aavePool; // Aave V3 Pool
    address public treasury; // 费用或小额残余收集地址

    constructor(address _router, address _aavePool, address _treasury) {
        if (
            _router == address(0) ||
            _aavePool == address(0) ||
            _treasury == address(0)
        ) revert ZeroAddress();
        router = ILotusRouter(_router);
        aavePool = IAaveV3Pool(_aavePool);
        treasury = _treasury;
        routerWhitelist[_router] = true;
    }

    /* ---------- 管理 ---------- */

    function setRouter(address r, bool ok) external onlyOwner {
        routerWhitelist[r] = ok;
        if (ok) router = ILotusRouter(r);
    }

    function setAavePool(address p) external onlyOwner {
        aavePool = IAaveV3Pool(p);
    }

    function setTreasury(address t) external onlyOwner {
        if (t == address(0)) revert ZeroAddress();
        treasury = t;
    }

    function setAllowedPair(address pair, bool ok) external onlyOwner {
        allowedPairs[pair] = ok;
    }

    /* ---------- 内部：批准 ---------- */
    function _approveMax(IERC20 token, address spender, uint256 need) internal {
        uint256 cur = token.allowance(address(this), spender);
        if (cur < need) {
            if (cur > 0) token.safeApprove(spender, 0); // ✅ 兼容非标准 ERC20
            token.safeApprove(spender, type(uint256).max);
        }
    }

    /* =======================================================================================
       1) 自有资金执行（无闪电贷）
       - 场景：你们在合约里/提前转入了 inputToken；或者先从 EOA -> 合约转入再执行
       ======================================================================================= */

    struct ExecuteArgs {
        address routerAddr; // 允许使用不同 Router（聚合器/不同版本）
        address inputToken; // 起始 token（例如 WETH/USDC）
        uint256 amountIn; // 使用多少作为起始（0 = 用合约当前余额）
        bytes plan; // Lotus 指令字节串（off-chain 构造）
        address expectedOutToken; // 预期最终 token；用于利润结算
        uint256 minTotalOut; // 全局 min out（双重保险：Router + 本合约）
        address profitReceiver; // 利润接收者（一般是 off-chain 策略账户）
        bytes32 tag; // 业务标记/追踪 ID（方便链上日志对齐）
    }

    function executeWithFunds(ExecuteArgs calldata a) external nonReentrant {
        if (!routerWhitelist[a.routerAddr]) revert RouterNotWhitelisted();
        if (
            a.profitReceiver == address(0) ||
            a.inputToken == address(0) ||
            a.expectedOutToken == address(0)
        ) {
            revert ZeroAddress();
        }

        // 1) 计算资金来源数量
        uint256 startBalIn = IERC20(a.inputToken).balanceOf(address(this));
        uint256 amountIn = a.amountIn == 0 ? startBalIn : a.amountIn;
        if (amountIn == 0) revert InvalidAmount();

        // 2) 允许 Router 花费 inputToken
        _approveMax(IERC20(a.inputToken), a.routerAddr, amountIn);

        // 3) 记录执行前后 expectedOutToken 的余额变化来判断利润
        uint256 beforeOut = IERC20(a.expectedOutToken).balanceOf(address(this));

        // 4) 调用 Lotus Router 执行整条路径（原子性由本笔 tx 保证）
        uint256 routerOut = ILotusRouter(a.routerAddr).executePlan(
            ILotusRouter.ExecuteParams({
                inputToken: a.inputToken,
                amountIn: amountIn,
                plan: a.plan,
                recipient: address(this),
                minAmountOut: a.minTotalOut // ✅ 再加一层 require 见下
            })
        );

        // 5) 基线风控：Router 声称的 out 应 >= minTotalOut
        require(routerOut >= a.minTotalOut, "SLIPPAGE_ROUTER");

        // 6) 计算真实产出与利润（以 expectedOutToken 计）
        uint256 afterOut = IERC20(a.expectedOutToken).balanceOf(address(this));
        uint256 gained = afterOut > beforeOut ? afterOut - beforeOut : 0;
        if (gained == 0) revert NotProfitable();

        // 7) 将全部产出/利润发送给接收者（你也可以拆分：回本入库 + 利润发给策略账户）
        IERC20(a.expectedOutToken).safeTransfer(a.profitReceiver, gained);

        emit PlanExecuted(msg.sender, a.expectedOutToken, gained, a.tag);
    }

    /* =======================================================================================
       2) Aave V3 Flashloan 执行
       - 思路：借入 inputToken -> 回调 executeOperation 内调用 Lotus Router -> 还本付息 -> 余下为利润
       - ⚠️ 一定要在 params 里带上 plan / 预期 token / 接收者等参数
       ======================================================================================= */

    struct FlashArgs {
        address routerAddr;
        address asset; // 借入资产（作为 plan 的 inputToken）
        uint256 amount; // 借入数量
        bytes plan; // Lotus 指令字节串
        address expectedOutToken; // 预计最终产出 token
        uint256 minTotalOut; // 总最小产出（保护）
        address profitReceiver; // 利润接收者
        bytes32 tag; // 标记
    }

    function executeWithAaveFlash(FlashArgs calldata f) external nonReentrant {
        if (!routerWhitelist[f.routerAddr]) revert RouterNotWhitelisted();
        if (
            f.asset == address(0) ||
            f.expectedOutToken == address(0) ||
            f.profitReceiver == address(0)
        ) {
            revert ZeroAddress();
        }
        if (f.amount == 0) revert InvalidAmount();

        // 将参数编码给回调使用
        bytes memory params = abi.encode(f);

        // 触发 Aave V3 Flashloan
        aavePool.flashLoanSimple(address(this), f.asset, f.amount, params, 0);
        // ⚠️ 注意：真正的执行发生在 executeOperation 回调里
    }

    /// @notice Aave V3 回调
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        // ✅ 验证回调来源（生产：白名单池地址 / msg.sender == aavePool）
        require(msg.sender == address(aavePool), "BAD_POOL");
        require(initiator == address(this), "BAD_INITIATOR");

        FlashArgs memory f = abi.decode(params, (FlashArgs));
        require(asset == f.asset && f.amount == amount, "BAD_ASSET");

        // 1) 授权 Router 花费借来的资产
        _approveMax(IERC20(f.asset), f.routerAddr, amount);

        // 2) 记录 expectedOutToken 余额
        uint256 beforeOut = IERC20(f.expectedOutToken).balanceOf(address(this));

        // 3) 调 Router 执行
        uint256 routerOut = ILotusRouter(f.routerAddr).executePlan(
            ILotusRouter.ExecuteParams({
                inputToken: f.asset,
                amountIn: amount,
                plan: f.plan,
                recipient: address(this),
                minAmountOut: f.minTotalOut
            })
        );
        require(routerOut >= f.minTotalOut, "SLIPPAGE_ROUTER");

        // 4) 归还 Aave：本金 + premium
        uint256 repay = amount + premium;
        IERC20(asset).safeTransfer(address(aavePool), repay);

        // 5) 利润结算（可能利润就是 expectedOutToken，也可能仍是 asset，视路径而定）
        uint256 afterOut = IERC20(f.expectedOutToken).balanceOf(address(this));
        uint256 gained = afterOut > beforeOut ? afterOut - beforeOut : 0;
        require(gained > 0, "NO_PROFIT");

        IERC20(f.expectedOutToken).safeTransfer(f.profitReceiver, gained);

        emit FlashExecuted(msg.sender, asset, amount, premium, true, f.tag);
        return true;
    }

    /* =======================================================================================
       3) Uniswap V2 flash swap（可选）
       - 如果你们偏好 V2 Pair flash：从 pair 借 token0/token1，回调里执行 plan，再归还 + fee
       - 需要先把 pair 加入 allowedPairs 防伪
       ======================================================================================= */

    struct V2FlashArgs {
        address routerAddr;
        address pair; // V2 Pair 地址（需已 allow）
        uint256 amount0Out; // 借 token0 的数量（或 0）
        uint256 amount1Out; // 借 token1 的数量（或 0）
        bytes plan; // Lotus 指令字节串
        address expectedOutToken;
        uint256 minTotalOut;
        address profitReceiver;
        bytes32 tag;
    }

    function executeWithV2Flash(V2FlashArgs calldata v) external nonReentrant {
        if (!routerWhitelist[v.routerAddr]) revert RouterNotWhitelisted();
        if (!allowedPairs[v.pair]) revert BadCallback();
        if (v.profitReceiver == address(0) || v.expectedOutToken == address(0))
            revert ZeroAddress();
        // 把参数塞进 data，触发回调
        bytes memory data = abi.encode(v);
        IUniswapV2Pair(v.pair).swap(
            v.amount0Out,
            v.amount1Out,
            address(this),
            data
        );
    }

    function uniswapV2Call(
        address /*sender*/,
        uint amount0,
        uint amount1,
        bytes calldata data
    ) external {
        // 验证回调来源
        if (!allowedPairs[msg.sender]) revert BadCallback();

        V2FlashArgs memory v = abi.decode(data, (V2FlashArgs));

        // 确定借出的 token 与数量
        address t0 = IUniswapV2Pair(msg.sender).token0();
        address t1 = IUniswapV2Pair(msg.sender).token1();
        address borrowed = amount0 > 0 ? t0 : t1;
        uint256 borrowedAmt = amount0 > 0 ? amount0 : amount1;

        // 授权 Router
        _approveMax(IERC20(borrowed), v.routerAddr, borrowedAmt);

        // 记录 expectedOutToken 余额
        uint256 beforeOut = IERC20(v.expectedOutToken).balanceOf(address(this));

        // 调 Router 执行
        uint256 outAmt = ILotusRouter(v.routerAddr).executePlan(
            ILotusRouter.ExecuteParams({
                inputToken: borrowed,
                amountIn: borrowedAmt,
                plan: v.plan,
                recipient: address(this),
                minAmountOut: v.minTotalOut
            })
        );
        require(outAmt >= v.minTotalOut, "SLIPPAGE_ROUTER");

        // 归还：V2 常见费 = amount * 3/997 + 1（实际以 pair 费率为准；最好从库/合约读取）
        uint256 fee = (borrowedAmt * 3) / 997 + 1;
        uint256 repay = borrowedAmt + fee;
        IERC20(borrowed).safeTransfer(msg.sender, repay);

        // 利润结算
        uint256 afterOut = IERC20(v.expectedOutToken).balanceOf(address(this));
        uint256 gained = afterOut > beforeOut ? afterOut - beforeOut : 0;
        require(gained > 0, "NO_PROFIT");
        IERC20(v.expectedOutToken).safeTransfer(v.profitReceiver, gained);

        emit PlanExecuted(tx.origin, v.expectedOutToken, gained, v.tag);
    }

    /* ---------- 资产救援 ---------- */
    function sweep(address token, address to) external onlyOwner {
        if (to == address(0)) revert ZeroAddress();
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) IERC20(token).safeTransfer(to, bal);
    }
}
