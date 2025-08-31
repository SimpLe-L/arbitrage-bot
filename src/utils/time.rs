//! 时间工具

use chrono::{DateTime, Utc, Duration};

/// 获取当前UTC时间戳（毫秒）
pub fn current_timestamp_ms() -> u64 {
    Utc::now().timestamp_millis() as u64
}

/// 获取当前UTC时间戳（秒）
pub fn current_timestamp() -> u64 {
    Utc::now().timestamp() as u64
}

/// 检查时间戳是否在指定的秒数内
pub fn is_within_seconds(timestamp: u64, seconds: i64) -> bool {
    let now = Utc::now().timestamp();
    let diff = (now - timestamp as i64).abs();
    diff <= seconds
}

/// 将时间戳转换为可读的时间字符串
pub fn timestamp_to_string(timestamp: u64) -> String {
    let datetime = DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| Utc::now());
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_current_timestamp() {
        let ts1 = current_timestamp();
        thread::sleep(StdDuration::from_millis(100));
        let ts2 = current_timestamp();
        
        assert!(ts2 >= ts1);
        assert!(ts2 - ts1 <= 1); // Should be within 1 second
    }

    #[test]
    fn test_current_timestamp_ms() {
        let ts1 = current_timestamp_ms();
        thread::sleep(StdDuration::from_millis(100));
        let ts2 = current_timestamp_ms();
        
        assert!(ts2 > ts1);
        assert!(ts2 - ts1 >= 90); // Should be at least 90ms
    }

    #[test]
    fn test_is_within_seconds() {
        let now = current_timestamp();
        
        // Test current timestamp should be within 1 second
        assert!(is_within_seconds(now, 1));
        
        // Test old timestamp should not be within 1 second
        let old_timestamp = now - 10;
        assert!(!is_within_seconds(old_timestamp, 1));
        assert!(is_within_seconds(old_timestamp, 15));
    }

    #[test]
    fn test_timestamp_to_string() {
        let timestamp = 1640995200u64; // 2022-01-01 00:00:00 UTC
        let formatted = timestamp_to_string(timestamp);
        
        assert!(formatted.contains("2022-01-01"));
        assert!(formatted.contains("00:00:00"));
        assert!(formatted.contains("UTC"));
    }
}
