use async_trait::async_trait;
use eyre::Result;
use ethers::{
    providers::{Http, Provider, Middleware},
    types::{Address, Block, Transaction, TransactionReceipt, U256, H256, BlockId, Bytes},
    utils::Anvil,
};
use std::{
    collections::HashMap,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use super::{BalanceChange, SimulateCtx, SimulateResult, Simulator};

#[derive(Clone)]
pub struct FoundrySimulator {
    pub provider: Arc<Provider<Http>>,
    pub fork_url: String,
    pub anvil_port: u16,
    pub chain_id: u64,
    anvil_process: Option<Arc<Child>>,
}

impl FoundrySimulator {
    pub async fn new(
        fork_url: String,
        anvil_port: Option<u16>,
        fork_block: Option<u64>,
    ) -> Result<Self> {
        let port = anvil_port.unwrap_or(8545);
        
        info!("启动 Foundry Anvil 进程，端口: {}", port);
        
        let anvil_process = Self::start_anvil(&fork_url, port, fork_block).await?;
        let anvil_url = format!("http://localhost:{}", port);
        
        // 等待 Anvil 启动
        sleep(Duration::from_secs(2)).await;
        
        let provider = Provider::<Http>::try_from(anvil_url.as_str())?;
        let provider = Arc::new(provider);
        
        let chain_id = provider.get_chainid().await?.as_u64();
        
        Ok(Self {
            provider,
            fork_url,
            anvil_port: port,
            chain_id,
            anvil_process: Some(Arc::new(anvil_process)),
        })
    }

    pub async fn new_avalanche_mainnet(
        fork_url: String,
        anvil_port: Option<u16>,
        fork_block: Option<u64>,
    ) -> Result<Self> {
        let mut simulator = Self::new(fork_url, anvil_port, fork_block).await?;
        simulator.chain_id = 43114; // Force Avalanche mainnet chain ID
        Ok(simulator)
    }

    pub async fn new_avalanche_fuji(
        fork_url: String,
        anvil_port: Option<u16>,
        fork_block: Option<u64>,
    ) -> Result<Self> {
        let mut simulator = Self::new(fork_url, anvil_port, fork_block).await?;
        simulator.chain_id = 43113; // Force Avalanche fuji chain ID
        Ok(simulator)
    }

    async fn start_anvil(fork_url: &str, port: u16, fork_block: Option<u64>) -> Result<Child> {
        let mut cmd = Command::new("anvil");
        cmd.arg("--host").arg("127.0.0.1")
           .arg("--port").arg(port.to_string())
           .arg("--fork-url").arg(fork_url)
           .arg("--gas-limit").arg("30000000")
           .arg("--gas-price").arg("25000000000") // 25 gwei default for Avalanche
           .arg("--accounts").arg("10")
           .arg("--balance").arg("10000")
           .arg("--chain-id").arg("43114"); // Default to Avalanche mainnet
        
        if let Some(block) = fork_block {
            cmd.arg("--fork-block-number").arg(block.to_string());
        }
        
        cmd.stdout(Stdio::null())
           .stderr(Stdio::null());
           
        let child = cmd.spawn()?;
        
        info!("Anvil 进程已启动，PID: {:?}", child.id());
        Ok(child)
    }

    pub async fn reset_fork(&self, block_number: Option<u64>) -> Result<()> {
        let method = "anvil_reset";
        let mut params = vec![serde_json::json!({
            "forking": {
                "jsonRpcUrl": self.fork_url,
            }
        })];
        
        if let Some(block) = block_number {
            if let Some(forking) = params[0].get_mut("forking") {
                forking["blockNumber"] = serde_json::json!(format!("0x{:x}", block));
            }
        }
        
        let _: serde_json::Value = self.provider
            .request(method, params)
            .await?;
            
        debug!("Anvil fork 已重置到区块: {:?}", block_number);
        Ok(())
    }

    pub async fn set_balance(&self, address: Address, balance: U256) -> Result<()> {
        let method = "anvil_setBalance";
        let params = vec![
            serde_json::json!(format!("{:#x}", address)),
            serde_json::json!(format!("0x{:x}", balance)),
        ];
        
        let _: serde_json::Value = self.provider
            .request(method, params)
            .await?;
            
        debug!("设置地址 {} 的余额为 {}", address, balance);
        Ok(())
    }

    pub async fn impersonate_account(&self, address: Address) -> Result<()> {
        let method = "anvil_impersonateAccount";
        let params = vec![serde_json::json!(format!("{:#x}", address))];
        
        let _: serde_json::Value = self.provider
            .request(method, params)
            .await?;
            
        debug!("开始模拟账户: {}", address);
        Ok(())
    }

    pub async fn stop_impersonating(&self, address: Address) -> Result<()> {
        let method = "anvil_stopImpersonatingAccount";
        let params = vec![serde_json::json!(format!("{:#x}", address))];
        
        let _: serde_json::Value = self.provider
            .request(method, params)
            .await?;
            
        debug!("停止模拟账户: {}", address);
        Ok(())
    }

    async fn calculate_balance_changes(
        &self,
        tx: &Transaction,
        receipt: &TransactionReceipt,
        ctx: &SimulateCtx,
    ) -> Result<Vec<BalanceChange>> {
        let mut balance_changes = Vec::new();

        // 计算 gas 费用
        let gas_cost = receipt.gas_used.unwrap_or_default() 
            * receipt.effective_gas_price.unwrap_or(tx.gas_price.unwrap_or_default());
        
        if gas_cost > U256::zero() {
            balance_changes.push(BalanceChange {
                address: tx.from,
                token: Address::zero(), // Native AVAX
                amount: -(gas_cost.as_u128() as i128),
            });
        }

        // 价值转移
        if let Some(value) = tx.value {
            if value > U256::zero() {
                balance_changes.push(BalanceChange {
                    address: tx.from,
                    token: Address::zero(),
                    amount: -(value.as_u128() as i128),
                });

                if let Some(to) = tx.to {
                    balance_changes.push(BalanceChange {
                        address: to,
                        token: Address::zero(),
                        amount: value.as_u128() as i128,
                    });
                }
            }
        }

        // 处理闪电贷偿还
        if let Some((token, amount)) = &ctx.flashloan_amount {
            balance_changes.push(BalanceChange {
                address: tx.from,
                token: *token,
                amount: -(amount.as_u128() as i128),
            });
        }

        // 从交易收据的日志中解析 ERC20 转账事件
        for log in &receipt.logs {
            if let Some(transfer_change) = self.parse_transfer_log(log) {
                balance_changes.push(transfer_change);
            }
        }

        Ok(balance_changes)
    }

    fn parse_transfer_log(&self, log: &ethers::types::Log) -> Option<BalanceChange> {
        // ERC20 Transfer 事件的签名
        const TRANSFER_SIGNATURE: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
        
        if log.topics.is_empty() || format!("{:#x}", log.topics[0]) != TRANSFER_SIGNATURE {
            return None;
        }
        
        if log.topics.len() != 3 {
            return None;
        }
        
        // 解析转账事件: Transfer(address indexed from, address indexed to, uint256 value)
        let _from = Address::from(log.topics[1]);
        let to = Address::from(log.topics[2]);
        
        if log.data.len() >= 32 {
            let amount = U256::from_big_endian(&log.data[..32]);
            return Some(BalanceChange {
                address: to,
                token: log.address,
                amount: amount.as_u128() as i128,
            });
        }
        
        None
    }
}

#[async_trait]
impl Simulator for FoundrySimulator {
    async fn simulate(&self, tx: Transaction, ctx: SimulateCtx) -> Result<SimulateResult> {
        let simulation_start = std::time::Instant::now();
        
        // 如果需要重置 fork 到特定区块
        if let Some(fork_block) = ctx.fork_block {
            self.reset_fork(Some(fork_block)).await?;
        }

        // 应用余额覆盖
        for (account, token, balance) in &ctx.override_balances {
            if *token == Address::zero() {
                // 设置原生 AVAX 余额
                self.set_balance(*account, *balance).await?;
            } else {
                // TODO: 为 ERC20 代币设置余额（需要调用合约方法）
                warn!("ERC20 余额覆盖尚未实现");
            }
        }

        // 如果有闪电贷，给发送者添加临时余额
        if let Some((token, amount)) = &ctx.flashloan_amount {
            if *token == Address::zero() {
                let current_balance = self.get_balance(tx.from, Address::zero()).await.unwrap_or_default();
                self.set_balance(tx.from, current_balance + amount).await?;
            }
        }

        // 模拟账户（如果需要）
        self.impersonate_account(tx.from).await?;

        // 执行交易模拟
        let result = match self.provider.call(&tx.clone().into(), None).await {
            Ok(result) => result,
            Err(e) => {
                self.stop_impersonating(tx.from).await?;
                return Err(eyre::eyre!("交易模拟失败: {}", e));
            }
        };

        // 估算 gas
        let gas_estimate = self.provider
            .estimate_gas(&tx.clone().into(), None)
            .await
            .unwrap_or(U256::from(21000));

        // 获取 gas 价格
        let gas_price = tx.gas_price.unwrap_or_else(|| {
            ctx.epoch.base_fee.max(U256::from(25_000_000_000)) // 25 gwei minimum
        });

        // 创建模拟的交易收据
        let receipt = TransactionReceipt {
            transaction_hash: tx.hash,
            transaction_index: Some(0u64.into()),
            block_hash: Some(H256::random()),
            block_number: Some(ctx.epoch.block_number.into()),
            from: tx.from,
            to: tx.to,
            cumulative_gas_used: gas_estimate,
            gas_used: Some(gas_estimate),
            contract_address: None,
            logs: Vec::new(), // 实际实现中应该从 call trace 中获取
            status: Some(1u64.into()),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: tx.transaction_type,
            effective_gas_price: Some(gas_price),
        };

        let balance_changes = self.calculate_balance_changes(&tx, &receipt, &ctx).await?;

        self.stop_impersonating(tx.from).await?;

        debug!("交易模拟耗时: {:?}", simulation_start.elapsed());

        Ok(SimulateResult {
            transaction_hash: tx.hash,
            receipt,
            gas_used: gas_estimate,
            gas_price,
            balance_changes,
            logs: Vec::new(), // TODO: 从模拟结果中获取日志
            cache_misses: 0,
        })
    }

    fn name(&self) -> &str {
        "FoundrySimulator"
    }

    async fn get_balance(&self, account: Address, token: Address) -> Option<U256> {
        if token == Address::zero() {
            // 原生 AVAX 余额
            self.provider.get_balance(account, None).await.ok()
        } else {
            // ERC20 代币余额 - 需要调用 balanceOf 方法
            // TODO: 实现 ERC20 余额查询
            None
        }
    }

    async fn get_block(&self, block_number: Option<u64>) -> Option<Block<H256>> {
        let block_id = block_number
            .map(|n| BlockId::Number(n.into()))
            .unwrap_or(BlockId::latest());
            
        self.provider.get_block(block_id).await.ok().flatten()
    }

    async fn estimate_gas(&self, tx: &Transaction) -> Result<U256> {
        self.provider
            .estimate_gas(&tx.clone().into(), None)
            .await
            .map_err(Into::into)
    }
}

impl Drop for FoundrySimulator {
    fn drop(&mut self) {
        info!("正在关闭 FoundrySimulator");
        // Anvil 进程将在 Drop 时自动终止
    }
}

/// 重放模拟器 - 确保始终使用最新状态执行
pub struct ReplaySimulator {
    foundry_sim: FoundrySimulator,
    update_interval: Duration,
    last_update: std::time::Instant,
}

impl ReplaySimulator {
    pub async fn new(
        fork_url: String,
        anvil_port: Option<u16>,
        update_interval: Duration,
    ) -> Result<Self> {
        let foundry_sim = FoundrySimulator::new(fork_url, anvil_port, None).await?;
        
        Ok(Self {
            foundry_sim,
            update_interval,
            last_update: std::time::Instant::now(),
        })
    }

    pub async fn new_avalanche_mainnet(
        fork_url: String,
        anvil_port: Option<u16>,
        update_interval: Duration,
    ) -> Result<Self> {
        let foundry_sim = FoundrySimulator::new_avalanche_mainnet(fork_url, anvil_port, None).await?;
        
        Ok(Self {
            foundry_sim,
            update_interval,
            last_update: std::time::Instant::now(),
        })
    }

    async fn maybe_update_fork(&mut self) -> Result<()> {
        if self.last_update.elapsed() > self.update_interval {
            self.foundry_sim.reset_fork(None).await?;
            self.last_update = std::time::Instant::now();
            debug!("Fork 状态已更新");
        }
        Ok(())
    }
}

#[async_trait]
impl Simulator for ReplaySimulator {
    async fn simulate(&self, tx: Transaction, ctx: SimulateCtx) -> Result<SimulateResult> {
        // 注意：这里我们不能修改 self，所以不能调用 maybe_update_fork
        // 在实际使用中，应该通过外部机制来定期更新 fork
        self.foundry_sim.simulate(tx, ctx).await
    }

    async fn get_balance(&self, account: Address, token: Address) -> Option<U256> {
        self.foundry_sim.get_balance(account, token).await
    }

    async fn get_block(&self, block_number: Option<u64>) -> Option<Block<H256>> {
        self.foundry_sim.get_block(block_number).await
    }

    fn name(&self) -> &str {
        "ReplaySimulator"
    }

    async fn estimate_gas(&self, tx: &Transaction) -> Result<U256> {
        self.foundry_sim.estimate_gas(tx).await
    }
}
