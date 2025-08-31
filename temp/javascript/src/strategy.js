const { ethers } = require('ethers');
const EventEmitter = require('events');

const {
    HTTPS_URL,
    WSS_URL,
    PRIVATE_KEY,
    SIGNING_KEY,
    BOT_ADDRESS,
} = require('./constants');
const { logger, blacklistTokens } = require('./constants');
const { loadAllPoolsFromV2 } = require('./pools');
const { generateTriangularPaths } = require('./paths');
const { batchGetUniswapV2Reserves } = require('./multi');
const { streamNewBlocks } = require('./streams');
const { getTouchedPoolReserves } = require('./utils');
const { Bundler } = require('./bundler');

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(HTTPS_URL);

    const factoryAddresses = ['0xc35DADB65012eC5796536bD9864eD8773aBc74C4'];
    const factoryBlocks = [11333218];

    // 同步历史池子数据
    let pools = await loadAllPoolsFromV2(
        HTTPS_URL, factoryAddresses, factoryBlocks, 50000
    );
    logger.info(`Initial pool count: ${Object.keys(pools).length}`);

    const usdcAddress = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174';
    const usdcDecimals = 6;

    // 构造三角套利路径
    let paths = generateTriangularPaths(pools, usdcAddress);

    // 过滤出在套利路径中的池子
    pools = {};
    for (let path of paths) {
        if (!path.shouldBlacklist(blacklistTokens)) {
            pools[path.pool1.address] = path.pool1;
            pools[path.pool2.address] = path.pool2;
            pools[path.pool3.address] = path.pool3;
        }
    }
    logger.info(`New pool count: ${Object.keys(pools).length}`);

    let s = new Date();
    // 批量获取当前 reserves（token 余额），后续用于套利模拟计算
    let reserves = await batchGetUniswapV2Reserves(HTTPS_URL, Object.keys(pools));
    let e = new Date();
    logger.info(`Batch reserves call took: ${(e - s) / 1000} seconds`);

    // Transaction handler (can send transactions to mempool / bundles to Flashbots)
    let bundler = new Bundler(
        PRIVATE_KEY,
        SIGNING_KEY,
        HTTPS_URL,
        BOT_ADDRESS,
    );
    await bundler.setup();

    let eventEmitter = new EventEmitter();

    // 连接 WebSocket RPC，监听新区块； 
    streamNewBlocks(WSS_URL, eventEmitter);
    // 就是响应新区块的主回调
    eventEmitter.on('event', async (event) => {
        if (event.type == 'block') {
            let blockNumber = event.blockNumber;
            logger.info(`▪️ New Block #${blockNumber}`);
            // 获取本区块内对池子的交易（通常是 Sync 事件）， 返回结构是 { poolAddress: { reserve0, reserve1 } }
            let touchedReserves = await getTouchedPoolReserves(provider, blockNumber);
            let touchedPools = [];
            //  更新内存中对应池子的 reserves：
            for (let address in touchedReserves) {
                let reserve = touchedReserves[address];
                if (address in reserves) {
                    reserves[address] = reserve;
                    touchedPools.push(address);
                }
            }

            let spreads = {};
            for (let idx = 0; idx < Object.keys(paths).length; idx++) {
                let path = paths[idx];
                let touchedPath = touchedPools.reduce((touched, pool) => {
                    return touched + (path.hasPool(pool) ? 1 : 0)
                }, 0);
                if (touchedPath > 0) {
                    // 模拟路径，计算套利收益：
                    let priceQuote = path.simulateV2Path(1, reserves);
                    let spread = (priceQuote / (10 ** usdcDecimals) - 1) * 100;
                    if (spread > 0) {
                        // 打印所有三角路径中，收益率 > 0 的路径；
                        spreads[idx] = spread;
                    }
                }
            }

            console.log('▶️ Spread over 0%: ', spreads);
        }
    });
}

module.exports = {
    main,
};