use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::aog::error::AogError;

const MAX_ERROR_HISTORY: usize = 100;
const MAX_ERRORS_PER_MODULE: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: u64,
    pub module: String,
    pub operation: String,
    pub error_type: String,
    pub message: String,
    pub recoverable: bool,
    pub recovery_action: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    pub total_errors: u64,
    pub recoverable_errors: u64,
    pub non_recoverable_errors: u64,
    pub errors_last_hour: u64,
    pub errors_last_24h: u64,
    pub most_common_errors: Vec<(String, u64)>,
    pub modules_with_errors: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthStatus {
    pub overall_health: String, // "healthy", "degraded", "critical"
    pub uptime_seconds: u64,
    pub last_error: Option<ErrorEntry>,
    pub critical_errors_count: u64,
    pub warnings_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMonitoringDashboard {
    pub system_health: SystemHealthStatus,
    pub statistics: ErrorStatistics,
    pub recent_errors: Vec<ErrorEntry>,
    pub module_status: Vec<ModuleStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStatus {
    pub name: String,
    pub status: String, // "operational", "degraded", "failed"
    pub last_error: Option<String>,
    pub error_count: u64,
}

pub struct ErrorMonitor {
    error_history: Arc<Mutex<VecDeque<ErrorEntry>>>,
    total_errors: Arc<Mutex<u64>>,
    recoverable_count: Arc<Mutex<u64>>,
    non_recoverable_count: Arc<Mutex<u64>>,
    startup_time: SystemTime,
}

impl ErrorMonitor {
    pub fn new() -> Self {
        ErrorMonitor {
            error_history: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ERROR_HISTORY))),
            total_errors: Arc::new(Mutex::new(0)),
            recoverable_count: Arc::new(Mutex::new(0)),
            non_recoverable_count: Arc::new(Mutex::new(0)),
            startup_time: SystemTime::now(),
        }
    }
    
    pub fn log_error(&self, module: &str, operation: &str, error: &AogError, context: Option<String>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let error_type = match error {
            AogError::GpioError(_) => "GPIO",
            AogError::PumpError(_) => "Pump",
            AogError::RelayError(_) => "Relay",
            AogError::SensorError(_) => "Sensor",
            AogError::ConfigError(_) => "Configuration",
            AogError::HttpError(_) => "HTTP",
            AogError::I2cError(_) => "I2C",
            AogError::SerialError(_) => "Serial",
            AogError::LockError(_) => "Lock",
            AogError::IoError(_) => "IO",
            AogError::ThreadError(_) => "Thread",
            AogError::TimeoutError(_) => "Timeout",
            AogError::SafetyError(_) => "Safety",
            AogError::HardwareInitError(_) => "Hardware Init",
            AogError::Unknown => "Unknown",
        }.to_string();
        
        let entry = ErrorEntry {
            timestamp,
            module: module.to_string(),
            operation: operation.to_string(),
            error_type,
            message: error.to_string(),
            recoverable: error.can_recover(),
            recovery_action: error.recovery_action(),
            context,
        };
        
        if let Ok(mut history) = self.error_history.lock() {
            if history.len() >= MAX_ERROR_HISTORY {
                history.pop_front();
            }
            history.push_back(entry);
        }
        
        if let Ok(mut total) = self.total_errors.lock() {
            *total += 1;
        }
        
        if error.can_recover() {
            if let Ok(mut count) = self.recoverable_count.lock() {
                *count += 1;
            }
        } else {
            if let Ok(mut count) = self.non_recoverable_count.lock() {
                *count += 1;
            }
        }
    }
    
    fn get_empty_dashboard(&self) -> ErrorMonitoringDashboard {
        ErrorMonitoringDashboard {
            system_health: SystemHealthStatus {
                overall_health: "unknown".to_string(),
                uptime_seconds: 0,
                last_error: None,
                critical_errors_count: 0,
                warnings_count: 0,
            },
            statistics: ErrorStatistics {
                total_errors: 0,
                recoverable_errors: 0,
                non_recoverable_errors: 0,
                errors_last_hour: 0,
                errors_last_24h: 0,
                most_common_errors: vec![],
                modules_with_errors: vec![],
            },
            recent_errors: vec![],
            module_status: vec![],
        }
    }
    
    pub fn get_dashboard(&self) -> ErrorMonitoringDashboard {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let uptime = self.startup_time
            .elapsed()
            .unwrap_or_default()
            .as_secs();
        
        let history = match self.error_history.lock() {
            Ok(h) => h,
            Err(_) => return self.get_empty_dashboard(),
        };
        
        let recent_errors: Vec<ErrorEntry> = history.iter()
            .rev()
            .take(20)
            .cloned()
            .collect();
        
        let last_error = recent_errors.first().cloned();
        
        // Calculate statistics
        let hour_ago = now - 3600;
        let day_ago = now - 86400;
        
        let errors_last_hour = history.iter()
            .filter(|e| e.timestamp >= hour_ago)
            .count() as u64;
        
        let errors_last_24h = history.iter()
            .filter(|e| e.timestamp >= day_ago)
            .count() as u64;
        
        // Count errors by type
        let mut error_counts = std::collections::HashMap::new();
        let mut module_counts = std::collections::HashMap::new();
        
        for error in history.iter() {
            *error_counts.entry(error.error_type.clone()).or_insert(0) += 1;
            *module_counts.entry(error.module.clone()).or_insert(0) += 1;
        }
        
        let mut most_common_errors: Vec<(String, u64)> = error_counts.into_iter().collect();
        most_common_errors.sort_by(|a, b| b.1.cmp(&a.1));
        most_common_errors.truncate(5);
        
        let mut modules_with_errors: Vec<(String, u64)> = module_counts.clone().into_iter().collect();
        modules_with_errors.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Determine module status
        let module_status: Vec<ModuleStatus> = modules_with_errors.iter()
            .map(|(module, count)| {
                let module_errors: Vec<&ErrorEntry> = history.iter()
                    .filter(|e| e.module == *module)
                    .collect();
                
                let last_module_error = module_errors.last().map(|e| e.message.clone());
                
                let status = if *count == 0 {
                    "operational"
                } else if *count < 5 {
                    "degraded"
                } else {
                    "failed"
                }.to_string();
                
                ModuleStatus {
                    name: module.clone(),
                    status,
                    last_error: last_module_error,
                    error_count: *count,
                }
            })
            .collect();
        
        let total_errors = self.total_errors.lock().map(|g| *g).unwrap_or(0);
        let recoverable_errors = self.recoverable_count.lock().map(|g| *g).unwrap_or(0);
        let non_recoverable_errors = self.non_recoverable_count.lock().map(|g| *g).unwrap_or(0);
        
        // Determine overall health
        let overall_health = if non_recoverable_errors > 0 || errors_last_hour > 10 {
            "critical"
        } else if errors_last_hour > 5 {
            "degraded"
        } else {
            "healthy"
        }.to_string();
        
        let critical_errors_count = history.iter()
            .filter(|e| !e.recoverable)
            .count() as u64;
        
        let warnings_count = history.iter()
            .filter(|e| e.recoverable)
            .count() as u64;
        
        ErrorMonitoringDashboard {
            system_health: SystemHealthStatus {
                overall_health,
                uptime_seconds: uptime,
                last_error,
                critical_errors_count,
                warnings_count,
            },
            statistics: ErrorStatistics {
                total_errors,
                recoverable_errors,
                non_recoverable_errors,
                errors_last_hour,
                errors_last_24h,
                most_common_errors,
                modules_with_errors,
            },
            recent_errors,
            module_status,
        }
    }
}

lazy_static::lazy_static! {
    pub static ref GLOBAL_ERROR_MONITOR: ErrorMonitor = ErrorMonitor::new();
}

pub fn log_error(module: &str, operation: &str, error: &AogError, context: Option<String>) {
    GLOBAL_ERROR_MONITOR.log_error(module, operation, error, context);
}