// Copyright (c) 2024 Terragon Labs
//
// MIT License
//
// Error handling module - Provides comprehensive error types for the AOG system

use std::fmt;
use std::error::Error;
use std::sync::PoisonError;

/// Main error type for the AOG system
#[derive(Debug)]
pub enum AogError {
    /// Configuration-related errors
    Config(String),
    
    /// File I/O errors
    Io(std::io::Error),
    
    /// Serialization/deserialization errors
    Serialization(String),
    
    /// Mutex lock poisoning errors
    MutexPoisoned(String),
    
    /// Sensor-related errors
    Sensor(String),
    
    /// Pump operation errors
    Pump(String),
    
    /// Network/HTTP errors
    Network(String),
    
    /// Hardware interface errors
    Hardware(String),
    
    /// Validation errors
    Validation(String),
    
    /// Emergency stop condition
    EmergencyStop(String),
    
    /// Authentication/session errors
    Auth(String),
}

impl fmt::Display for AogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AogError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AogError::Io(err) => write!(f, "I/O error: {}", err),
            AogError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            AogError::MutexPoisoned(msg) => write!(f, "Mutex poisoned: {}", msg),
            AogError::Sensor(msg) => write!(f, "Sensor error: {}", msg),
            AogError::Pump(msg) => write!(f, "Pump error: {}", msg),
            AogError::Network(msg) => write!(f, "Network error: {}", msg),
            AogError::Hardware(msg) => write!(f, "Hardware error: {}", msg),
            AogError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AogError::EmergencyStop(msg) => write!(f, "Emergency stop: {}", msg),
            AogError::Auth(msg) => write!(f, "Authentication error: {}", msg),
        }
    }
}

impl Error for AogError {}

// Conversion implementations for common error types
impl From<std::io::Error> for AogError {
    fn from(err: std::io::Error) -> Self {
        AogError::Io(err)
    }
}

impl From<serde_json::Error> for AogError {
    fn from(err: serde_json::Error) -> Self {
        AogError::Serialization(err.to_string())
    }
}

impl<T> From<PoisonError<T>> for AogError {
    fn from(err: PoisonError<T>) -> Self {
        AogError::MutexPoisoned(format!("Mutex lock poisoned: {}", err))
    }
}

// Result type alias for convenience
pub type AogResult<T> = Result<T, AogError>;

/// Helper trait for converting Option to Result with context
pub trait OptionExt<T> {
    fn ok_or_log(self, context: &str) -> AogResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_log(self, context: &str) -> AogResult<T> {
        self.ok_or_else(|| {
            let err = AogError::Validation(context.to_string());
            log::error!("{}", err);
            err
        })
    }
}

/// Helper trait for Result types to add logging
pub trait ResultExt<T> {
    fn log_error(self, context: &str) -> AogResult<T>;
    fn or_log_default(self, context: &str, default: T) -> T;
}

impl<T, E: fmt::Display> ResultExt<T> for Result<T, E> {
    fn log_error(self, context: &str) -> AogResult<T> {
        self.map_err(|e| {
            let err = AogError::Config(format!("{}: {}", context, e));
            log::error!("{}", err);
            err
        })
    }
    
    fn or_log_default(self, context: &str, default: T) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                log::warn!("{}: {} - using default value", context, e);
                default
            }
        }
    }
}

/// Recovery strategies for mutex poisoning
pub fn recover_mutex_lock<'a, T: Clone>(
    mutex: &'a std::sync::Mutex<T>,
    context: &str,
) -> AogResult<std::sync::MutexGuard<'a, T>> {
    match mutex.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            log::error!("Mutex poisoned in {}: attempting recovery", context);
            // Recovery: get the guard anyway (data might still be valid)
            Ok(poisoned.into_inner())
        }
    }
}

/// Safe mutex access with automatic recovery and default value fallback
pub fn safe_mutex_access<T: Clone, F, R>(
    mutex: &std::sync::Mutex<T>,
    context: &str,
    operation: F,
    default: R,
) -> R
where
    F: FnOnce(&T) -> R,
{
    match recover_mutex_lock(mutex, context) {
        Ok(guard) => operation(&*guard),
        Err(e) => {
            log::error!("Failed to access mutex in {}: {} - returning default", context, e);
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    
    #[test]
    fn test_error_display() {
        let err = AogError::Config("test error".to_string());
        assert_eq!(format!("{}", err), "Configuration error: test error");
        
        let err = AogError::EmergencyStop("pump overflow".to_string());
        assert_eq!(format!("{}", err), "Emergency stop: pump overflow");
    }
    
    #[test]
    fn test_error_conversions() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let aog_err: AogError = io_err.into();
        assert!(matches!(aog_err, AogError::Io(_)));
    }
    
    #[test]
    fn test_option_ext() {
        let some_val: Option<i32> = Some(42);
        let result = some_val.ok_or_log("test context");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        let none_val: Option<i32> = None;
        let result = none_val.ok_or_log("missing value");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_safe_mutex_access() {
        let mutex = Mutex::new(42);
        let result = safe_mutex_access(&mutex, "test", |val| *val * 2, 0);
        assert_eq!(result, 84);
    }
}