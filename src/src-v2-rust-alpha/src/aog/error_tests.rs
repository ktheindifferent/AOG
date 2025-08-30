#[cfg(test)]
mod tests {
    use crate::aog::error::*;
    use crate::aog::retry::*;
    use crate::aog::error_monitor::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    #[test]
    fn test_aog_error_types() {
        let gpio_error = AogError::GpioError("Pin not available".to_string());
        assert!(gpio_error.to_string().contains("GPIO error"));
        
        let pump_error = AogError::PumpError("Pump failed to start".to_string());
        assert!(pump_error.to_string().contains("Pump control error"));
        
        let safety_error = AogError::SafetyError("Overflow detected".to_string());
        assert!(safety_error.to_string().contains("Safety check failed"));
    }
    
    #[test]
    fn test_error_recovery_classification() {
        // Recoverable errors
        let sensor_error = AogError::SensorError("Reading failed".to_string());
        assert!(sensor_error.can_recover());
        assert!(sensor_error.recovery_action().is_some());
        
        let timeout_error = AogError::TimeoutError("Operation timed out".to_string());
        assert!(timeout_error.can_recover());
        
        // Non-recoverable errors
        let safety_error = AogError::SafetyError("Critical failure".to_string());
        assert!(!safety_error.can_recover());
        assert!(safety_error.recovery_action().is_none());
        
        let hw_error = AogError::HardwareInitError("Hardware not found".to_string());
        assert!(!hw_error.can_recover());
    }
    
    #[test]
    fn test_error_context() {
        let context = ErrorContext::new("test_module", "test_operation");
        assert_eq!(context.module, "test_module");
        assert_eq!(context.operation, "test_operation");
        assert!(context.details.is_none());
        
        let context_with_details = ErrorContext::new("pump", "start")
            .with_details("GPIO pin 17 unavailable");
        assert_eq!(context_with_details.details, Some("GPIO pin 17 unavailable".to_string()));
    }
    
    #[test]
    fn test_retry_config() {
        let default_config = RetryConfig::default();
        assert_eq!(default_config.max_retries, 3);
        assert_eq!(default_config.initial_delay_ms, 100);
        assert_eq!(default_config.max_delay_ms, 5000);
        assert!(default_config.exponential_backoff);
        
        let custom_config = RetryConfig::default()
            .with_max_retries(5)
            .with_delays(200, 10000)
            .with_linear_backoff();
        
        assert_eq!(custom_config.max_retries, 5);
        assert_eq!(custom_config.initial_delay_ms, 200);
        assert_eq!(custom_config.max_delay_ms, 10000);
        assert!(!custom_config.exponential_backoff);
    }
    
    #[test]
    fn test_retry_operation_success_first_attempt() {
        let result = retry_operation(
            || Ok(42),
            "test_op",
            RetryConfig::default(),
        );
        
        assert_eq!(result.unwrap(), 42);
    }
    
    #[test]
    fn test_retry_operation_eventual_success() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempt_count);
        
        let result = retry_operation(
            move || {
                let attempts = count_clone.fetch_add(1, Ordering::SeqCst);
                if attempts < 2 {
                    Err(AogError::TimeoutError("Temporary failure".to_string()))
                } else {
                    Ok("success")
                }
            },
            "test_op",
            RetryConfig::default().with_delays(10, 100),
        );
        
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
    
    #[test]
    fn test_retry_operation_max_retries_exceeded() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempt_count);
        
        let result = retry_operation::<_, String>(
            move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
                Err(AogError::TimeoutError("Always fails".to_string()))
            },
            "test_op",
            RetryConfig::default().with_max_retries(2).with_delays(10, 100),
        );
        
        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // initial + 2 retries
    }
    
    #[test]
    fn test_retry_operation_non_recoverable() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempt_count);
        
        let result = retry_operation::<_, String>(
            move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
                Err(AogError::SafetyError("Critical safety error".to_string()))
            },
            "test_op",
            RetryConfig::default(),
        );
        
        assert!(result.is_err());
        // Should only try once since it's non-recoverable
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }
    
    #[test]
    fn test_error_monitor_logging() {
        let monitor = ErrorMonitor::new();
        
        let error1 = AogError::SensorError("Sensor read failed".to_string());
        monitor.log_error("sensors", "read_co2", &error1, Some("Pin timeout".to_string()));
        
        let error2 = AogError::SafetyError("Overflow detected".to_string());
        monitor.log_error("pump", "check_overflow", &error2, None);
        
        let dashboard = monitor.get_dashboard();
        
        assert_eq!(dashboard.statistics.total_errors, 2);
        assert_eq!(dashboard.statistics.recoverable_errors, 1);
        assert_eq!(dashboard.statistics.non_recoverable_errors, 1);
        assert_eq!(dashboard.recent_errors.len(), 2);
    }
    
    #[test]
    fn test_error_monitor_dashboard() {
        let monitor = ErrorMonitor::new();
        
        // Log various errors
        for i in 0..5 {
            let error = AogError::SensorError(format!("Sensor error {}", i));
            monitor.log_error("sensors", "read", &error, None);
        }
        
        for i in 0..3 {
            let error = AogError::PumpError(format!("Pump error {}", i));
            monitor.log_error("pump", "control", &error, None);
        }
        
        let error = AogError::SafetyError("Critical error".to_string());
        monitor.log_error("safety", "check", &error, None);
        
        let dashboard = monitor.get_dashboard();
        
        // Verify statistics
        assert_eq!(dashboard.statistics.total_errors, 9);
        assert_eq!(dashboard.statistics.recoverable_errors, 5); // Only sensor errors are recoverable
        assert_eq!(dashboard.statistics.non_recoverable_errors, 4); // 3 pump + 1 safety
        
        // Verify module status
        assert!(dashboard.module_status.iter().any(|m| m.name == "sensors" && m.error_count == 5));
        assert!(dashboard.module_status.iter().any(|m| m.name == "pump" && m.error_count == 3));
        assert!(dashboard.module_status.iter().any(|m| m.name == "safety" && m.error_count == 1));
        
        // Verify health status
        assert!(dashboard.system_health.critical_errors_count >= 4); // Non-recoverable errors
        assert!(dashboard.system_health.warnings_count >= 5); // Recoverable errors
    }
    
    #[test]
    fn test_error_monitor_max_history() {
        let monitor = ErrorMonitor::new();
        
        // Log more than MAX_ERROR_HISTORY errors
        for i in 0..150 {
            let error = AogError::SensorError(format!("Error {}", i));
            monitor.log_error("test", "operation", &error, None);
        }
        
        let dashboard = monitor.get_dashboard();
        
        // Should only keep the most recent MAX_ERROR_HISTORY entries
        assert!(dashboard.recent_errors.len() <= 20); // We show max 20 recent errors
        assert_eq!(dashboard.statistics.total_errors, 150);
    }
    
    #[test]
    fn test_error_conversions() {
        // Test From implementations
        let gpio_error = rppal::gpio::Error::PermissionDenied("/dev/gpiomem".to_string());
        let aog_error: AogError = gpio_error.into();
        assert!(matches!(aog_error, AogError::GpioError(_)));
        
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let aog_error: AogError = io_error.into();
        assert!(matches!(aog_error, AogError::IoError(_)));
    }
    
    #[test]
    fn test_retry_macro() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempt_count);
        
        let result = retry_with_backoff!(
            {
                let attempts = count_clone.fetch_add(1, Ordering::SeqCst);
                if attempts < 1 {
                    Err(AogError::TimeoutError("Temp failure".to_string()))
                } else {
                    Ok(100)
                }
            },
            3,
            10
        );
        
        assert_eq!(result.unwrap(), 100);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
    }
    
    #[test]
    fn test_system_health_determination() {
        let monitor = ErrorMonitor::new();
        
        // Healthy state
        let dashboard = monitor.get_dashboard();
        assert_eq!(dashboard.system_health.overall_health, "healthy");
        
        // Degraded state (multiple recoverable errors)
        for _ in 0..6 {
            let error = AogError::SensorError("Sensor glitch".to_string());
            monitor.log_error("sensors", "read", &error, None);
        }
        
        let dashboard = monitor.get_dashboard();
        // This might be "degraded" or "critical" depending on timing
        assert!(dashboard.system_health.overall_health != "healthy");
        
        // Critical state (non-recoverable error)
        let critical_error = AogError::SafetyError("System failure".to_string());
        monitor.log_error("safety", "critical_check", &critical_error, None);
        
        let dashboard = monitor.get_dashboard();
        assert_eq!(dashboard.system_health.overall_health, "critical");
    }
}