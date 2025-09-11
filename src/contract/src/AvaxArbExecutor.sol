// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(
        address from,
        address to,
        uint256 amount
    ) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
}

interface IAaveV3Pool {
    function flashLoanSimple(
        address receiver,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

interface IUniswapV2Pair {
    function swap(
        uint amount0Out,
        uint amount1Out,
        address to,
        bytes calldata data
    ) external;
    function token0() external view returns (address);
    function token1() external view returns (address);
}

/// @title Avalanche Arbitrage Executor
/// @notice 精简的原子套利执行合约，专为AVAX网络优化
contract AvaxArbExecutor {
    /* ========== EVENTS ========== */

    event ArbExecuted(
        address indexed token,
        uint256 profit,
        bytes32 indexed tag
    );

    /* ========== ERRORS ========== */

    error NotOwner();
    error NotProfitable();
    error InvalidCallback();
    error TransferFailed();

    /* ========== STATE ========== */

    address public immutable owner;
    IAaveV3Pool public constant AAVE_POOL =
        IAaveV3Pool(0x794a61358D6845594F94dc1DB02A252b5b4814aD); // Aave V3 AVAX

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    /* ========== CORE EXECUTION ========== */

    struct ArbParams {
        address tokenIn;
        uint256 amountIn;
        bytes swapData; // 编码的交易路径数据
        address profitToken; // 期望获利的代币
        uint256 minProfit; // 最小利润要求
        bytes32 tag; // 追踪标记
    }

    /// @notice 使用自有资金执行套利
    function executeArb(ArbParams calldata params) external onlyOwner {
        uint256 balanceBefore = IERC20(params.profitToken).balanceOf(
            address(this)
        );

        // 执行交换序列
        _executeSwaps(params.swapData);

        // 计算并验证利润
        uint256 balanceAfter = IERC20(params.profitToken).balanceOf(
            address(this)
        );
        uint256 profit = balanceAfter - balanceBefore;

        if (profit < params.minProfit) revert NotProfitable();

        // 转移利润给owner
        IERC20(params.profitToken).transfer(owner, profit);

        emit ArbExecuted(params.profitToken, profit, params.tag);
    }

    /// @notice 使用Aave闪电贷执行套利
    function executeArbWithFlash(ArbParams calldata params) external onlyOwner {
        bytes memory data = abi.encode(params);
        AAVE_POOL.flashLoanSimple(
            address(this),
            params.tokenIn,
            params.amountIn,
            data,
            0
        );
    }

    /// @notice Aave闪电贷回调
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool) {
        if (msg.sender != address(AAVE_POOL) || initiator != address(this)) {
            revert InvalidCallback();
        }

        ArbParams memory arbParams = abi.decode(params, (ArbParams));
        uint256 balanceBefore = IERC20(arbParams.profitToken).balanceOf(
            address(this)
        );

        // 执行套利交换
        _executeSwaps(arbParams.swapData);

        // 归还闪电贷
        uint256 repayAmount = amount + premium;
        IERC20(asset).transfer(address(AAVE_POOL), repayAmount);

        // 计算利润
        uint256 balanceAfter = IERC20(arbParams.profitToken).balanceOf(
            address(this)
        );
        uint256 profit = balanceAfter - balanceBefore;

        if (profit < arbParams.minProfit) revert NotProfitable();

        // 转移利润
        IERC20(arbParams.profitToken).transfer(owner, profit);

        emit ArbExecuted(arbParams.profitToken, profit, arbParams.tag);
        return true;
    }

    /* ========== INTERNAL FUNCTIONS ========== */

    /// @notice 执行编码的交换序列
    /// @dev swapData格式: [路径数量][路径1数据][路径2数据]...
    function _executeSwaps(bytes memory swapData) internal {
        uint256 offset = 0;
        uint8 pathCount = uint8(swapData[0]);
        offset += 1;

        for (uint8 i = 0; i < pathCount; i++) {
            offset = _executeSingleSwap(swapData, offset);
        }
    }

    /// @notice 执行单个交换
    function _executeSingleSwap(
        bytes memory data,
        uint256 offset
    ) internal returns (uint256 newOffset) {
        // 解析交换类型 (1字节)
        uint8 swapType = uint8(data[offset]);
        offset += 1;

        if (swapType == 1) {
            // UniswapV2风格
            (address pair, uint256 amount0Out, uint256 amount1Out) = abi.decode(
                _slice(data, offset, 84),
                (address, uint256, uint256)
            );

            IUniswapV2Pair(pair).swap(
                amount0Out,
                amount1Out,
                address(this),
                ""
            );
            return offset + 84;
        } else if (swapType == 2) {
            // 直接转账
            (address token, address to, uint256 amount) = abi.decode(
                _slice(data, offset, 84),
                (address, address, uint256)
            );

            IERC20(token).transfer(to, amount);
            return offset + 84;
        } else if (swapType == 3) {
            // 批准操作
            (address token, address spender, uint256 amount) = abi.decode(
                _slice(data, offset, 84),
                (address, address, uint256)
            );

            IERC20(token).approve(spender, amount);
            return offset + 84;
        }

        return offset;
    }

    /// @notice 切片bytes数据
    function _slice(
        bytes memory data,
        uint256 start,
        uint256 length
    ) internal pure returns (bytes memory) {
        bytes memory result = new bytes(length);
        for (uint256 i = 0; i < length; i++) {
            result[i] = data[start + i];
        }
        return result;
    }

    /* ========== EMERGENCY FUNCTIONS ========== */

    /// @notice 紧急提取代币
    function emergencyWithdraw(address token) external onlyOwner {
        uint256 balance = IERC20(token).balanceOf(address(this));
        if (balance > 0) {
            IERC20(token).transfer(owner, balance);
        }
    }

    /// @notice 紧急提取ETH/AVAX
    function emergencyWithdrawNative() external onlyOwner {
        payable(owner).transfer(address(this).balance);
    }

    receive() external payable {}
}
