use eyre::Result;
use ethers::{
    prelude::*,
    types::{Address, Bytes, U256, H256},
};
use std::sync::Arc;
use tracing::{info, warn};

// 导入合约绑定
use crate::bindings::avaxarbexecutor::{AvaxArbExecutor, ArbParams};

/// 套利路径编码器
pub struct SwapDataEncoder;

impl SwapDataEncoder {
    /// 编码UniswapV2风格的交换
    pub fn encode_v2_swap(pair: Address, amount0_out: U256, amount1_out: U256) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(1u8); // 交换类型
        data.extend_from_slice(pair.as_bytes());
        data.extend_from_slice(&amount0_out.to_be_bytes_vec());
        data.extend_from_slice(&amount1_out.to_be_bytes_vec());
        data
    }
    
    /// 编码代币转账
    pub fn encode_transfer(token: Address, to: Address, amount: U256) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(2u8); // 转账类型
        data.extend_from_slice(token.as_bytes());
        data.extend_from_slice(to.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes_vec());
        data
    }
    
    /// 编码代币批准
    pub fn encode_approve(token: Address, spender: Address, amount: U256) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(3u8); // 批准类型
        data.extend_from_slice(token.as_bytes());
        data.extend_from_slice(spender.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes_vec());
        data
    }
    
    /// 组合多个操作
    pub fn encode_multi_swap(operations: Vec<Vec<u8>>) -> Bytes {
        let mut result = Vec::new();
        result.push(operations.len() as u8); // 操作数量
        
        for op in operations {
            result.extend(op);
        }
        
        Bytes::from(result)
    }
}

/// 套利参数构建器
#[derive(Debug, Clone)]
pub struct ArbParamsBuilder {
    token_in: Address,
    amount_in: U256,
    swap_operations: Vec<Vec<u8>>,
    profit_token: Address,
    min_profit: U256,
    tag: H256,
}

impl ArbParamsBuilder {
    pub fn new(token_in: Address, amount_in: U256, profit_token: Address) -> Self {
        Self {
            token_in,
            amount_in,
            swap_operations: Vec::new(),
            profit_token,
            min_profit: U256::zero(),
            tag: H256::random(),
        }
    }
    
    pub fn add_v2_swap(mut self, pair: Address, amount0_out: U256, amount1_out: U256) -> Self {
        self.swap_operations.push(SwapDataEncoder::encode_v2_swap(pair, amount0_out, amount1_out));
        self
    }
    
    pub fn add_transfer(mut self, token: Address, to: Address, amount: U256) -> Self {
        self.swap_operations.push(SwapDataEncoder::encode_transfer(token, to, amount));
        self
    }
    
    pub fn add_approve(mut self, token: Address, spender: Address, amount: U256) -> Self {
        self.swap_operations.push(SwapDataEncoder::encode_approve(token, spender, amount));
        self
    }
    
    pub fn min_profit(mut self, min_profit: U256) -> Self {
        self.min_profit = min_profit;
        self
    }
    
    pub fn tag(mut self, tag: H256) -> Self {
        self.tag = tag;
        self
    }
    
    pub fn build(self) -> ArbParams {
        ArbParams {
            token_in: self.token_in,
            amount_in: self.amount_in,
            swap_data: SwapDataEncoder::encode_multi_swap(self.swap_operations),
            profit_token: self.profit_token,
            min_profit: self.min_profit,
            tag: self.tag.into(),
        }
    }
}

/// 合约套利执行器
pub struct ContractArbExecutor<M> {
    contract: AvaxArbExecutor<M>,
    client: Arc<M>,
}

impl<M: Middleware> ContractArbExecutor<M> {
    pub fn new(contract_address: Address, client: Arc<M>) -> Self {
        let contract = AvaxArbExecutor::new(contract_address, client.clone());
        Self { contract, client }
    }
    
    /// 执行使用自有资金的套利
    pub async fn execute_arb(&self, params: ArbParams) -> Result<TransactionReceipt> {
        info!("执行套利交易: token_in={:?}, amount_in={}, profit_token={:?}", 
              params.token_in, params.amount_in, params.profit_token);
        
        let call = self.contract.execute_arb(params);
        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;
        
        match receipt {
            Some(receipt) => {
                info!("套利交易成功: tx_hash={:?}, gas_used={:?}", 
                      receipt.transaction_hash, receipt.gas_used);
                Ok(receipt)
            },
            None => {
                eyre::bail!("交易执行失败，未获得收据")
            }
        }
    }
    
    /// 执行使用闪电贷的套利
    pub async fn execute_arb_with_flash(&self, params: ArbParams) -> Result<TransactionReceipt> {
        info!("执行闪电贷套利: token_in={:?}, amount_in={}, profit_token={:?}", 
              params.token_in, params.amount_in, params.profit_token);
        
        let call = self.contract.execute_arb_with_flash(params);
        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;
        
        match receipt {
            Some(receipt) => {
                info!("闪电贷套利成功: tx_hash={:?}, gas_used={:?}", 
                      receipt.transaction_hash, receipt.gas_used);
                Ok(receipt)
            },
            None => {
                eyre::bail!("闪电贷交易执行失败")
            }
        }
    }
    
    /// 获取合约owner
    pub async fn get_owner(&self) -> Result<Address> {
        let owner = self.contract.owner().call().await?;
        Ok(owner)
    }
    
    /// 紧急提取代币
    pub async fn emergency_withdraw(&self, token: Address) -> Result<TransactionReceipt> {
        warn!("执行紧急提取: token={:?}", token);
        
        let call = self.contract.emergency_withdraw(token);
        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;
        
        match receipt {
            Some(receipt) => {
                info!("紧急提取成功: tx_hash={:?}", receipt.transaction_hash);
                Ok(receipt)
            },
            None => {
                eyre::bail!("紧急提取失败")
            }
        }
    }
    
    /// 构建套利交易（不发送）
    pub async fn build_arb_tx(&self, params: ArbParams) -> Result<TypedTransaction> {
        let call = self.contract.execute_arb(params);
        let tx = call.tx;
        Ok(tx)
    }
    
    /// 构建闪电贷套利交易（不发送）
    pub async fn build_flash_arb_tx(&self, params: ArbParams) -> Result<TypedTransaction> {
        let call = self.contract.execute_arb_with_flash(params);
        let tx = call.tx;
        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_swap_data_encoding() {
        let pair = Address::from_low_u64_be(1);
        let amount0 = U256::from(1000);
        let amount1 = U256::from(2000);
        
        let encoded = SwapDataEncoder::encode_v2_swap(pair, amount0, amount1);
        assert_eq!(encoded[0], 1u8); // 确认类型为V2交换
        assert!(encoded.len() > 1);
    }
    
    #[test]
    fn test_arb_params_builder() {
        let token_in = Address::from_low_u64_be(1);
        let token_out = Address::from_low_u64_be(2);
        let pair = Address::from_low_u64_be(3);
        let amount = U256::from(1000);
        
        let params = ArbParamsBuilder::new(token_in, amount, token_out)
            .add_v2_swap(pair, U256::zero(), amount)
            .min_profit(U256::from(100))
            .build();
            
        assert_eq!(params.token_in, token_in);
        assert_eq!(params.amount_in, amount);
        assert_eq!(params.profit_token, token_out);
        assert_eq!(params.min_profit, U256::from(100));
    }
}
