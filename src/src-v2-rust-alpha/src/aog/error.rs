use thiserror::Error;
use std::fmt;

#[derive(Error, Debug)]
pub enum AogError {
    #[error("GPIO error: {0}")]
    GpioError(String),
    
    #[error("Pump control error: {0}")]
    PumpError(String),
    
    #[error("Relay control error: {0}")]
    RelayError(String),
    
    #[error("Sensor reading error: {0}")]
    SensorError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("HTTP error: {0}")]
    HttpError(String),
    
    #[error("I2C communication error: {0}")]
    I2cError(String),
    
    #[error("Serial communication error: {0}")]
    SerialError(String),
    
    #[error("Lock acquisition error: {0}")]
    LockError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Thread communication error: {0}")]
    ThreadError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Safety check failed: {0}")]
    SafetyError(String),
    
    #[error("Hardware initialization failed: {0}")]
    HardwareInitError(String),
    
    #[error("Unknown error occurred")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, AogError>;

impl From<rppal::gpio::Error> for AogError {
    fn from(err: rppal::gpio::Error) -> Self {
        AogError::GpioError(err.to_string())
    }
}

impl From<reqwest::Error> for AogError {
    fn from(err: reqwest::Error) -> Self {
        AogError::HttpError(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for AogError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        AogError::LockError(format!("Mutex poisoned: {}", err))
    }
}

impl From<serde_json::Error> for AogError {
    fn from(err: serde_json::Error) -> Self {
        AogError::ConfigError(err.to_string())
    }
}

pub struct ErrorContext {
    pub module: String,
    pub operation: String,
    pub details: Option<String>,
}

impl ErrorContext {
    pub fn new(module: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            operation: operation.into(),
            details: None,
        }
    }
    
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl AogError {
    pub fn can_recover(&self) -> bool {
        match self {
            AogError::SensorError(_) => true,
            AogError::HttpError(_) => true,
            AogError::TimeoutError(_) => true,
            AogError::I2cError(_) => true,
            AogError::SerialError(_) => true,
            AogError::SafetyError(_) => false,
            AogError::HardwareInitError(_) => false,
            _ => false,
        }
    }
    
    pub fn recovery_action(&self) -> Option<String> {
        match self {
            AogError::SensorError(_) => Some("Retry sensor reading after delay".to_string()),
            AogError::HttpError(_) => Some("Retry HTTP request with exponential backoff".to_string()),
            AogError::TimeoutError(_) => Some("Increase timeout and retry".to_string()),
            AogError::I2cError(_) => Some("Reset I2C bus and retry".to_string()),
            AogError::SerialError(_) => Some("Reconnect serial port and retry".to_string()),
            _ => None,
        }
    }
}

pub fn log_error_with_context(error: &AogError, context: &ErrorContext) {
    let base_msg = format!(
        "[{}::{}] Error: {}", 
        context.module, 
        context.operation, 
        error
    );
    
    let full_msg = if let Some(details) = &context.details {
        format!("{} - Details: {}", base_msg, details)
    } else {
        base_msg
    };
    
    if error.can_recover() {
        if let Some(action) = error.recovery_action() {
            log::error!("{} | Recovery: {}", full_msg, action);
        } else {
            log::error!("{} | Recoverable error", full_msg);
        }
    } else {
        log::error!("{} | Non-recoverable error", full_msg);
    }
    
    // Log to error monitor
    crate::aog::error_monitor::log_error(
        &context.module,
        &context.operation,
        error,
        context.details.clone()
    );
}

#[macro_export]
macro_rules! retry_with_backoff {
    ($operation:expr, $max_retries:expr, $initial_delay_ms:expr) => {{
        use std::thread;
        use std::time::Duration;
        
        let mut retries = 0;
        let mut delay_ms = $initial_delay_ms;
        
        loop {
            match $operation {
                Ok(result) => break Ok(result),
                Err(e) if retries < $max_retries => {
                    log::warn!("Operation failed (attempt {}/{}): {}", retries + 1, $max_retries, e);
                    thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms *= 2;
                    retries += 1;
                }
                Err(e) => {
                    log::error!("Operation failed after {} retries: {}", $max_retries, e);
                    break Err(e);
                }
            }
        }
    }};
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;