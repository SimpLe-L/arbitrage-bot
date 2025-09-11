//! AVAX链上代币和DEX配置

use std::collections::HashMap;

/// AVAX链上知名代币配置
pub struct TokenConfig {
    /// 代币地址到符号的映射
    pub tokens: HashMap<String, TokenInfo>,
    /// DEX路由器地址
    pub dex_routers: HashMap<String, String>,
    /// ERC20函数签名
    pub erc20_selectors: HashMap<[u8; 4], String>,
    /// DEX函数签名
    pub dex_selectors: HashMap<[u8; 4], String>,
}

#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub symbol: String,
    pub decimals: u8,
    pub address: String,
}

impl TokenConfig {
    pub fn new() -> Self {
        let mut tokens = HashMap::new();
        
        // AVAX链主要代币
        tokens.insert(
            "0xa7d7079b0fead91f3e65f86e8915cb59c1a4c664".to_lowercase(),
            TokenInfo {
                symbol: "USDC.e".to_string(),
                decimals: 6,
                address: "0xa7d7079b0fead91f3e65f86e8915cb59c1a4c664".to_string(),
            }
        );
        
        tokens.insert(
            "0xb31f66aa3c1e785363f0875a1b74e27b85fd66c7".to_lowercase(),
            TokenInfo {
                symbol: "WAVAX".to_string(),
                decimals: 18,
                address: "0xb31f66aa3c1e785363f0875a1b74e27b85fd66c7".to_string(),
            }
        );
        
        tokens.insert(
            "0xc7198437980c041c805a1edcba50c1ce5db95118".to_lowercase(),
            TokenInfo {
                symbol: "USDT.e".to_string(),
                decimals: 6,
                address: "0xc7198437980c041c805a1edcba50c1ce5db95118".to_string(),
            }
        );
        
        tokens.insert(
            "0x49d5c2bdffac6ce2bfdb6640f4f80f226bc10bab".to_lowercase(),
            TokenInfo {
                symbol: "WETH.e".to_string(),
                decimals: 18,
                address: "0x49d5c2bdffac6ce2bfdb6640f4f80f226bc10bab".to_string(),
            }
        );
        
        tokens.insert(
            "0x60781c2586d68229fde47564546784ab3faca982".to_lowercase(),
            TokenInfo {
                symbol: "PNG".to_string(),
                decimals: 18,
                address: "0x60781c2586d68229fde47564546784ab3faca982".to_string(),
            }
        );
        
        tokens.insert(
            "0x6e84a6216ea6dacc71ee8e6b0a5b7322eebc0fdd".to_lowercase(),
            TokenInfo {
                symbol: "JOE".to_string(),
                decimals: 18,
                address: "0x6e84a6216ea6dacc71ee8e6b0a5b7322eebc0fdd".to_string(),
            }
        );

        // DEX路由器地址
        let mut dex_routers = HashMap::new();
        dex_routers.insert(
            "0x60ae616a2155ee3d9a68541ba4544862310933d4".to_lowercase(),
            "TraderJoe".to_string()
        );
        dex_routers.insert(
            "0xe54ca86531e17ef3616d22ca28b0d458b6c89106".to_lowercase(),
            "Pangolin".to_string()
        );
        dex_routers.insert(
            "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506".to_lowercase(),
            "SushiSwap".to_string()
        );

        // ERC20函数签名
        let mut erc20_selectors = HashMap::new();
        erc20_selectors.insert([0xa9, 0x05, 0x9c, 0xbb], "transfer".to_string());
        erc20_selectors.insert([0x23, 0xb8, 0x72, 0xdd], "transferFrom".to_string());
        erc20_selectors.insert([0x09, 0x5e, 0xa7, 0xb3], "approve".to_string());

        // DEX函数签名
        let mut dex_selectors = HashMap::new();
        dex_selectors.insert([0x38, 0xed, 0x17, 0x39], "swapExactTokensForTokens".to_string());
        dex_selectors.insert([0x8a, 0x03, 0xb2, 0xd4], "swapExactTokensForETH".to_string());
        dex_selectors.insert([0x7f, 0xf3, 0x6a, 0xb5], "swapExactETHForTokens".to_string());
        dex_selectors.insert([0xe8, 0xe3, 0x37, 0x00], "addLiquidity".to_string());

        Self {
            tokens,
            dex_routers,
            erc20_selectors,
            dex_selectors,
        }
    }

    /// 根据地址获取代币信息
    pub fn get_token_by_address(&self, address: &str) -> Option<&TokenInfo> {
        self.tokens.get(&address.to_lowercase())
    }

    /// 检查是否为已知代币地址
    pub fn is_known_token(&self, address: &str) -> bool {
        self.tokens.contains_key(&address.to_lowercase())
    }

    /// 检查是否为DEX路由器地址
    pub fn is_dex_router(&self, address: &str) -> bool {
        self.dex_routers.contains_key(&address.to_lowercase())
    }

    /// 获取DEX名称
    pub fn get_dex_name(&self, address: &str) -> Option<&String> {
        self.dex_routers.get(&address.to_lowercase())
    }

    /// 检查函数选择器是否为ERC20函数
    pub fn is_erc20_function(&self, selector: &[u8; 4]) -> Option<&String> {
        self.erc20_selectors.get(selector)
    }

    /// 检查函数选择器是否为DEX函数
    pub fn is_dex_function(&self, selector: &[u8; 4]) -> Option<&String> {
        self.dex_selectors.get(selector)
    }
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self::new()
    }
}
