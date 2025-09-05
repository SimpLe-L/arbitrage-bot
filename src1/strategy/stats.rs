//! 策略统计信息

/// 策略统计信息
#[derive(Debug, Clone)]
pub struct StrategyStats {
    pub processed_transactions: u64,
    pub opportunities_found: u64,
    pub successful_executions: u64,
    pub total_profit: f64,
    pub avg_profit_per_opportunity: f64,
    pub success_rate: f64,
}

impl Default for StrategyStats {
    fn default() -> Self {
        Self {
            processed_transactions: 0,
            opportunities_found: 0,
            successful_executions: 0,
            total_profit: 0.0,
            avg_profit_per_opportunity: 0.0,
            success_rate: 0.0,
        }
    }
}

impl StrategyStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 更新统计信息
    pub fn update(&mut self, profit: Option<f64>, success: bool) {
        self.processed_transactions += 1;
        
        if let Some(p) = profit {
            self.opportunities_found += 1;
            if success {
                self.successful_executions += 1;
                self.total_profit += p;
            }
            
            // 重新计算平均利润和成功率
            self.avg_profit_per_opportunity = if self.opportunities_found > 0 {
                self.total_profit / self.opportunities_found as f64
            } else {
                0.0
            };
            
            self.success_rate = if self.opportunities_found > 0 {
                self.successful_executions as f64 / self.opportunities_found as f64
            } else {
                0.0
            };
        }
    }
    
    /// 重置统计信息
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    
    /// 获取成功率百分比
    pub fn success_rate_percent(&self) -> f64 {
        self.success_rate * 100.0
    }
}
