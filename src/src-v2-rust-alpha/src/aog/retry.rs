use std::thread;
use std::time::Duration;
use crate::aog::error::{AogError, Result};

pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            exponential_backoff: true,
        }
    }
}

impl RetryConfig {
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
    
    pub fn with_delays(mut self, initial_ms: u64, max_ms: u64) -> Self {
        self.initial_delay_ms = initial_ms;
        self.max_delay_ms = max_ms;
        self
    }
    
    pub fn with_linear_backoff(mut self) -> Self {
        self.exponential_backoff = false;
        self
    }
}

pub fn retry_operation<F, T>(
    operation: F, 
    operation_name: &str,
    config: RetryConfig,
) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    let mut retries = 0;
    let mut delay_ms = config.initial_delay_ms;
    
    loop {
        match operation() {
            Ok(result) => {
                if retries > 0 {
                    log::info!("{} succeeded after {} retries", operation_name, retries);
                }
                return Ok(result);
            }
            Err(e) if retries < config.max_retries && e.can_recover() => {
                log::warn!(
                    "{} failed (attempt {}/{}): {}. Retrying in {}ms...", 
                    operation_name, 
                    retries + 1, 
                    config.max_retries,
                    e, 
                    delay_ms
                );
                
                thread::sleep(Duration::from_millis(delay_ms));
                
                if config.exponential_backoff {
                    delay_ms = (delay_ms * 2).min(config.max_delay_ms);
                } else {
                    delay_ms = delay_ms.min(config.max_delay_ms);
                }
                
                retries += 1;
            }
            Err(e) => {
                if !e.can_recover() {
                    log::error!("{} failed with non-recoverable error: {}", operation_name, e);
                } else {
                    log::error!("{} failed after {} retries: {}", operation_name, config.max_retries, e);
                }
                return Err(e);
            }
        }
    }
}

pub async fn retry_async_operation<F, T, Fut>(
    operation: F,
    operation_name: &str,
    config: RetryConfig,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut retries = 0;
    let mut delay_ms = config.initial_delay_ms;
    
    loop {
        match operation().await {
            Ok(result) => {
                if retries > 0 {
                    log::info!("{} succeeded after {} retries", operation_name, retries);
                }
                return Ok(result);
            }
            Err(e) if retries < config.max_retries && e.can_recover() => {
                log::warn!(
                    "{} failed (attempt {}/{}): {}. Retrying in {}ms...",
                    operation_name,
                    retries + 1,
                    config.max_retries,
                    e,
                    delay_ms
                );
                
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                
                if config.exponential_backoff {
                    delay_ms = (delay_ms * 2).min(config.max_delay_ms);
                } else {
                    delay_ms = delay_ms.min(config.max_delay_ms);
                }
                
                retries += 1;
            }
            Err(e) => {
                if !e.can_recover() {
                    log::error!("{} failed with non-recoverable error: {}", operation_name, e);
                } else {
                    log::error!("{} failed after {} retries: {}", operation_name, config.max_retries, e);
                }
                return Err(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    #[test]
    fn test_retry_success_on_first_attempt() {
        let result = retry_operation(
            || Ok(42),
            "test_operation",
            RetryConfig::default(),
        );
        
        assert_eq!(result.unwrap(), 42);
    }
    
    #[test]
    fn test_retry_success_after_failures() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempt_count);
        
        let result = retry_operation(
            move || {
                let attempts = count_clone.fetch_add(1, Ordering::SeqCst);
                if attempts < 2 {
                    Err(AogError::TimeoutError("Simulated timeout".to_string()))
                } else {
                    Ok(42)
                }
            },
            "test_operation",
            RetryConfig::default(),
        );
        
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
    
    #[test]
    fn test_retry_exhaustion() {
        let result = retry_operation::<_, i32>(
            || Err(AogError::TimeoutError("Always fails".to_string())),
            "test_operation",
            RetryConfig::default().with_max_retries(2),
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_non_recoverable_error() {
        let result = retry_operation::<_, i32>(
            || Err(AogError::SafetyError("Safety check failed".to_string())),
            "test_operation",
            RetryConfig::default(),
        );
        
        assert!(result.is_err());
    }
}