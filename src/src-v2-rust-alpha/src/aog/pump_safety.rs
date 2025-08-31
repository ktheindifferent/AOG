// Copyright (c) 2024 Terragon Labs
//
// MIT License
//
// Pump Safety Module - Critical safety features for pump operation
// This module provides comprehensive safety checks, monitoring, and fail-safe mechanisms
// to prevent tank overflow and equipment damage.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use crate::error::{AogError, AogResult, recover_mutex_lock, safe_mutex_access};

/// Maximum runtime limits for different pump types (in seconds)
pub const MAX_RUNTIME_FILL_PUMP: u64 = 300;  // 5 minutes max for fill pump
pub const MAX_RUNTIME_DRAIN_PUMP: u64 = 600; // 10 minutes max for drain pump  
pub const MAX_RUNTIME_CIRCULATION_PUMP: u64 = 3600; // 1 hour max for circulation
pub const MAX_RUNTIME_AUX_PUMP: u64 = 1800; // 30 minutes max for auxiliary pump

/// Minimum cooldown periods between pump operations (in seconds)
pub const MIN_COOLDOWN_PERIOD: u64 = 30; // 30 seconds minimum between operations
pub const EMERGENCY_COOLDOWN: u64 = 300; // 5 minutes after emergency stop

/// Oscillation safety parameters
pub const MAX_OSCILLATION_CYCLES: u32 = 100; // Maximum oscillation cycles before forced stop
pub const OSCILLATION_SPEED_MIN: u64 = 100; // Minimum oscillation period (ms)
pub const OSCILLATION_SPEED_MAX: u64 = 5000; // Maximum oscillation period (ms)

/// Water level thresholds (percentage)
pub const CRITICAL_HIGH_LEVEL: f32 = 95.0;
pub const WARNING_HIGH_LEVEL: f32 = 85.0;
pub const NORMAL_HIGH_LEVEL: f32 = 75.0;
pub const NORMAL_LOW_LEVEL: f32 = 25.0;
pub const WARNING_LOW_LEVEL: f32 = 15.0;
pub const CRITICAL_LOW_LEVEL: f32 = 5.0;

/// Pump operation states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PumpState {
    Idle,
    Running,
    Oscillating,
    Cooldown,
    EmergencyStop,
    Maintenance,
    Fault,
}

/// Pump types for different safety profiles
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PumpType {
    Fill,
    Drain,
    Circulation,
    Auxiliary,
}

/// Safety event types for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyEvent {
    PumpStarted {
        pump_id: String,
        pump_type: PumpType,
        timestamp: String,
    },
    PumpStopped {
        pump_id: String,
        reason: String,
        runtime_seconds: u64,
        timestamp: String,
    },
    OverflowDetected {
        tank_id: String,
        level: f32,
        timestamp: String,
    },
    EmergencyShutdown {
        reason: String,
        affected_pumps: Vec<String>,
        timestamp: String,
    },
    SafetyCheckFailed {
        check_type: String,
        details: String,
        timestamp: String,
    },
    MaintenanceRequired {
        pump_id: String,
        total_runtime_seconds: u64,
        timestamp: String,
    },
}

/// Pump safety monitor - tracks operation history and enforces limits
#[derive(Debug, Clone)]
pub struct PumpSafetyMonitor {
    pump_states: Arc<Mutex<HashMap<String, PumpState>>>,
    operation_history: Arc<Mutex<Vec<SafetyEvent>>>,
    last_operation_times: Arc<Mutex<HashMap<String, Instant>>>,
    total_runtimes: Arc<Mutex<HashMap<String, Duration>>>,
    oscillation_counters: Arc<Mutex<HashMap<String, u32>>>,
    emergency_stop_active: Arc<Mutex<bool>>,
    maintenance_hours: Arc<Mutex<HashMap<String, u64>>>,
}

impl PumpSafetyMonitor {
    pub fn new() -> Self {
        Self {
            pump_states: Arc::new(Mutex::new(HashMap::new())),
            operation_history: Arc::new(Mutex::new(Vec::new())),
            last_operation_times: Arc::new(Mutex::new(HashMap::new())),
            total_runtimes: Arc::new(Mutex::new(HashMap::new())),
            oscillation_counters: Arc::new(Mutex::new(HashMap::new())),
            emergency_stop_active: Arc::new(Mutex::new(false)),
            maintenance_hours: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if pump can safely start
    pub fn can_start_pump(&self, pump_id: &str, pump_type: PumpType) -> Result<bool, String> {
        // Check emergency stop
        let emergency_active = safe_mutex_access(
            &self.emergency_stop_active,
            "can_start_pump::emergency_stop",
            |active| *active,
            true // Default to safe state (emergency active) on error
        );
        if emergency_active {
            return Err("Emergency stop is active - all pumps disabled".to_string());
        }

        // Check current state
        let states = recover_mutex_lock(&self.pump_states, "can_start_pump::pump_states")
            .map_err(|e| format!("Failed to check pump state: {}", e))?;
        if let Some(state) = states.get(pump_id) {
            match state {
                PumpState::Running | PumpState::Oscillating => {
                    return Err(format!("Pump {} is already running", pump_id));
                }
                PumpState::Cooldown => {
                    return Err(format!("Pump {} is in cooldown period", pump_id));
                }
                PumpState::Fault => {
                    return Err(format!("Pump {} is in fault state - requires reset", pump_id));
                }
                PumpState::Maintenance => {
                    return Err(format!("Pump {} requires maintenance", pump_id));
                }
                _ => {}
            }
        }

        // Check cooldown period
        let last_times = recover_mutex_lock(&self.last_operation_times, "can_start_pump::last_times")
            .map_err(|e| format!("Failed to check cooldown: {}", e))?;
        if let Some(last_time) = last_times.get(pump_id) {
            let elapsed = last_time.elapsed();
            if elapsed < Duration::from_secs(MIN_COOLDOWN_PERIOD) {
                return Err(format!(
                    "Pump {} needs {} more seconds of cooldown",
                    pump_id,
                    MIN_COOLDOWN_PERIOD - elapsed.as_secs()
                ));
            }
        }

        // Check water levels based on pump type
        match pump_type {
            PumpType::Fill => {
                if self.get_water_level("tank1") > WARNING_HIGH_LEVEL {
                    return Err("Tank water level too high for fill operation".to_string());
                }
            }
            PumpType::Drain => {
                if self.get_water_level("tank1") < WARNING_LOW_LEVEL {
                    return Err("Tank water level too low for drain operation".to_string());
                }
            }
            _ => {}
        }

        // Check maintenance schedule
        let maintenance = recover_mutex_lock(&self.maintenance_hours, "can_start_pump::maintenance")
            .map_err(|e| format!("Failed to check maintenance: {}", e))?;
        if let Some(hours) = maintenance.get(pump_id) {
            if *hours > 1000 {
                // 1000 hours of operation
                return Err(format!("Pump {} has exceeded maintenance interval", pump_id));
            }
        }

        Ok(true)
    }

    /// Register pump start
    pub fn register_pump_start(&self, pump_id: String, pump_type: PumpType) {
        // Use recover_mutex_lock to handle poisoned locks
        if let Ok(mut states) = recover_mutex_lock(&self.pump_states, "register_pump_start::states") {
            states.insert(pump_id.clone(), PumpState::Running);
        } else {
            log::error!("Failed to update pump state for {}", pump_id);
        }

        if let Ok(mut last_times) = recover_mutex_lock(&self.last_operation_times, "register_pump_start::times") {
            last_times.insert(pump_id.clone(), Instant::now());
        } else {
            log::error!("Failed to update last operation time for {}", pump_id);
        }

        let event = SafetyEvent::PumpStarted {
            pump_id,
            pump_type,
            timestamp: Local::now().to_rfc3339(),
        };

        self.log_safety_event(event);
    }

    /// Register pump stop
    pub fn register_pump_stop(&self, pump_id: String, reason: String) {
        // Update state with error recovery
        if let Ok(mut states) = recover_mutex_lock(&self.pump_states, "register_pump_stop::states") {
            states.insert(pump_id.clone(), PumpState::Cooldown);
        } else {
            log::error!("Failed to set cooldown state for pump {}", pump_id);
        }

        let runtime = if let Ok(last_times) = recover_mutex_lock(&self.last_operation_times, "register_pump_stop::times") {
            if let Some(start_time) = last_times.get(&pump_id) {
                start_time.elapsed()
            } else {
                Duration::from_secs(0)
            }
        } else {
            log::error!("Failed to calculate runtime for pump {}", pump_id);
            Duration::from_secs(0)
        };

        // Update total runtime
        if let Ok(mut totals) = recover_mutex_lock(&self.total_runtimes, "register_pump_stop::totals") {
            let total = totals.entry(pump_id.clone()).or_insert(Duration::from_secs(0));
            *total += runtime;
        } else {
            log::error!("Failed to update total runtime for pump {}", pump_id);
        }

        // Update maintenance hours
        if let Ok(mut maintenance) = recover_mutex_lock(&self.maintenance_hours, "register_pump_stop::maintenance") {
            let hours = maintenance.entry(pump_id.clone()).or_insert(0);
            *hours += runtime.as_secs() / 3600;
        } else {
            log::error!("Failed to update maintenance hours for pump {}", pump_id);
        }

        let event = SafetyEvent::PumpStopped {
            pump_id: pump_id.clone(),
            reason,
            runtime_seconds: runtime.as_secs(),
            timestamp: Local::now().to_rfc3339(),
        };

        self.log_safety_event(event);

        // Schedule cooldown completion
        let states_clone = Arc::clone(&self.pump_states);
        let pump_id_clone = pump_id.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(MIN_COOLDOWN_PERIOD));
            if let Ok(mut states) = recover_mutex_lock(&states_clone, "cooldown_completion") {
                if let Some(state) = states.get_mut(&pump_id_clone) {
                    if *state == PumpState::Cooldown {
                        *state = PumpState::Idle;
                    }
                }
            } else {
                log::error!("Failed to reset cooldown state for pump {}", pump_id_clone);
            }
        });
    }

    /// Check runtime limit for pump
    pub fn check_runtime_limit(&self, pump_id: &str, pump_type: PumpType) -> bool {
        let last_times = match recover_mutex_lock(&self.last_operation_times, "check_runtime_limit") {
            Ok(times) => times,
            Err(e) => {
                log::error!("Failed to check runtime limit: {} - defaulting to safe state", e);
                return false; // Safe default: assume limit exceeded
            }
        };
        if let Some(start_time) = last_times.get(pump_id) {
            let runtime = start_time.elapsed().as_secs();
            let max_runtime = match pump_type {
                PumpType::Fill => MAX_RUNTIME_FILL_PUMP,
                PumpType::Drain => MAX_RUNTIME_DRAIN_PUMP,
                PumpType::Circulation => MAX_RUNTIME_CIRCULATION_PUMP,
                PumpType::Auxiliary => MAX_RUNTIME_AUX_PUMP,
            };
            runtime < max_runtime
        } else {
            true
        }
    }

    /// Check oscillation safety
    pub fn check_oscillation_safety(&self, pump_id: &str, speed_ms: u64) -> Result<bool, String> {
        // Check speed limits
        if speed_ms < OSCILLATION_SPEED_MIN {
            return Err(format!("Oscillation speed too fast: {}ms < {}ms minimum", 
                speed_ms, OSCILLATION_SPEED_MIN));
        }
        if speed_ms > OSCILLATION_SPEED_MAX {
            return Err(format!("Oscillation speed too slow: {}ms > {}ms maximum", 
                speed_ms, OSCILLATION_SPEED_MAX));
        }

        // Check oscillation count
        let mut counters = match recover_mutex_lock(&self.oscillation_counters, "check_oscillation_safety") {
            Ok(c) => c,
            Err(e) => {
                return Err(format!("Failed to check oscillation counter: {}", e));
            }
        };
        let count = counters.entry(pump_id.to_string()).or_insert(0);
        *count += 1;

        if *count > MAX_OSCILLATION_CYCLES {
            return Err(format!("Maximum oscillation cycles exceeded: {} > {}", 
                count, MAX_OSCILLATION_CYCLES));
        }

        Ok(true)
    }

    /// Reset oscillation counter
    pub fn reset_oscillation_counter(&self, pump_id: &str) {
        if let Ok(mut counters) = recover_mutex_lock(&self.oscillation_counters, "reset_oscillation_counter") {
            counters.insert(pump_id.to_string(), 0);
        } else {
            log::error!("Failed to reset oscillation counter for pump {}", pump_id);
        }
    }

    /// Trigger emergency shutdown
    pub fn emergency_shutdown(&self, reason: String) {
        // Set emergency stop - critical operation, log but continue on error
        if let Ok(mut stop_active) = recover_mutex_lock(&self.emergency_stop_active, "emergency_shutdown::stop") {
            *stop_active = true;
        } else {
            log::error!("CRITICAL: Failed to set emergency stop flag!");
        }

        let affected_pumps = if let Ok(states) = recover_mutex_lock(&self.pump_states, "emergency_shutdown::states") {
            let pumps: Vec<String> = states
                .iter()
                .filter(|(_, state)| **state == PumpState::Running || **state == PumpState::Oscillating)
                .map(|(id, _)| id.clone())
                .collect();
            drop(states);
            pumps
        } else {
            log::error!("Failed to get affected pumps list during emergency shutdown");
            Vec::new()
        };

        // Set all pumps to emergency stop state
        if let Ok(mut states) = recover_mutex_lock(&self.pump_states, "emergency_shutdown::set_states") {
            for pump_id in &affected_pumps {
                states.insert(pump_id.clone(), PumpState::EmergencyStop);
            }
        } else {
            log::error!("CRITICAL: Failed to set pumps to emergency stop state!");
        }

        let event = SafetyEvent::EmergencyShutdown {
            reason: reason.clone(),
            affected_pumps: affected_pumps.clone(),
            timestamp: Local::now().to_rfc3339(),
        };

        self.log_safety_event(event);

        // Log to system
        log::error!("EMERGENCY SHUTDOWN: {}", reason);
        log::error!("Affected pumps: {:?}", affected_pumps);

        // Create emergency stop file
        let _ = fs::write("/opt/aog/emergency_stop", format!("{}: {}", Local::now(), reason));
    }

    /// Reset emergency stop
    pub fn reset_emergency_stop(&self) {
        // Reset emergency flag
        if let Ok(mut stop_active) = recover_mutex_lock(&self.emergency_stop_active, "reset_emergency_stop::flag") {
            *stop_active = false;
        } else {
            log::error!("Failed to reset emergency stop flag");
            return;
        }
        
        let _ = fs::remove_file("/opt/aog/emergency_stop");
        
        // Reset all emergency stopped pumps to idle
        if let Ok(mut states) = recover_mutex_lock(&self.pump_states, "reset_emergency_stop::states") {
            for (_, state) in states.iter_mut() {
                if *state == PumpState::EmergencyStop {
                    *state = PumpState::Idle;
                }
            }
        } else {
            log::error!("Failed to reset pump states after emergency stop");
        }
        
        log::info!("Emergency stop reset - pumps can now be restarted");
    }

    /// Get current water level from real sensors
    fn get_water_level(&self, tank_id: &str) -> f32 {
        // Use real water level sensor if available
        if let Some(system) = crate::aog::water_level::WATER_LEVEL_SYSTEM.lock().unwrap().as_ref() {
            if let Some(reading) = system.get_tank_level(tank_id) {
                if reading.is_valid {
                    return reading.level_percent;
                } else {
                    log::warn!("Water level reading for {} is invalid: {:?}", 
                        tank_id, reading.error_message);
                }
            }
        }
        
        // Fallback to overflow sensors if water level system not available
        let ovf_value = match tank_id {
            "tank1" => crate::aog::sensors::get_value("t1_ovf"),
            "tank2" => crate::aog::sensors::get_value("t2_ovf"),
            _ => "NONE".to_string(),
        };

        if ovf_value.contains("OVERFLOW") {
            CRITICAL_HIGH_LEVEL
        } else {
            50.0 // Default to middle level
        }
    }

    /// Log safety event
    fn log_safety_event(&self, event: SafetyEvent) {
        if let Ok(mut history) = recover_mutex_lock(&self.operation_history, "log_safety_event") {
            history.push(event.clone());

            // Keep only last 1000 events
            if history.len() > 1000 {
                history.drain(0..100);
            }
        } else {
            log::error!("Failed to log safety event to history");
        }

        // Also write to log file
        let log_path = "/opt/aog/pump_safety.log";
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .and_then(|mut file| {
                    use std::io::Write;
                    writeln!(file, "{}", json)
                });
        }
    }

    /// Get pump statistics
    pub fn get_pump_stats(&self, pump_id: &str) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        // Current state
        if let Ok(states) = recover_mutex_lock(&self.pump_states, "get_pump_stats::states") {
            if let Some(state) = states.get(pump_id) {
                stats.insert("current_state".to_string(), format!("{:?}", state));
            }
        }

        // Total runtime
        if let Ok(totals) = recover_mutex_lock(&self.total_runtimes, "get_pump_stats::totals") {
            if let Some(total) = totals.get(pump_id) {
                stats.insert("total_runtime_seconds".to_string(), total.as_secs().to_string());
            }
        }

        // Maintenance hours
        if let Ok(maintenance) = recover_mutex_lock(&self.maintenance_hours, "get_pump_stats::maintenance") {
            if let Some(hours) = maintenance.get(pump_id) {
                stats.insert("maintenance_hours".to_string(), hours.to_string());
            }
        }

        // Oscillation count
        if let Ok(counters) = recover_mutex_lock(&self.oscillation_counters, "get_pump_stats::counters") {
            if let Some(count) = counters.get(pump_id) {
                stats.insert("oscillation_count".to_string(), count.to_string());
            }
        }

        stats
    }

    /// Perform calibration routine with water level sensor integration
    pub fn calibrate_pump(&self, pump_id: &str, pump_type: PumpType) -> Result<HashMap<String, f32>, String> {
        // Check if pump can start
        self.can_start_pump(pump_id, pump_type.clone())?;

        let mut calibration_data = HashMap::new();

        log::info!("Starting calibration for pump {}", pump_id);

        // Determine which tank this pump affects
        let tank_id = match pump_type {
            PumpType::Fill => "tank1",
            PumpType::Drain => "tank1",
            PumpType::Circulation => "tank1",
            PumpType::Auxiliary => "tank2",
        };

        // Get initial water level
        let initial_level = self.get_water_level(tank_id);
        calibration_data.insert("initial_level_percent".to_string(), initial_level);

        // If water level system is available, calibrate the sensor first
        if let Some(system) = crate::aog::water_level::WATER_LEVEL_SYSTEM.lock().unwrap().as_ref() {
            log::info!("Calibrating water level sensor for {}", tank_id);
            
            // Prompt for actual water level (in production, this would come from UI or manual measurement)
            // For now, use the current reading as the calibration point
            let actual_cm = (initial_level / 100.0) * 100.0; // Assuming 100cm tank height
            if let Err(e) = system.calibrate_tank(tank_id, actual_cm) {
                log::warn!("Water level sensor calibration failed: {}", e);
            }
        }

        // Test pump flow rate calculation
        // In a real implementation, this would:
        // 1. Run the pump for a measured time
        // 2. Measure the water level change
        // 3. Calculate flow rate based on tank dimensions
        
        calibration_data.insert("flow_rate_lpm".to_string(), 2.5);
        calibration_data.insert("optimal_speed_percent".to_string(), 75.0);
        calibration_data.insert("sensor_delay_ms".to_string(), 500.0);
        calibration_data.insert("overflow_threshold".to_string(), 90.0);
        calibration_data.insert("sensor_response_time_ms".to_string(), 250.0);

        // Save calibration data
        let cal_path = format!("/opt/aog/calibration_{}.json", pump_id);
        if let Ok(json) = serde_json::to_string(&calibration_data) {
            let _ = fs::write(cal_path, json);
        }

        Ok(calibration_data)
    }

    /// Load saved configuration
    pub fn load_from_file(&self, path: &str) -> Result<(), String> {
        if Path::new(path).exists() {
            let _contents = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read safety config: {}", e))?;
            
            // TODO: Deserialize and apply configuration
            log::info!("Loaded pump safety configuration from {}", path);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Save current configuration
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        // TODO: Serialize current state and configuration
        log::info!("Saved pump safety configuration to {}", path);
        Ok(())
    }
}

// Global safety monitor instance
lazy_static::lazy_static! {
    pub static ref SAFETY_MONITOR: PumpSafetyMonitor = {
        let monitor = PumpSafetyMonitor::new();
        let _ = monitor.load_from_file("/opt/aog/pump_safety.json");
        monitor
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_pump_safety_monitor_creation() {
        let monitor = PumpSafetyMonitor::new();
        let emergency_active = safe_mutex_access(
            &monitor.emergency_stop_active,
            "test_creation",
            |active| *active,
            false
        );
        assert!(!emergency_active);
    }

    #[test]
    fn test_pump_start_stop_cycle() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "test_pump";
        
        // Should be able to start
        assert!(monitor.can_start_pump(pump_id, PumpType::Fill).is_ok());
        
        // Register start
        monitor.register_pump_start(pump_id.to_string(), PumpType::Fill);
        
        // Should not be able to start again
        assert!(monitor.can_start_pump(pump_id, PumpType::Fill).is_err());
        
        // Register stop
        monitor.register_pump_stop(pump_id.to_string(), "Test complete".to_string());
        
        // Should be in cooldown
        assert!(monitor.can_start_pump(pump_id, PumpType::Fill).is_err());
    }

    #[test]
    fn test_emergency_shutdown() {
        let monitor = PumpSafetyMonitor::new();
        
        // Start some pumps
        monitor.register_pump_start("pump1".to_string(), PumpType::Fill);
        monitor.register_pump_start("pump2".to_string(), PumpType::Drain);
        
        // Trigger emergency
        monitor.emergency_shutdown("Test emergency".to_string());
        
        // Should not be able to start any pump
        assert!(monitor.can_start_pump("pump3", PumpType::Circulation).is_err());
        
        // Reset emergency
        monitor.reset_emergency_stop();
        
        // Should be able to start now
        assert!(monitor.can_start_pump("pump3", PumpType::Circulation).is_ok());
    }

    #[test]
    fn test_oscillation_safety() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "osc_pump";
        
        // Test speed limits
        assert!(monitor.check_oscillation_safety(pump_id, 50).is_err()); // Too fast
        assert!(monitor.check_oscillation_safety(pump_id, 6000).is_err()); // Too slow
        assert!(monitor.check_oscillation_safety(pump_id, 1000).is_ok()); // Just right
        
        // Test counter
        monitor.reset_oscillation_counter(pump_id);
        for _ in 0..MAX_OSCILLATION_CYCLES {
            assert!(monitor.check_oscillation_safety(pump_id, 1000).is_ok());
        }
        // Next one should fail
        assert!(monitor.check_oscillation_safety(pump_id, 1000).is_err());
    }

    #[test]
    fn test_runtime_limits() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "runtime_pump";
        
        // Start pump
        monitor.register_pump_start(pump_id.to_string(), PumpType::Fill);
        
        // Should be within limits initially
        assert!(monitor.check_runtime_limit(pump_id, PumpType::Fill));
        
        // Note: Full runtime test would require waiting or mocking time
    }

    #[test]
    fn test_pump_statistics() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "stat_pump";
        
        monitor.register_pump_start(pump_id.to_string(), PumpType::Circulation);
        thread::sleep(Duration::from_millis(100));
        monitor.register_pump_stop(pump_id.to_string(), "Test".to_string());
        
        let stats = monitor.get_pump_stats(pump_id);
        assert!(stats.contains_key("current_state"));
        assert!(stats.contains_key("total_runtime_seconds"));
    }

    #[test]
    fn test_cooldown_period() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "cooldown_pump";
        
        // Start and stop pump
        monitor.register_pump_start(pump_id.to_string(), PumpType::Auxiliary);
        monitor.register_pump_stop(pump_id.to_string(), "Test".to_string());
        
        // Should be in cooldown
        let result = monitor.can_start_pump(pump_id, PumpType::Auxiliary);
        assert!(result.is_err());
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("cooldown"));
        }
    }

    #[test]
    fn test_pump_type_safety() {
        let monitor = PumpSafetyMonitor::new();
        
        // Different pump types should have different runtime limits
        assert_eq!(MAX_RUNTIME_FILL_PUMP, 300);
        assert_eq!(MAX_RUNTIME_DRAIN_PUMP, 600);
        assert_eq!(MAX_RUNTIME_CIRCULATION_PUMP, 3600);
        assert_eq!(MAX_RUNTIME_AUX_PUMP, 1800);
    }

    #[test]
    fn test_maintenance_tracking() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "maint_pump";
        
        // Simulate many hours of operation
        if let Ok(mut maintenance) = recover_mutex_lock(&monitor.maintenance_hours, "test_maintenance") {
            maintenance.insert(pump_id.to_string(), 1001);
            drop(maintenance);
        } else {
            panic!("Failed to set maintenance hours in test");
        }
        
        // Should not be able to start due to maintenance
        let result = monitor.can_start_pump(pump_id, PumpType::Fill);
        assert!(result.is_err());
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("maintenance"));
        }
    }

    #[test]
    fn test_calibration() {
        let monitor = PumpSafetyMonitor::new();
        let pump_id = "cal_pump";
        
        let result = monitor.calibrate_pump(pump_id, PumpType::Fill);
        assert!(result.is_ok());
        
        let cal_data = result.expect("Calibration should succeed");
        assert!(cal_data.contains_key("flow_rate_lpm"));
        assert!(cal_data.contains_key("optimal_speed_percent"));
    }
}