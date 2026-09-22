use qwiic_relay_rs::{QwiicRelay, QwiicRelayConfig};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_RETRY_ATTEMPTS: u32 = 5;
const INITIAL_RETRY_DELAY_MS: u64 = 100;
const MAX_RETRY_DELAY_MS: u64 = 5000;
const HEALTH_CHECK_INTERVAL_SECS: u64 = 30;
const MAX_CONSECUTIVE_FAILURES: u32 = 10;

#[derive(Debug, Clone)]
pub enum RelayError {
    CommunicationFailure(String),
    FirmwareIncompatible(String),
    InitializationFailure(String),
    OperationFailure(String),
}

impl std::fmt::Display for RelayError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RelayError::CommunicationFailure(msg) => write!(f, "Communication failure: {}", msg),
            RelayError::FirmwareIncompatible(msg) => write!(f, "Firmware incompatible: {}", msg),
            RelayError::InitializationFailure(msg) => write!(f, "Initialization failure: {}", msg),
            RelayError::OperationFailure(msg) => write!(f, "Operation failure: {}", msg),
        }
    }
}

impl std::error::Error for RelayError {}

// AOG uses a quad relay. Register 0x04 toggles relay 4 on these boards;
// get_version() in qwiic-relay-rs is valid only for the single-relay board.
// Keep startup and health checks limited to OFF and status operations.
trait QuadRelayIo {
    fn turn_all_off(&mut self) -> Result<(), RelayError>;
    fn read_state(&mut self, channel: u8) -> Result<bool, RelayError>;
}

impl QuadRelayIo for QwiicRelay {
    fn turn_all_off(&mut self) -> Result<(), RelayError> {
        self.set_all_relays_off().map_err(|error| {
            RelayError::OperationFailure(format!("Failed to turn off relays: {}", error))
        })
    }

    fn read_state(&mut self, channel: u8) -> Result<bool, RelayError> {
        self.get_relay_state(Some(channel)).map_err(|error| {
            RelayError::CommunicationFailure(format!("Cannot read relay {}: {}", channel, error))
        })
    }
}

fn read_quad_states(relay: &mut impl QuadRelayIo) -> Result<[bool; 4], RelayError> {
    let mut states = [false; 4];
    for (index, state) in states.iter_mut().enumerate() {
        *state = relay.read_state((index + 1) as u8)?;
    }
    Ok(states)
}

fn initialize_quad_relay(relay: &mut impl QuadRelayIo) -> Result<(), RelayError> {
    relay.turn_all_off()?;
    // Retain the board's existing settling interval before verifying outputs.
    thread::sleep(Duration::from_secs(2));
    if read_quad_states(relay)?.iter().any(|on| *on) {
        return Err(RelayError::OperationFailure(
            "A relay remained on after startup shutdown".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RelayHealthStatus {
    pub is_healthy: bool,
    pub last_successful_operation: Option<Instant>,
    pub consecutive_failures: u32,
    pub firmware_version: Option<f32>,
    pub last_error: Option<RelayError>,
}

impl Default for RelayHealthStatus {
    fn default() -> Self {
        RelayHealthStatus {
            is_healthy: false,
            last_successful_operation: None,
            consecutive_failures: 0,
            firmware_version: None,
            last_error: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecoveryConfig {
    pub enable_auto_recovery: bool,
    pub enable_system_reboot: bool,
    pub max_retry_attempts: u32,
    pub health_check_interval_secs: u64,
    pub max_consecutive_failures: u32,
    pub alert_webhook_url: Option<String>,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        RecoveryConfig {
            enable_auto_recovery: true,
            enable_system_reboot: true,
            max_retry_attempts: MAX_RETRY_ATTEMPTS,
            health_check_interval_secs: HEALTH_CHECK_INTERVAL_SECS,
            max_consecutive_failures: MAX_CONSECUTIVE_FAILURES,
            alert_webhook_url: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QwiicRelayDevice {
    pub id: u16,
    pub aux_tank_pump_relay_id: Option<u16>,
    pub grow_light_relay_id: Option<u16>,
    pub water_pump_relay_id: Option<u16>,
    pub water_drain_relay_id: Option<u16>,
    pub air_circulation_relay_id: Option<u16>,
    #[serde(skip)]
    health_status: Arc<Mutex<RelayHealthStatus>>,
    #[serde(skip)]
    recovery_config: RecoveryConfig,
}

impl QwiicRelayDevice {
    pub fn new(id: u16) -> QwiicRelayDevice {
        QwiicRelayDevice {
            id,
            aux_tank_pump_relay_id: Some(4),
            grow_light_relay_id: Some(1),
            water_pump_relay_id: Some(3),
            water_drain_relay_id: Some(2),
            air_circulation_relay_id: Some(1),
            health_status: Arc::new(Mutex::new(RelayHealthStatus::default())),
            recovery_config: RecoveryConfig::default(),
        }
    }

    pub fn new_with_config(id: u16, recovery_config: RecoveryConfig) -> QwiicRelayDevice {
        QwiicRelayDevice {
            id,
            aux_tank_pump_relay_id: Some(4),
            grow_light_relay_id: Some(1),
            water_pump_relay_id: Some(3),
            water_drain_relay_id: Some(2),
            air_circulation_relay_id: Some(1),
            health_status: Arc::new(Mutex::new(RelayHealthStatus::default())),
            recovery_config,
        }
    }

    fn execute_with_retry<F, T>(&self, operation: F, operation_name: &str) -> Result<T, RelayError>
    where
        F: Fn() -> Result<T, RelayError>,
    {
        let mut attempts = 0;
        let mut delay_ms = INITIAL_RETRY_DELAY_MS;

        loop {
            attempts += 1;
            log::debug!(
                "Attempting {} (attempt {}/{})",
                operation_name,
                attempts,
                self.recovery_config.max_retry_attempts
            );

            match operation() {
                Ok(result) => {
                    log::debug!("{} succeeded on attempt {}", operation_name, attempts);
                    self.update_health_status(true, None);
                    return Ok(result);
                }
                Err(err) => {
                    log::warn!("{} failed on attempt {}: {}", operation_name, attempts, err);

                    if attempts >= self.recovery_config.max_retry_attempts {
                        log::error!("{} failed after {} attempts", operation_name, attempts);
                        self.update_health_status(false, Some(err.clone()));
                        self.handle_critical_failure(operation_name, &err);
                        return Err(err);
                    }

                    log::info!("Retrying {} after {}ms delay", operation_name, delay_ms);
                    thread::sleep(Duration::from_millis(delay_ms));

                    delay_ms = (delay_ms * 2).min(MAX_RETRY_DELAY_MS);
                }
            }
        }
    }

    fn update_health_status(&self, success: bool, error: Option<RelayError>) {
        if let Ok(mut status) = self.health_status.lock() {
            if success {
                status.is_healthy = true;
                status.last_successful_operation = Some(Instant::now());
                status.consecutive_failures = 0;
                status.last_error = None;
            } else {
                status.is_healthy = false;
                status.consecutive_failures += 1;
                status.last_error = error;
            }
        }
    }

    fn handle_critical_failure(&self, operation_name: &str, error: &RelayError) {
        log::error!("Critical failure in {}: {}", operation_name, error);

        self.send_alert(&format!(
            "Critical relay failure: {} - {}",
            operation_name, error
        ));

        if let Ok(status) = self.health_status.lock() {
            if status.consecutive_failures >= self.recovery_config.max_consecutive_failures {
                log::error!(
                    "Maximum consecutive failures ({}) reached",
                    self.recovery_config.max_consecutive_failures
                );

                if self.recovery_config.enable_system_reboot {
                    self.trigger_system_reboot();
                }
            }
        }
    }

    fn send_alert(&self, message: &str) {
        log::error!("ALERT: {}", message);

        if let Some(webhook_url) = &self.recovery_config.alert_webhook_url {
            let webhook_url = webhook_url.clone();
            let message = message.to_string();

            thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let payload = serde_json::json!({
                    "text": format!("AOG Relay Alert: {}", message),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });

                if let Err(e) = client.post(&webhook_url).json(&payload).send() {
                    log::error!("Failed to send alert webhook: {}", e);
                }
            });
        }
    }

    fn trigger_system_reboot(&self) {
        log::error!("Triggering system reboot due to critical relay failure");
        self.send_alert("System reboot initiated due to relay communication failure");

        thread::sleep(Duration::from_secs(5));

        match Command::new("sudo").arg("reboot").output() {
            Ok(_) => log::info!("System reboot command executed"),
            Err(e) => log::error!("Failed to execute system reboot: {}", e),
        }
    }

    pub fn start_health_monitor(&self) {
        let health_status = Arc::clone(&self.health_status);
        let device_id = self.id;
        let interval = self.recovery_config.health_check_interval_secs;
        let recovery_config = self.recovery_config.clone();

        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(interval));

            let qwiic_relay_config = QwiicRelayConfig::default();
            match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", device_id) {
                Ok(mut qwiic_relay) => match read_quad_states(&mut qwiic_relay) {
                    Ok(_) => {
                        log::debug!("Quad relay health check successful");
                        if let Ok(mut status) = health_status.lock() {
                            status.is_healthy = true;
                            status.last_successful_operation = Some(Instant::now());
                            status.consecutive_failures = 0;
                            status.last_error = None;
                        }
                    }
                    Err(e) => {
                        log::warn!("Health check failed: {}", e);
                        if let Ok(mut status) = health_status.lock() {
                            status.is_healthy = false;
                            status.consecutive_failures += 1;

                            if status.consecutive_failures
                                >= recovery_config.max_consecutive_failures
                            {
                                log::error!("Health check: Maximum consecutive failures reached");
                            }
                        }
                    }
                },
                Err(e) => {
                    log::error!("Health check: Failed to connect to relay: {}", e);
                    if let Ok(mut status) = health_status.lock() {
                        status.is_healthy = false;
                        status.consecutive_failures += 1;
                    }
                }
            }
        });
    }

    pub fn get_health_status(&self) -> RelayHealthStatus {
        self.health_status
            .lock()
            .unwrap_or_else(|e| {
                log::error!("Failed to lock health status: {}", e);
                e.into_inner()
            })
            .clone()
    }

    pub fn test(&self) {
        if !self.recovery_config.enable_auto_recovery {
            self.test_legacy();
            return;
        }

        let result = self.execute_with_retry(
            || {
                let qwiic_relay_config = QwiicRelayConfig::default();
                match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
                    Ok(mut qwiic_relay) => {
                        initialize_quad_relay(&mut qwiic_relay)?;

                        log::info!("Relay test completed successfully");
                        Ok(())
                    }
                    Err(err) => Err(RelayError::InitializationFailure(format!(
                        "Failed to initialize relay: {}",
                        err
                    ))),
                }
            },
            "relay test",
        );

        if result.is_ok() {
            self.start_health_monitor();
        }
    }

    fn test_legacy(&self) {
        let qwiic_relay_config = QwiicRelayConfig::default();
        let qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
        match qwiic_relay_d {
            Ok(mut qwiic_relay) => match initialize_quad_relay(&mut qwiic_relay) {
                Ok(()) => log::info!("All four relays are off"),
                Err(error) => log::error!("Relay initialization failed: {}", error),
            },
            Err(err) => {
                log::error!("{}", err);
            }
        }
    }

    pub fn all_off(&self) {
        if !self.recovery_config.enable_auto_recovery {
            self.all_off_legacy();
            return;
        }

        let _ = self.execute_with_retry(
            || {
                let qwiic_relay_config = QwiicRelayConfig::default();
                match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
                    Ok(mut qwiic_relay) => qwiic_relay.set_all_relays_off().map_err(|e| {
                        RelayError::OperationFailure(format!(
                            "Failed to turn off all relays: {}",
                            e
                        ))
                    }),
                    Err(err) => Err(RelayError::InitializationFailure(format!(
                        "Failed to initialize relay: {}",
                        err
                    ))),
                }
            },
            "all relays off",
        );
    }

    fn all_off_legacy(&self) {
        let qwiic_relay_config = QwiicRelayConfig::default();
        let qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
        match qwiic_relay_d {
            Ok(mut qwiic_relay) => match qwiic_relay.set_all_relays_off() {
                Ok(_) => log::debug!("All relays turned off"),
                Err(e) => log::error!("Failed to turn off all relays: {}", e),
            },
            Err(err) => {
                log::error!("{}", err);
            }
        }
    }

    pub fn set_relay(&self, relay_id: u16, state: bool) -> Result<(), RelayError> {
        if !self.recovery_config.enable_auto_recovery {
            return self.set_relay_legacy(relay_id, state);
        }

        self.execute_with_retry(
            || {
                let qwiic_relay_config = QwiicRelayConfig::default();
                match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
                    Ok(mut qwiic_relay) => {
                        if state {
                            qwiic_relay.set_relay_on(Some(relay_id as u8)).map_err(|e| {
                                RelayError::OperationFailure(format!(
                                    "Failed to turn on relay {}: {}",
                                    relay_id, e
                                ))
                            })
                        } else {
                            qwiic_relay
                                .set_relay_off(Some(relay_id as u8))
                                .map_err(|e| {
                                    RelayError::OperationFailure(format!(
                                        "Failed to turn off relay {}: {}",
                                        relay_id, e
                                    ))
                                })
                        }
                    }
                    Err(err) => Err(RelayError::InitializationFailure(format!(
                        "Failed to initialize relay: {}",
                        err
                    ))),
                }
            },
            &format!("set relay {} to {}", relay_id, state),
        )
    }

    fn set_relay_legacy(&self, relay_id: u16, state: bool) -> Result<(), RelayError> {
        let qwiic_relay_config = QwiicRelayConfig::default();
        match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
            Ok(mut qwiic_relay) => {
                let result = if state {
                    qwiic_relay.set_relay_on(Some(relay_id as u8))
                } else {
                    qwiic_relay.set_relay_off(Some(relay_id as u8))
                };

                result.map_err(|e| {
                    RelayError::OperationFailure(format!("Failed to set relay {}: {}", relay_id, e))
                })
            }
            Err(err) => {
                log::error!("{}", err);
                Err(RelayError::InitializationFailure(format!(
                    "Failed to initialize relay: {}",
                    err
                )))
            }
        }
    }

    pub fn get_relay_state(&self, relay_id: u16) -> Result<bool, RelayError> {
        if !self.recovery_config.enable_auto_recovery {
            return self.get_relay_state_legacy(relay_id);
        }

        self.execute_with_retry(
            || {
                let qwiic_relay_config = QwiicRelayConfig::default();
                match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
                    Ok(mut qwiic_relay) => qwiic_relay
                        .get_relay_state(Some(relay_id as u8))
                        .map_err(|e| {
                            RelayError::OperationFailure(format!(
                                "Failed to get relay {} state: {}",
                                relay_id, e
                            ))
                        }),
                    Err(err) => Err(RelayError::InitializationFailure(format!(
                        "Failed to initialize relay: {}",
                        err
                    ))),
                }
            },
            &format!("get relay {} state", relay_id),
        )
    }

    fn get_relay_state_legacy(&self, relay_id: u16) -> Result<bool, RelayError> {
        let qwiic_relay_config = QwiicRelayConfig::default();
        match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id) {
            Ok(mut qwiic_relay) => qwiic_relay
                .get_relay_state(Some(relay_id as u8))
                .map_err(|e| {
                    RelayError::OperationFailure(format!(
                        "Failed to get relay {} state: {}",
                        relay_id, e
                    ))
                }),
            Err(err) => {
                log::error!("{}", err);
                Err(RelayError::InitializationFailure(format!(
                    "Failed to initialize relay: {}",
                    err
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeQuadRelay {
        states: [bool; 4],
        operations: Vec<String>,
        stuck_on: bool,
        read_failure: Option<u8>,
        off_failure: bool,
    }

    impl QuadRelayIo for FakeQuadRelay {
        fn turn_all_off(&mut self) -> Result<(), RelayError> {
            self.operations.push("off".into());
            if self.off_failure {
                return Err(RelayError::OperationFailure("disconnected".into()));
            }
            if !self.stuck_on {
                self.states.fill(false);
            }
            Ok(())
        }

        fn read_state(&mut self, channel: u8) -> Result<bool, RelayError> {
            // Only channel status registers 0x05..0x08 may be read.
            assert!((1..=4).contains(&channel));
            self.operations.push(format!("read {}", channel));
            if self.read_failure == Some(channel) {
                return Err(RelayError::CommunicationFailure("disconnected".into()));
            }
            Ok(self.states[usize::from(channel - 1)])
        }
    }

    fn fake_quad() -> FakeQuadRelay {
        FakeQuadRelay {
            states: [true, false, true, false],
            operations: Vec::new(),
            stuck_on: false,
            read_failure: None,
            off_failure: false,
        }
    }

    #[test]
    fn quad_health_check_preserves_output_states() {
        let mut relay = fake_quad();
        let original = relay.states;
        assert_eq!(read_quad_states(&mut relay).unwrap(), original);
        assert_eq!(relay.states, original);
        assert_eq!(relay.operations, ["read 1", "read 2", "read 3", "read 4"]);
    }

    #[test]
    fn quad_startup_switches_off_before_reading_status() {
        let mut relay = fake_quad();
        initialize_quad_relay(&mut relay).unwrap();
        assert_eq!(relay.states, [false; 4]);
        assert_eq!(
            relay.operations,
            ["off", "read 1", "read 2", "read 3", "read 4"]
        );
    }

    #[test]
    fn quad_startup_detects_a_stuck_output() {
        let mut relay = fake_quad();
        relay.stuck_on = true;
        assert!(matches!(
            initialize_quad_relay(&mut relay),
            Err(RelayError::OperationFailure(_))
        ));
    }

    #[test]
    fn quad_checks_propagate_bus_failures() {
        let mut relay = fake_quad();
        relay.off_failure = true;
        assert!(initialize_quad_relay(&mut relay).is_err());
        assert_eq!(relay.operations, ["off"]);
        relay.operations.clear();
        relay.read_failure = Some(2);
        assert!(matches!(
            read_quad_states(&mut relay),
            Err(RelayError::CommunicationFailure(_))
        ));
        assert_eq!(relay.operations, ["read 1", "read 2"]);
    }

    #[test]
    fn test_relay_error_display() {
        let error = RelayError::CommunicationFailure("Test error".to_string());
        assert_eq!(format!("{}", error), "Communication failure: Test error");
    }

    #[test]
    fn test_health_status_default() {
        let status = RelayHealthStatus::default();
        assert!(!status.is_healthy);
        assert_eq!(status.consecutive_failures, 0);
        assert!(status.firmware_version.is_none());
    }

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert!(config.enable_auto_recovery);
        assert!(config.enable_system_reboot);
        assert_eq!(config.max_retry_attempts, MAX_RETRY_ATTEMPTS);
    }

    #[test]
    fn test_qwiic_relay_device_new() {
        let device = QwiicRelayDevice::new(0x25);
        assert_eq!(device.id, 0x25);
        assert_eq!(device.aux_tank_pump_relay_id, Some(4));
        assert_eq!(device.grow_light_relay_id, Some(1));
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let mut delay = INITIAL_RETRY_DELAY_MS;
        let expected_delays = [100, 200, 400, 800, 1600, 3200, 5000, 5000];

        for expected in &expected_delays {
            assert_eq!(delay, *expected);
            delay = (delay * 2).min(MAX_RETRY_DELAY_MS);
        }
    }
}
