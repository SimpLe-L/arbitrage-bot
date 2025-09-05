use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use burberry::ActionSubmitter;
use eyre::{bail, ensure, Context, OptionExt, Result};
use object_pool::ObjectPool;
use simulator::{ReplaySimulator, SimulateCtx, Simulator};
use ethers::types::{Address, TransactionRequest, H256, U256};
use tracing::{error, info, instrument};

use crate::{
    arb::{Arb, ArbResult},
    common::notification::new_tg_messages,
    types::{Action, Source},
};

use super::arb_cache::ArbItem;

pub struct Worker {
    pub _id: usize,
    pub sender: Address,

    pub arb_item_receiver: async_channel::Receiver<ArbItem>,

    pub simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>,
    pub simulator_name: String,

    pub dedicated_simulator: Option<Arc<ReplaySimulator>>,

    pub submitter: Arc<dyn ActionSubmitter<Action>>,
    pub arb: Arc<Arb>,
}

impl Worker {
    #[tokio::main]
    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                arb_item = self.arb_item_receiver.recv() => {
                    if let Err(error) = self.handle_arb_item(arb_item.context("arb_item channel error")?).await {
                        error!(?error, "Handle arb_item failed");
                    }
                }
                else => bail!("strategy channels undefined behavior"),
            }
        }
    }

    #[instrument(skip_all, fields(token = %arb_item.token.split("x").last().unwrap_or(&arb_item.token), tx = %arb_item.tx_hash))]
    pub async fn handle_arb_item(&mut self, arb_item: ArbItem) -> Result<()> {
        let ArbItem {
            token,
            pool_address,
            tx_hash,
            sim_ctx,
            source,
        } = arb_item;

        if let Some((arb_result, elapsed)) = arbitrage_one_token(
            self.arb.clone(),
            self.sender,
            &token,
            pool_address,
            sim_ctx.clone(),
            false,
            source,
        )
        .await
        {
            let tx_request = match self.dry_run_tx_request(arb_result.tx_data.clone(), sim_ctx.clone()).await {
                Ok(tx_request) => tx_request,
                Err(error) => {
                    error!(?arb_result, ?error, "Dry run final tx_request failed");
                    return Ok(());
                }
            };

            let arb_tx_hash = H256::zero(); // Placeholder - actual hash would be computed after sending
            let action = match arb_result.source {
                Source::MevRelay { bid_amount, .. } => Action::MevRelaySubmitBid((tx_request, bid_amount, tx_hash)),
                _ => Action::ExecutePublicTx(tx_request),
            };

            self.submitter.submit(action);

            let tg_msgs = new_tg_messages(tx_hash, arb_tx_hash, &arb_result, elapsed, &self.simulator_name);
            for tg_msg in tg_msgs {
                self.submitter.submit(tg_msg.into());
            }

            // notify dedicated simulator to update more frequently
            if let Some(dedicated_sim) = &self.dedicated_simulator {
                dedicated_sim.update_notifier.send(()).await.unwrap();
            }
        }

        Ok(())
    }

    // return a final tx_request with updated gas estimates
    async fn dry_run_tx_request(&self, tx_request: TransactionRequest, sim_ctx: SimulateCtx) -> Result<TransactionRequest> {
        let tx_request = self.update_gas_estimates(tx_request).await?;

        let resp = if let Some(dedicated_sim) = &self.dedicated_simulator {
            dedicated_sim.simulate_tx_request(tx_request.clone(), sim_ctx).await?
        } else {
            self.simulator_pool.get().simulate_tx_request(tx_request.clone(), sim_ctx).await?
        };

        ensure!(resp.success, "Dry run failed: {:?}", resp.error);
        ensure!(resp.profit > U256::zero(), "No profit from dry run: {:?}", resp);

        Ok(tx_request)
    }

    // Update gas price and gas limit estimates
    async fn update_gas_estimates(&self, mut tx_request: TransactionRequest) -> Result<TransactionRequest> {
        // For AVAX, use reasonable default gas settings
        let gas_price = U256::from(25_000_000_000u64); // 25 gwei
        let gas_limit = U256::from(300_000u64); // 300k gas limit
        
        tx_request.gas_price = Some(gas_price);
        tx_request.gas = Some(gas_limit);
        
        // Ensure sender is set
        tx_request.from = Some(self.sender);

        Ok(tx_request)
    }
}

async fn arbitrage_one_token(
    arb: Arc<Arb>,
    attacker: Address,
    token_address: &str,
    pool_address: Option<Address>,
    sim_ctx: SimulateCtx,
    use_gss: bool,
    source: Source,
) -> Option<(ArbResult, Duration)> {
    let start = Instant::now();
    let gas_limit = 300000u64;
    let arb_result = match arb
        .find_opportunity(attacker, token_address, pool_address, gas_limit, sim_ctx, use_gss, source)
        .await
    {
        Ok(r) => r,
        Err(error) => {
            let elapsed = start.elapsed();
            if elapsed > Duration::from_secs(1) {
                info!(elapsed = ?elapsed, %token_address, "🥱 \x1b[31mNo opportunity: {error:#}\x1b[0m");
            } else {
                info!(elapsed = ?elapsed, %token_address, "🥱 No opportunity: {error:#}");
            }
            return None;
        }
    };

    info!(
        elapsed = ?start.elapsed(),
        elapsed.ctx_creation = ?arb_result.create_trial_ctx_duration,
        elapsed.grid_search = ?arb_result.grid_search_duration,
        elapsed.gss = ?arb_result.gss_duration,
        cache_misses = ?arb_result.cache_misses,
        token = %token_address,
        "💰 Profitable opportunity found: {:?}",
        &arb_result.best_trial_result
    );

    Some((arb_result, start.elapsed()))
}
