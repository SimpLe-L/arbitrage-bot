const cliProgress = require('cli-progress');

const { logger } = require('./constants');
const { Path } = require('./bundler');
const { UniswapV2Simulator } = require('./simulator');

const range = (start, stop, step) => {
    let loopCnt = Math.ceil((stop - start) / step);
    let rangeArray = [];
    for (let i = 0; i < loopCnt; i++) {
        let num = start + (i * step);
        rangeArray.push(num);
    }
    return rangeArray;
}

class ArbPath {
    // 参与的 3 个 pool；每跳的 token 方向（zeroForOneX，用于判断是 token0 → token1，还是反过来）
    constructor(
        pool1,
        pool2,
        pool3,
        zeroForOne1,
        zeroForOne2,
        zeroForOne3
    ) {
        this.pool1 = pool1;
        this.pool2 = pool2;
        this.pool3 = pool3;
        this.zeroForOne1 = zeroForOne1;
        this.zeroForOne2 = zeroForOne2;
        this.zeroForOne3 = zeroForOne3;
    }

    nhop() {
        return this.pool3 === undefined ? 2 : 3;
    }

    hasPool(pool) {
        let isPool1 = this.pool1.address.toLowerCase() == pool.toLowerCase();
        let isPool2 = this.pool2.address.toLowerCase() == pool.toLowerCase();
        let isPool3 = this.pool3.address.toLowerCase() == pool.toLowerCase();
        return isPool1 || isPool2 || isPool3;
    }

    // x判断路径中是否包含被黑名单命中的 token
    shouldBlacklist(blacklistTokens) {
        for (let i = 0; i < this.nhop(); i++) {
            let pool = this[`pool${i + 1}`];
            if ((pool.token0 in blacklistTokens) || (pool.token1 in blacklistTokens)) {
                return true;
            }
            return false;
        }
    }

    // 模拟该路径在给定 reserve 状态下，从 amountIn 开始，经过三个 pool 最终得到多少 token
    // 内部逻辑：
    // 初始值 amountIn 被扩大到真实精度（乘上 decimals）；
    // 每一跳用 UniswapV2Simulator.getAmountOut(...) 计算下一跳输入。
    // 这就复用了恒定乘积公式（含手续费）：
    // out = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    simulateV2Path(amountIn, reserves) {
        let tokenInDecimals = this.zeroForOne1 ? this.pool1.decimals0 : this.pool1.decimals1;
        let amountOut = amountIn * 10 ** tokenInDecimals;

        let sim = new UniswapV2Simulator();
        let nhop = this.nhop();
        for (let i = 0; i < nhop; i++) {
            let pool = this[`pool${i + 1}`];
            let zeroForOne = this[`zeroForOne${i + 1}`];
            let reserve0 = reserves[pool.address][0];
            let reserve1 = reserves[pool.address][1];
            let fee = pool.fee;
            let reserveIn = zeroForOne ? reserve0 : reserve1;
            let reserveOut = zeroForOne ? reserve1 : reserve0;
            amountOut = sim.getAmountOut(amountOut, reserveIn, reserveOut, fee);
        }
        return amountOut;
    }
    // 暴力枚举从 0 到 maxAmountIn 所有输入值，找出收益最大的那一个（贪婪搜索）
    // const [bestAmountIn, profit] = path.optimizeAmountIn(100, 1, reserves);
    // bestAmountIn: 最佳的输入金额（单位是“原始单位”，如 12 表示 12 USDC）；
    // profit: 收益（单位同样是“原始单位”）。
    optimizeAmountIn(maxAmountIn, stepSize, reserves) {
        let tokenInDecimals = this.zeroForOne1 ? this.pool1.decimals0 : this.pool1.decimals1;
        let optimizedIn = 0;
        let profit = 0;
        for (let amountIn of range(0, maxAmountIn, stepSize)) {
            let amountOut = this.simulateV2Path(amountIn, reserves);
            let thisProfit = amountOut - (amountIn * (10 ** tokenInDecimals));
            if (thisProfit >= profit) {
                optimizedIn = amountIn;
                profit = thisProfit;
            } else {
                break;
            }
        }
        return [optimizedIn, profit / (10 ** tokenInDecimals)];
    }

    // 构造路径对象用于传递给打包器发送真实交易,这部分主要用于 Flashbots/0x/multicall 提交交易。
    toPathParams(routers) {
        let pathParams = [];
        for (let i = 0; i < this.nhop(); i++) {
            let pool = this[`pool${i + 1}`];
            let zeroForOne = this[`zeroForOne${i + 1}`];
            let tokenIn = zeroForOne ? pool.token0 : pool.token1;
            let tokenOut = zeroForOne ? pool.token1 : pool.token0;
            let path = new Path(routers[i], tokenIn, tokenOut);
            pathParams.push(path);
        }
        return pathParams;
    }
}

// 从所有池中构建出符合条件的三角套利路径，找出所有能从 tokenIn 出发、绕一圈又回到 tokenIn 的 3-hop 路径
function generateTriangularPaths(pools, tokenIn) {
    /*
    This can easily be refactored into a recursive function to support the
    generation of n-hop paths. However, I left it as a 3-hop path generating function
    just for demonstration. This will be easier to follow along.

    👉 The recursive version can be found here (Python):
    https://github.com/solidquant/whack-a-mole/blob/main/data/dex.py
    */
    const paths = [];

    pools = Object.values(pools);

    const progress = new cliProgress.SingleBar({}, cliProgress.Presets.shades_classic);
    progress.start(pools.length);

    for (let i = 0; i < pools.length; i++) {
        let pool1 = pools[i];
        let canTrade1 = (pool1.token0 == tokenIn) || (pool1.token1 == tokenIn);
        if (canTrade1) {
            let zeroForOne1 = pool1.token0 == tokenIn;
            let [tokenIn1, tokenOut1] = zeroForOne1 ? [pool1.token0, pool1.token1] : [pool1.token1, pool1.token0];
            if (tokenIn1 != tokenIn) {
                continue;
            }

            for (let j = 0; j < pools.length; j++) {
                let pool2 = pools[j];
                let canTrade2 = (pool2.token0 == tokenOut1) || (pool2.token1 == tokenOut1);
                if (canTrade2) {
                    let zeroForOne2 = pool2.token0 == tokenOut1;
                    let [tokenIn2, tokenOut2] = zeroForOne2 ? [pool2.token0, pool2.token1] : [pool2.token1, pool2.token0];
                    if (tokenOut1 != tokenIn2) {
                        continue;
                    }

                    for (let k = 0; k < pools.length; k++) {
                        let pool3 = pools[k];
                        let canTrade3 = (pool3.token0 == tokenOut2) || (pool3.token1 == tokenOut2);
                        if (canTrade3) {
                            let zeroForOne3 = pool3.token0 == tokenOut2;
                            let [tokenIn3, tokenOut3] = zeroForOne3 ? [pool3.token0, pool3.token1] : [pool3.token1, pool3.token0];
                            if (tokenOut2 != tokenIn3) {
                                continue;
                            }

                            if (tokenOut3 == tokenIn) {
                                let uniquePoolCnt = [...new Set([
                                    pool1.address,
                                    pool2.address,
                                    pool3.address,
                                ])].length;

                                if (uniquePoolCnt < 3) {
                                    continue;
                                }

                                let arbPath = new ArbPath(pool1,
                                    pool2,
                                    pool3,
                                    zeroForOne1,
                                    zeroForOne2,
                                    zeroForOne3);
                                paths.push(arbPath);
                            }
                        }
                    }
                }
            }
        }
        progress.update(i + 1);
    }

    progress.stop();
    logger.info(`Generated ${paths.length} 3-hop arbitrage paths`);
    return paths;
}

module.exports = {
    ArbPath,
    generateTriangularPaths,
};