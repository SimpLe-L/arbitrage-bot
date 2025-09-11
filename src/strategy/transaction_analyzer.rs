//! 交易分析器 - 负责从区块链交易中提取代币信息和套利机会

use ethers::types::{Transaction, Address};
use tracing::{info, warn};
use eyre::Result;

use crate::utils::token_config::TokenConfig;

/// 交易分析器
pub struct TransactionAnalyzer {
    token_config: TokenConfig,
}

impl TransactionAnalyzer {
    pub fn new() -> Self {
        Self {
            token_config: TokenConfig::new(),
        }
    }

    /// 从交易中提取相关的代币地址
    /// 完善版本：解析交易input数据和识别ERC20/DEX交互
    pub fn extract_token_from_tx(&self, tx: &Transaction) -> Option<String> {
        if let Some(to) = tx.to {
            let to_str = format!("{:?}", to);
            
            // 如果交易没有input data，可能是简单的ETH转账，跳过
            if tx.input.0.is_empty() {
                return None;
            }

            // 解析交易input data
            let input_data = &tx.input.0;
            if input_data.len() < 4 {
                return None;
            }

            // 提取函数选择器（前4字节）
            let function_selector: [u8; 4] = [
                input_data[0], input_data[1], input_data[2], input_data[3]
            ];
            
            // 检查ERC20函数
            if let Some(function_name) = self.token_config.is_erc20_function(&function_selector) {
                info!("Detected ERC20 {} to: {}", function_name, to_str);
                return Some(to_str);
            }
            
            // 检查DEX函数
            if let Some(function_name) = self.token_config.is_dex_function(&function_selector) {
                info!("Detected DEX {} transaction to: {}", function_name, to_str);
                return self.extract_token_from_dex_call(&function_selector, input_data);
            }
            
            // 检查是否是已知的代币合约地址
            if self.token_config.is_known_token(&to_str) {
                info!("Detected known token contract interaction: {}", to_str);
                return Some(to_str);
            }
            
            // 检查是否是DEX路由器地址
            if self.token_config.is_dex_router(&to_str) {
                if let Some(dex_name) = self.token_config.get_dex_name(&to_str) {
                    info!("Detected {} router interaction: {}", dex_name, to_str);
                }
                // 对于未识别的DEX函数，尝试解析路径参数
                return self.extract_token_from_unknown_dex_call(input_data);
            }
        }
        
        None
    }

    /// 从DEX调用中提取代币信息
    fn extract_token_from_dex_call(&self, selector: &[u8; 4], input_data: &[u8]) -> Option<String> {
        match selector {
            [0x38, 0xed, 0x17, 0x39] | // swapExactTokensForTokens
            [0x8a, 0x03, 0xb2, 0xd4] | // swapExactTokensForETH  
            [0x7f, 0xf3, 0x6a, 0xb5] => { // swapExactETHForTokens
                self.extract_token_from_swap_path(input_data)
            },
            [0xe8, 0xe3, 0x37, 0x00] => { // addLiquidity
                self.extract_tokens_from_add_liquidity(input_data)
            },
            _ => None
        }
    }

    /// 从swap函数的路径参数中提取代币地址
    fn extract_token_from_swap_path(&self, input_data: &[u8]) -> Option<String> {
        // swapExactTokensForTokens的参数结构：
        // - amountIn (32 bytes)
        // - amountOutMin (32 bytes)  
        // - path offset (32 bytes)
        // - to (32 bytes)
        // - deadline (32 bytes)
        // - path length (32 bytes)
        // - path data...
        
        if input_data.len() < 4 + 32 * 6 {
            return None;
        }
        
        // 跳过函数选择器和前5个参数，读取路径长度
        let path_length_offset = 4 + 32 * 5;
        if input_data.len() < path_length_offset + 32 {
            return None;
        }
        
        // 读取路径长度（大端序）
        let path_length = u32::from_be_bytes([
            input_data[path_length_offset + 28],
            input_data[path_length_offset + 29], 
            input_data[path_length_offset + 30],
            input_data[path_length_offset + 31],
        ]);
        
        if path_length > 0 && input_data.len() >= path_length_offset + 32 + 20 {
            // 读取第一个代币地址（20字节）
            let token_start = path_length_offset + 32 + 12; // 跳过长度字段的padding
            if input_data.len() >= token_start + 20 {
                let token_bytes = &input_data[token_start..token_start + 20];
                let token_address = format!("0x{}", 
                    token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
                info!("Extracted token from swap path: {}", token_address);
                return Some(token_address);
            }
        }
        
        None
    }

    /// 从addLiquidity函数中提取代币地址
    fn extract_tokens_from_add_liquidity(&self, input_data: &[u8]) -> Option<String> {
        // addLiquidity参数：tokenA, tokenB, ...
        if input_data.len() < 4 + 32 * 2 {
            return None;
        }
        
        // 读取第一个代币地址（tokenA）
        let token_a_start = 4 + 12; // 跳过函数选择器和padding
        if input_data.len() >= token_a_start + 20 {
            let token_bytes = &input_data[token_a_start..token_a_start + 20];
            let token_address = format!("0x{}", 
                token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            info!("Extracted tokenA from addLiquidity: {}", token_address);
            return Some(token_address);
        }
        
        None
    }

    /// 从未识别的DEX调用中尝试提取代币信息
    fn extract_token_from_unknown_dex_call(&self, input_data: &[u8]) -> Option<String> {
        // 简单策略：扫描input data中的地址格式数据
        // 寻找可能的ERC20代币地址模式
        if input_data.len() < 4 + 32 {
            return None;
        }
        
        // 跳过函数选择器，扫描每32字节的参数
        let mut offset = 4;
        while offset + 32 <= input_data.len() {
            // 检查是否看起来像地址（前12字节为0，后20字节非全0）
            let is_address_like = input_data[offset..offset + 12].iter().all(|&b| b == 0) &&
                                  !input_data[offset + 12..offset + 32].iter().all(|&b| b == 0);
                                  
            if is_address_like {
                let token_bytes = &input_data[offset + 12..offset + 32];
                let token_address = format!("0x{}", 
                    token_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
                
                // 基本验证：地址应该不全为0且不全为F
                if !token_address.ends_with("0000000000000000000000000000000000000000") &&
                   !token_address.ends_with("ffffffffffffffffffffffffffffffffffffffff") {
                    info!("Extracted potential token from unknown DEX call: {}", token_address);
                    return Some(token_address);
                }
            }
            
            offset += 32;
        }
        
        None
    }

    /// 获取代币符号
    pub fn get_token_symbol(&self, address: &str) -> String {
        // 如果是AVAX原生代币
        if crate::utils::coin::is_native_coin(address) {
            return "AVAX".to_string();
        }
        
        // 从配置中获取代币信息
        if let Some(token_info) = self.token_config.get_token_by_address(address) {
            return token_info.symbol.clone();
        }
        
        // 如果是未知代币，尝试从地址提取简短格式
        if address.len() > 10 {
            format!("{}...{}", &address[0..6], &address[address.len()-4..])
        } else {
            address.to_string()
        }
    }
}

impl Default for TransactionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
