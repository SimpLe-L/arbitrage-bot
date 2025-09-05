//! 性能监控工具

use std::time::{Duration, Instant};
use log::info;

/// 简单的性能计时器
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    pub fn new(name: &str) -> Self {
        Self {
            start: Instant::now(),
            name: name.to_string(),
        }
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    
    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed().as_millis()
    }
    
    pub fn log_elapsed(&self) {
        info!("{} 执行时间: {}ms", self.name, self.elapsed_ms());
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.log_elapsed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_timer_basic() {
        let timer = Timer::new("test");
        thread::sleep(StdDuration::from_millis(100));
        
        let elapsed = timer.elapsed();
        assert!(elapsed.as_millis() >= 90); // At least 90ms should have passed
        assert!(elapsed.as_millis() < 200); // But less than 200ms for reasonable test
    }

    #[test]
    fn test_timer_elapsed_ms() {
        let timer = Timer::new("test_ms");
        thread::sleep(StdDuration::from_millis(50));
        
        let elapsed_ms = timer.elapsed_ms();
        assert!(elapsed_ms >= 45); // At least 45ms
        assert!(elapsed_ms < 100); // But less than 100ms
    }

    #[test]
    fn test_timer_name() {
        let timer = Timer::new("my_timer");
        // Timer name is private, but we can verify it through log output
        // This is more of an integration test
        timer.log_elapsed();
    }
}
