//! Dashboard output controls. Draining uses manual start/stop; other outputs
//! have watchdog-owned deadlines. All runs retain their configured interlocks.
use crate::{Config, OverflowConfig, PhConfig, PumpConfig, RelayRole, ScheduleConfig};
use chrono::{Local, Timelike};
use i2cdev::core::I2CDevice;
use i2cdev::linux::LinuxI2CDevice;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    pub setup: Option<SetupPatch>,
    pub pump: Option<PumpConfig>,
    pub ph: Option<PhConfig>,
    pub schedules: Option<ScheduleConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupPatch {
    pub setup_name: String,
    pub relay_labels: [String; 4],
    // Older clients can still change labels without resetting the wiring map.
    pub relay_roles: Option<[RelayRole; 4]>,
    pub overflow: Option<OverflowConfig>,
}

pub fn validate_roles(roles: &[RelayRole; 4]) -> Result<(), String> {
    for (index, role) in roles.iter().enumerate() {
        if *role != RelayRole::Unassigned && roles[..index].contains(role) {
            return Err(format!(
                "{} can be assigned to only one relay",
                role.label()
            ));
        }
    }
    Ok(())
}

impl SettingsPatch {
    pub fn apply(self, config: &mut Config) -> Result<(), String> {
        if self.setup.is_none()
            && self.pump.is_none()
            && self.ph.is_none()
            && self.schedules.is_none()
        {
            return Err("Choose settings to update".into());
        }
        if let Some(setup) = self.setup {
            for (name, value) in std::iter::once(("Setup name", &setup.setup_name)).chain(
                setup
                    .relay_labels
                    .iter()
                    .map(|label| ("Output label", label)),
            ) {
                if value.trim().is_empty()
                    || value.chars().count() > 60
                    || value.chars().any(char::is_control)
                {
                    return Err(format!("{} must contain 1–60 printable characters", name));
                }
            }
            config.dashboard_config.setup_name = setup.setup_name;
            config.dashboard_config.relay_labels = setup.relay_labels;
            if let Some(roles) = setup.relay_roles {
                config.dashboard_config.relay_roles = roles;
            }
            if let Some(overflow) = setup.overflow {
                if overflow.sensor_labels.iter().any(|label| {
                    label.trim().is_empty()
                        || label.chars().count() > 60
                        || label.chars().any(char::is_control)
                }) {
                    return Err("Sensor labels must contain 1–60 printable characters".into());
                }
                config.dashboard_config.overflow = overflow;
            }
        }
        if let Some(pump) = self.pump {
            if pump.continuous_mode {
                return Err(
                    "Filling and Tank-to-tank use timed runs; Draining already supports start/stop"
                        .into(),
                );
            }
            if !(1..=1800).contains(&pump.pump_runtime_limit_seconds) {
                return Err("Pump runtime limit must be 1–1800 seconds".into());
            }
            if !(30..=3600).contains(&pump.pump_cooldown_seconds) {
                return Err("Pump cooldown must be 30–3600 seconds".into());
            }
            if pump.photo_cycle_start_hour > 23
                || pump.photo_cycle_end_hour > 24
                || pump.photo_cycle_start_hour == pump.photo_cycle_end_hour
            {
                return Err("Operating window needs different hours: start 0–23, end 0–24".into());
            }
            if pump
                .safety_gpio_pin
                .is_some_and(|pin| !(4..=27).contains(&pin))
            {
                return Err("Safety input must use BCM GPIO 4–27; I²C pins are reserved".into());
            }
            config.pump_config = Some(pump);
        }
        if let Some(ph) = self.ph {
            let levels = [
                ph.critical_min,
                ph.optimal_min,
                ph.optimal_max,
                ph.critical_max,
            ];
            if levels
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=14.0).contains(value))
                || !levels.windows(2).all(|pair| pair[0] < pair[1])
            {
                return Err("pH limits must increase: critical low < target low < target high < critical high, within 0–14".into());
            }
            if !(5..=3600).contains(&ph.monitoring_interval_seconds)
                || !(10..=1440).contains(&ph.history_size)
            {
                return Err(
                    "Use a pH interval of 5–3600 seconds and a history size of 10–1440 readings"
                        .into(),
                );
            }
            if ph.sensor_type != "arduino" || ph.serial_port.is_some() || ph.i2c_address.is_some() {
                return Err(
                    "This dashboard currently supports the connected Arduino pH sensor".into(),
                );
            }
            config.ph_config = Some(ph);
        }
        if let Some(schedules) = self.schedules {
            config.schedules = schedules;
        }
        let schedules = &config.schedules;
        let roles = &config.dashboard_config.relay_roles;
        validate_roles(roles)?;
        if schedules.lighting.enabled && !roles.contains(&RelayRole::LightsAir) {
            return Err(
                "Assign a relay to Lights + Air before enabling the lighting schedule".into(),
            );
        }
        let watering_role = schedules.watering.pump.role();
        if schedules.watering.enabled && !roles.contains(&watering_role) {
            return Err(format!(
                "Assign a relay to {} before enabling its water schedule",
                watering_role.label()
            ));
        }
        for (start, end) in [
            (
                schedules.lighting.start_minute,
                schedules.lighting.end_minute,
            ),
            (
                schedules.watering.start_minute,
                schedules.watering.end_minute,
            ),
        ] {
            if start >= 1440 || end >= 1440 || start == end {
                return Err(
                    "Schedule start and end must be different times between 00:00 and 23:59".into(),
                );
            }
        }
        let pump = config.pump_config.clone().unwrap_or_default();
        if !(1..=1440).contains(&schedules.watering.every_minutes)
            || schedules.watering.run_seconds == 0
            || watering_role
                .maximum_seconds(1800)
                .is_some_and(|limit| schedules.watering.run_seconds > limit)
            || (schedules.watering.enabled
                && (watering_role
                    .maximum_seconds(pump.pump_runtime_limit_seconds)
                    .is_some_and(|limit| schedules.watering.run_seconds > limit)
                    || schedules.watering.every_minutes * 60
                        < schedules.watering.run_seconds + pump.pump_cooldown_seconds))
        {
            return Err("The water schedule must fit the selected pump's runtime limit, with enough time between runs for cooldown".into());
        }
        Ok(())
    }
}

pub trait RelayBoard: Send {
    fn states(&mut self) -> Result<[bool; 4], String>;
    fn set(&mut self, channel: u8, on: bool) -> Result<(), String>;
    fn all_off(&mut self) -> Result<(), String>;
}

pub struct QuadBoard;

fn open_board() -> Result<LinuxI2CDevice, String> {
    LinuxI2CDevice::new("/dev/i2c-1", 0x25)
        .map_err(|error| format!("Relay board unavailable: {}", error))
}

impl RelayBoard for QuadBoard {
    fn states(&mut self) -> Result<[bool; 4], String> {
        let mut device = open_board()?;
        let mut states = [false; 4];
        for (index, state) in states.iter_mut().enumerate() {
            *state = device
                .smbus_read_byte_data(0x05 + index as u8)
                .map_err(|e| e.to_string())?
                != 0;
        }
        Ok(states)
    }

    fn set(&mut self, channel: u8, on: bool) -> Result<(), String> {
        if !(1..=4).contains(&channel) {
            return Err("Output must be 1–4".into());
        }
        let mut device = open_board()?;
        let current = device
            .smbus_read_byte_data(0x04 + channel)
            .map_err(|e| e.to_string())?
            != 0;
        if current != on {
            // Quad boards toggle individual outputs. Never retry a toggle
            // without reading back the state first.
            device
                .smbus_write_byte(channel)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(20));
        }
        let actual = device
            .smbus_read_byte_data(0x04 + channel)
            .map_err(|e| e.to_string())?
            != 0;
        if actual != on {
            return Err("Relay did not reach the requested state".into());
        }
        Ok(())
    }

    fn all_off(&mut self) -> Result<(), String> {
        open_board()?
            .smbus_write_byte(0x0a)
            .map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(20));
        if self.states()?.iter().any(|on| *on) {
            return Err("A relay still reports ON after shutdown".into());
        }
        Ok(())
    }
}

struct Run {
    deadline: Option<Instant>,
    scheduled: bool,
}

pub struct OutputController {
    board: Box<dyn RelayBoard>,
    directory: PathBuf,
    pump: PumpConfig,
    roles: [RelayRole; 4],
    overflow: OverflowConfig,
    active: [Option<Run>; 4],
    stopped: [Option<Instant>; 4],
    stop_pending: bool,
    schedules: ScheduleConfig,
    automation_paused: bool,
    next_water: Instant,
    last_schedule_check: Option<Instant>,
    pub last_event: String,
}

#[derive(Serialize)]
pub struct OutputStatus {
    pub channel: u8,
    pub role: RelayRole,
    pub on: Option<bool>,
    pub remaining_seconds: Option<u64>,
    pub cooldown_seconds: u64,
    pub maximum_seconds: Option<u64>,
    pub blocked_reason: Option<String>,
    pub schedule_controlled: bool,
    pub overflow_sensors: [bool; 2],
}

#[derive(Serialize)]
pub struct ControllerStatus {
    pub outputs: Vec<OutputStatus>,
    pub error: Option<String>,
    pub pump_block_reason: Option<String>,
    pub last_event: String,
    pub stop_pending: bool,
    pub automation_paused: bool,
    pub next_watering_in_seconds: Option<u64>,
}

pub fn within_window(hour: u8, start: u8, end: u8) -> bool {
    if start < end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

fn in_schedule(minute: u16, start: u16, end: u16) -> bool {
    if start < end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

impl OutputController {
    pub fn new(
        directory: &Path,
        pump: PumpConfig,
        roles: [RelayRole; 4],
        board: Box<dyn RelayBoard>,
    ) -> Self {
        Self {
            board,
            directory: directory.into(),
            pump,
            roles,
            overflow: OverflowConfig::default(),
            active: std::array::from_fn(|_| None),
            stopped: [None; 4],
            stop_pending: false,
            last_event: "Ready for manual output control".into(),
            schedules: ScheduleConfig::default(),
            automation_paused: directory.join("automation_paused").exists(),
            next_water: Instant::now() + Duration::from_secs(3600),
            last_schedule_check: None,
        }
    }

    pub fn configure(&mut self, pump: PumpConfig) -> Result<(), String> {
        if self.active.iter().any(Option::is_some) || self.stop_pending {
            return Err("Stop active outputs before changing pump settings".into());
        }
        self.pump = pump;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.active.iter().any(Option::is_some) || self.stop_pending
    }

    pub fn prepare_reassignment(&mut self) -> Result<(), String> {
        if self.is_running() || self.board.states()?.iter().any(|on| *on) {
            return Err("Stop all outputs before changing relay or overflow assignments".into());
        }
        self.pause_schedules()?;
        self.last_event =
            "Relay or overflow assignments changed; schedules are paused until resumed".into();
        Ok(())
    }

    // Called under the same controller lock after validation and durable save.
    pub(crate) fn assign_roles(&mut self, roles: [RelayRole; 4]) {
        self.roles = roles;
    }

    pub(crate) fn configure_overflow(&mut self, overflow: OverflowConfig) {
        self.overflow = overflow;
    }

    fn channel_for(&self, role: RelayRole) -> Option<u8> {
        self.roles
            .iter()
            .position(|assigned| *assigned == role)
            .map(|index| index as u8 + 1)
    }

    pub fn configure_schedules(&mut self, schedules: ScheduleConfig) {
        // Finish any run started by the old water schedule before changing its
        // pump or timing, so the former target cannot outlive that schedule.
        for index in 0..4 {
            if self.roles[index].is_water_pump()
                && self.active[index].as_ref().is_some_and(|run| run.scheduled)
            {
                let _ = self.stop(index as u8 + 1);
            }
        }
        self.schedules = schedules;
        self.next_water = Instant::now()
            + Duration::from_secs(self.schedules.watering.every_minutes.clamp(1, 1440) * 60);
        self.last_schedule_check = None;
    }

    pub fn pause_schedules(&mut self) -> Result<(), String> {
        self.automation_paused = true;
        crate::atomic_write(
            &self.directory.join("automation_paused"),
            b"Paused from dashboard\n",
        )
        .map_err(|e| e.to_string())
    }

    pub fn resume_schedules(&mut self) -> Result<(), String> {
        if self.stop_pending {
            return Err("Wait for output shutdown to finish".into());
        }
        match std::fs::remove_file(self.directory.join("automation_paused")) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
        self.automation_paused = false;
        self.next_water = Instant::now()
            + Duration::from_secs(self.schedules.watering.every_minutes.clamp(1, 1440) * 60);
        self.last_schedule_check = None;
        self.last_event = "Schedules resumed; watering begins after the configured interval".into();
        Ok(())
    }

    pub fn stop_from_dashboard(&mut self, channel: Option<u8>) -> Result<(), String> {
        if channel.is_some_and(|channel| !(1..=4).contains(&channel)) {
            return Err("Output must be 1–4".into());
        }
        let pause = self.pause_schedules();
        let stop = match channel {
            Some(channel) => self.stop(channel),
            None => self.stop_all(),
        };
        stop.and(pause)
    }

    fn maximum(&self, index: usize) -> Option<u64> {
        self.roles[index].maximum_seconds(self.pump.pump_runtime_limit_seconds)
    }

    fn pump_block_reason(&self, role: RelayRole, hour: u8) -> Option<String> {
        if self.directory.join("emergency_stop").exists() {
            return Some("Emergency stop is active".into());
        }
        if let Some(reason) = super::sensors::overflow_block_reason_in(
            &self.directory.join("sensors"),
            self.overflow.sensors_for(role),
            &self.overflow.sensor_labels,
        ) {
            return Some(reason);
        }
        if role != RelayRole::Draining
            && self.pump.photo_cycle_enabled
            && !within_window(
                hour,
                self.pump.photo_cycle_start_hour,
                self.pump.photo_cycle_end_hour,
            )
        {
            return Some("Outside the configured pump operating window".into());
        }
        if let Some(pin) = self.pump.safety_gpio_pin {
            let safe = rppal::gpio::Gpio::new()
                .ok()
                .and_then(|gpio| gpio.get(pin).ok())
                .is_some_and(|pin| pin.into_input().is_high());
            if !safe {
                return Some("Safety input is LOW or unavailable".into());
            }
        }
        None
    }

    fn cooldown(&self, index: usize, now: Instant) -> u64 {
        if !matches!(self.roles[index], RelayRole::Filling | RelayRole::TankCycle) {
            return 0;
        }
        self.stopped[index]
            .map(|stopped| {
                self.pump
                    .pump_cooldown_seconds
                    .saturating_sub(now.saturating_duration_since(stopped).as_secs())
            })
            .unwrap_or(0)
    }

    pub fn start(&mut self, channel: u8, seconds: Option<u64>) -> Result<(), String> {
        if self.channel_for(RelayRole::LightsAir) == Some(channel)
            && self.schedules.lighting.enabled
            && !self.automation_paused
        {
            return Err("Pause schedules before starting a manual Lights + Air run".into());
        }
        self.start_at(channel, seconds, Instant::now(), Local::now().hour() as u8)
    }

    fn start_at(
        &mut self,
        channel: u8,
        seconds: Option<u64>,
        now: Instant,
        hour: u8,
    ) -> Result<(), String> {
        if !(1..=4).contains(&channel) {
            return Err("Output must be 1–4".into());
        }
        let index = (channel - 1) as usize;
        validate_roles(&self.roles)?;
        if self.roles[index] == RelayRole::Unassigned {
            return Err(
                "Assign a function to this relay in Setup settings before starting it".into(),
            );
        }
        let deadline = match (self.maximum(index), seconds) {
            (None, None) => None,
            (None, Some(_)) => {
                return Err("Draining uses Start/Stop; omit the duration to start draining".into());
            }
            (Some(maximum), Some(seconds)) if seconds > 0 && seconds <= maximum => {
                Some(now + Duration::from_secs(seconds))
            }
            (Some(maximum), _) => {
                return Err(format!("Run duration must be 1–{} seconds", maximum));
            }
        };
        if self.stop_pending {
            return Err("Shutdown is pending; check the relay connection".into());
        }
        if self.directory.join("emergency_stop").exists() {
            return Err("Emergency stop is active".into());
        }
        if self.roles[index].is_water_pump() {
            if let Some(reason) = self.pump_block_reason(self.roles[index], hour) {
                return Err(reason);
            }
            if self.cooldown(index, now) > 0 {
                return Err("Pump cooldown is still active".into());
            }
        }
        let states = self.board.states()?;
        if states[index] || self.active[index].is_some() {
            return Err("Output is already on; stop it before starting another run".into());
        }
        if self.roles[index].is_water_pump()
            && states
                .iter()
                .enumerate()
                .any(|(other, on)| *on && self.roles[other].is_water_pump())
        {
            return Err("Another pump is already running".into());
        }
        self.active[index] = Some(Run {
            deadline,
            scheduled: false,
        });
        if let Err(error) = self.board.set(channel, true) {
            let _ = self.stop_all();
            return Err(error);
        }
        self.last_event = match seconds {
            Some(seconds) => format!(
                "{} on relay {} started for {} seconds",
                self.roles[index].label(),
                channel,
                seconds
            ),
            None => format!(
                "Draining on relay {} started; press Stop to finish",
                channel
            ),
        };
        log::info!("Dashboard: {}", self.last_event);
        Ok(())
    }

    pub fn stop(&mut self, channel: u8) -> Result<(), String> {
        if !(1..=4).contains(&channel) {
            return Err("Output must be 1–4".into());
        }
        if let Err(error) = self.board.set(channel, false) {
            let _ = self.stop_all();
            return Err(error);
        }
        self.active[(channel - 1) as usize] = None;
        self.stopped[(channel - 1) as usize] = Some(Instant::now());
        self.last_event = format!("Output {} stopped", channel);
        Ok(())
    }

    pub fn stop_all(&mut self) -> Result<(), String> {
        for (index, active) in self.active.iter_mut().enumerate() {
            *active = None;
            self.stopped[index] = Some(Instant::now());
        }
        self.stop_pending = true;
        match self.board.all_off() {
            Ok(()) => {
                self.stop_pending = false;
                self.last_event = "All outputs stopped".into();
                Ok(())
            }
            Err(error) => {
                self.last_event = format!("Shutdown retrying: {}", error);
                Err(error)
            }
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let local = Local::now();
        self.tick_at(now, local.hour() as u8);
        if self
            .last_schedule_check
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
        {
            self.last_schedule_check = Some(now);
            self.schedule_tick(now, (local.hour() * 60 + local.minute()) as u16);
        }
    }

    fn schedule_tick(&mut self, now: Instant, minute: u16) {
        if self.automation_paused
            || self.stop_pending
            || self.directory.join("emergency_stop").exists()
            || validate_roles(&self.roles).is_err()
        {
            return;
        }
        let lighting = &self.schedules.lighting;
        let wanted =
            lighting.enabled && in_schedule(minute, lighting.start_minute, lighting.end_minute);
        if let Some(channel) = self.channel_for(RelayRole::LightsAir) {
            let index = (channel - 1) as usize;
            if wanted && self.active[index].is_none() {
                match self.board.set(channel, true) {
                    Ok(()) => {
                        self.active[index] = Some(Run {
                            deadline: Some(now + Duration::from_secs(86400)),
                            scheduled: true,
                        });
                        self.last_event =
                            format!("Lighting schedule switched relay {} on", channel);
                    }
                    Err(error) => {
                        self.last_event = format!("Lighting schedule: {}", error);
                        let _ = self.stop_all();
                        return;
                    }
                }
            } else if !wanted && self.active[index].as_ref().is_some_and(|run| run.scheduled) {
                let _ = self.stop(channel);
            }
        }
        if self.stop_pending {
            return;
        }
        let water_role = self.schedules.watering.pump.role();
        let Some(water_channel) = self.channel_for(water_role) else {
            return;
        };
        let water_index = (water_channel - 1) as usize;
        let watering = &self.schedules.watering;
        if self.active[water_index]
            .as_ref()
            .is_some_and(|run| run.scheduled)
            && (!watering.enabled
                || !in_schedule(minute, watering.start_minute, watering.end_minute))
        {
            let _ = self.stop(water_channel);
        }
        if self.stop_pending {
            return;
        }
        let watering = &self.schedules.watering;
        if watering.enabled
            && now >= self.next_water
            && in_schedule(minute, watering.start_minute, watering.end_minute)
        {
            let seconds = watering.run_seconds;
            let interval = watering.every_minutes;
            match self.start_at(water_channel, Some(seconds), now, (minute / 60) as u8) {
                Ok(()) => {
                    if let Some(run) = self.active[water_index].as_mut() {
                        run.scheduled = true;
                    }
                    self.next_water = now + Duration::from_secs(interval * 60);
                    self.last_event = format!(
                        "Water schedule started {} on relay {} for {} seconds",
                        water_role.label(),
                        water_channel,
                        seconds
                    );
                }
                Err(error) => {
                    self.next_water = now + Duration::from_secs(30);
                    self.last_event = format!("Watering waiting: {}", error);
                }
            }
        }
    }

    fn tick_at(&mut self, now: Instant, hour: u8) {
        if self.stop_pending {
            let _ = self.stop_all();
            return;
        }
        if !self.active.iter().any(Option::is_some) {
            return;
        }
        let emergency = self.directory.join("emergency_stop").exists();
        for index in 0..4 {
            let blocked = self.roles[index]
                .is_water_pump()
                .then(|| self.pump_block_reason(self.roles[index], hour))
                .flatten();
            if self.active[index].as_ref().is_some_and(|run| {
                run.deadline.is_some_and(|deadline| deadline <= now)
                    || emergency
                    || (run.scheduled && self.automation_paused)
                    || (self.roles[index].is_water_pump() && blocked.is_some())
            }) {
                let reason = if self.automation_paused
                    && self.active[index].as_ref().is_some_and(|run| run.scheduled)
                {
                    "Schedules paused"
                } else if emergency {
                    "Emergency stop"
                } else if self.roles[index].is_water_pump() {
                    blocked.as_deref().unwrap_or("Run timer completed")
                } else {
                    "Run timer completed"
                };
                if self.stop((index + 1) as u8).is_ok() {
                    self.stopped[index] = Some(now);
                    self.last_event = format!("Output {} stopped: {}", index + 1, reason);
                    log::info!("Dashboard: {}", self.last_event);
                }
            }
        }
    }

    pub fn status(&mut self) -> ControllerStatus {
        let now = Instant::now();
        let states = self.board.states();
        let hour = Local::now().hour() as u8;
        let pump_blocks: [Option<String>; 4] = std::array::from_fn(|index| {
            self.roles[index]
                .is_water_pump()
                .then(|| self.pump_block_reason(self.roles[index], hour))
                .flatten()
        });
        let blocked_pumps: Vec<_> = pump_blocks
            .iter()
            .enumerate()
            .filter_map(|(index, reason)| {
                reason
                    .as_ref()
                    .map(|reason| format!("{}: {}", self.roles[index].label(), reason))
            })
            .collect();
        let pump_block_reason = (!blocked_pumps.is_empty()).then(|| blocked_pumps.join("; "));
        let configuration_error = validate_roles(&self.roles).err();
        let global_block = configuration_error.clone().or_else(|| {
            if self.stop_pending {
                Some("Shutdown is pending; check the relay connection".into())
            } else if self.directory.join("emergency_stop").exists() {
                Some("Emergency stop is active".into())
            } else {
                None
            }
        });
        let outputs = (0..4)
            .map(|index| OutputStatus {
                channel: (index + 1) as u8,
                role: self.roles[index],
                on: states.as_ref().ok().map(|states| states[index]),
                remaining_seconds: self.active[index]
                    .as_ref()
                    .and_then(|run| run.deadline)
                    .map(|deadline| deadline.saturating_duration_since(now).as_secs()),
                cooldown_seconds: self.cooldown(index, now),
                maximum_seconds: self.maximum(index),
                schedule_controlled: self.roles[index] == RelayRole::LightsAir
                    && self.schedules.lighting.enabled
                    && !self.automation_paused,
                blocked_reason: global_block.clone().or_else(|| {
                    if self.roles[index] == RelayRole::Unassigned {
                        Some("Assign a function to this relay in Setup settings".into())
                    } else if self.roles[index].is_water_pump() {
                        pump_blocks[index].clone().or_else(|| {
                            states.as_ref().ok().and_then(|states| {
                                states
                                    .iter()
                                    .enumerate()
                                    .any(|(other, on)| {
                                        other != index && *on && self.roles[other].is_water_pump()
                                    })
                                    .then(|| "Another water pump is running".into())
                            })
                        })
                    } else {
                        None
                    }
                }),
                overflow_sensors: self.overflow.sensors_for(self.roles[index]),
            })
            .collect();
        ControllerStatus {
            outputs,
            error: states.err().or(configuration_error),
            pump_block_reason,
            last_event: self.last_event.clone(),
            stop_pending: self.stop_pending,
            automation_paused: self.automation_paused,
            next_watering_in_seconds: (self.schedules.watering.enabled
                && !self.automation_paused
                && self
                    .channel_for(self.schedules.watering.pump.role())
                    .is_some())
            .then(|| self.next_water.saturating_duration_since(now).as_secs()),
        }
    }
}

static CONTROLLER: OnceLock<Arc<Mutex<OutputController>>> = OnceLock::new();
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Console and command API relay actions share the dashboard's wiring map,
/// deadlines, interlocks, and lock. Never bypass them with direct relay writes.
pub fn relay_command(command: &str) -> Result<String, String> {
    let controller = CONTROLLER.get().ok_or("Relay controller is not ready")?;
    let mut controller = controller.lock().unwrap_or_else(|error| error.into_inner());
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        ["relay", "status"] => {
            let status = controller.status();
            if let Some(error) = status.error {
                return Err(error);
            }
            return Ok(status
                .outputs
                .iter()
                .map(|output| {
                    format!(
                        "Relay {} ({}): {}",
                        output.channel,
                        output.role.label(),
                        match output.on {
                            Some(true) => "ON",
                            Some(false) => "OFF",
                            None => "Unavailable",
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"));
        }
        ["relay", "on", channel] | ["relay", "on", channel, _] => {
            if SHUTTING_DOWN.load(Ordering::SeqCst) {
                return Err("AOG is shutting down".into());
            }
            let channel = channel.parse::<u8>().map_err(|_| "Relay must be 1–4")?;
            let seconds = parts
                .get(3)
                .map(|seconds| seconds.parse::<u64>())
                .transpose()
                .map_err(|_| "Duration must be a whole number of seconds")?;
            controller.start(channel, seconds)?;
        }
        ["relay", "off", "all"] => controller.stop_from_dashboard(None)?,
        ["relay", "off", channel] => {
            let channel = channel.parse::<u8>().map_err(|_| "Relay must be 1–4")?;
            controller.stop_from_dashboard(Some(channel))?;
        }
        _ => {
            return Err("Use: relay status | relay on <1–4> [seconds] (omit seconds for Draining only) | relay off <1–4|all>".into())
        }
    }
    Ok(controller.last_event.clone())
}

pub fn shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    if let Some(controller) = CONTROLLER.get() {
        if let Err(error) = controller
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .stop_all()
        {
            log::error!("Output shutdown failed: {}", error);
        }
    }
}

pub fn start_watchdog(controller: Arc<Mutex<OutputController>>) {
    let _ = CONTROLLER.set(Arc::clone(&controller));
    std::thread::spawn(move || loop {
        if SHUTTING_DOWN.load(Ordering::SeqCst) {
            break;
        }
        {
            let mut controller = controller.lock().unwrap_or_else(|error| error.into_inner());
            if SHUTTING_DOWN.load(Ordering::SeqCst) {
                break;
            }
            controller.tick();
        }
        std::thread::sleep(Duration::from_millis(250));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FakeBoard {
        states: Arc<Mutex<[bool; 4]>>,
        fail_stop: Arc<Mutex<bool>>,
    }
    impl RelayBoard for FakeBoard {
        fn states(&mut self) -> Result<[bool; 4], String> {
            Ok(*self.states.lock().unwrap())
        }
        fn set(&mut self, channel: u8, on: bool) -> Result<(), String> {
            if !on && *self.fail_stop.lock().unwrap() {
                return Err("disconnected".into());
            }
            self.states.lock().unwrap()[(channel - 1) as usize] = on;
            Ok(())
        }
        fn all_off(&mut self) -> Result<(), String> {
            if *self.fail_stop.lock().unwrap() {
                return Err("disconnected".into());
            }
            *self.states.lock().unwrap() = [false; 4];
            Ok(())
        }
    }
    fn fixture() -> (
        tempfile::TempDir,
        OutputController,
        Arc<Mutex<[bool; 4]>>,
        Arc<Mutex<bool>>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sensors")).unwrap();
        for tank in ["t1_ovf", "t2_ovf"] {
            std::fs::write(dir.path().join("sensors").join(tank), "NONE").unwrap();
        }
        let board = FakeBoard::default();
        let states = Arc::clone(&board.states);
        let fail = Arc::clone(&board.fail_stop);
        let controller = OutputController::new(
            dir.path(),
            PumpConfig::default(),
            crate::DashboardConfig::default().relay_roles,
            Box::new(board),
        );
        (dir, controller, states, fail)
    }
    #[test]
    fn pump_interlocks_limits_exclusion_and_cooldown_are_enforced() {
        let (dir, mut controller, states, _) = fixture();
        let now = Instant::now();
        assert!(controller.start_at(0, Some(1), now, 12).is_err());
        assert!(controller.start_at(3, Some(301), now, 12).is_err());
        controller.start_at(3, Some(5), now, 12).unwrap();
        assert!(states.lock().unwrap()[2]);
        assert!(controller.start_at(2, None, now, 12).is_err());
        std::fs::write(dir.path().join("sensors/t1_ovf"), "OVERFLOW").unwrap();
        controller.tick_at(now, 12);
        assert!(!states.lock().unwrap()[2]);
        std::fs::write(dir.path().join("sensors/t1_ovf"), "NONE").unwrap();
        assert!(controller
            .start_at(3, Some(5), now + Duration::from_secs(1), 12)
            .is_err());
        assert!(controller
            .start_at(3, Some(5), now + Duration::from_secs(61), 12)
            .is_ok());
    }
    #[test]
    fn watchdog_stops_runs_without_a_browser_and_retries_failed_shutdown() {
        let (_dir, mut controller, states, fail) = fixture();
        let now = Instant::now();
        controller.start_at(1, Some(5), now, 12).unwrap();
        *fail.lock().unwrap() = true;
        controller.tick_at(now + Duration::from_secs(6), 12);
        assert!(controller.stop_pending);
        assert!(controller.start_at(1, Some(5), now, 12).is_err());
        *fail.lock().unwrap() = false;
        controller.tick_at(now + Duration::from_secs(7), 12);
        assert_eq!(*states.lock().unwrap(), [false; 4]);
        assert!(!controller.stop_pending);
    }
    #[test]
    fn missing_sensor_readings_and_emergency_stop_block_starts() {
        let (dir, mut controller, _, _) = fixture();
        std::fs::remove_file(dir.path().join("sensors/t2_ovf")).unwrap();
        assert!(controller.start(3, Some(5)).is_err());
        std::fs::write(dir.path().join("emergency_stop"), "stop").unwrap();
        assert!(controller.start(1, Some(5)).is_err());
    }
    #[test]
    fn operating_window_supports_midnight_and_overnight() {
        assert!(within_window(23, 6, 24));
        assert!(!within_window(5, 6, 24));
        assert!(within_window(1, 22, 6));
        assert!(within_window(23, 22, 6));
        assert!(!within_window(12, 22, 6));
    }

    #[test]
    fn lighting_follows_overnight_window_and_stop_pauses_automation() {
        let (dir, mut controller, states, _) = fixture();
        let now = Instant::now();
        controller.schedules.lighting = crate::LightingSchedule {
            enabled: true,
            start_minute: 22 * 60,
            end_minute: 6 * 60,
        };
        controller.schedule_tick(now, 23 * 60);
        assert!(states.lock().unwrap()[0]);
        controller.schedule_tick(now, 3 * 60);
        assert!(states.lock().unwrap()[0]);
        controller.schedule_tick(now, 6 * 60);
        assert!(!states.lock().unwrap()[0]);
        controller.schedule_tick(now, 23 * 60);
        controller.stop_from_dashboard(None).unwrap();
        controller.schedule_tick(now, 23 * 60);
        assert_eq!(*states.lock().unwrap(), [false; 4]);
        assert!(dir.path().join("automation_paused").exists());
        controller.resume_schedules().unwrap();
        controller.schedule_tick(now, 23 * 60);
        assert!(states.lock().unwrap()[0]);
    }

    #[test]
    fn scheduled_watering_waits_for_interval_and_safety_then_stops_on_deadline() {
        let (dir, mut controller, states, _) = fixture();
        let now = Instant::now();
        let mut schedules = ScheduleConfig::default();
        schedules.watering.enabled = true;
        schedules.watering.every_minutes = 5;
        schedules.watering.run_seconds = 10;
        controller.configure_schedules(schedules);
        controller.next_water = now + Duration::from_secs(300);
        controller.schedule_tick(now, 12 * 60);
        assert_eq!(*states.lock().unwrap(), [false; 4]);
        std::fs::write(dir.path().join("sensors/t2_ovf"), "OVERFLOW").unwrap();
        controller.schedule_tick(now + Duration::from_secs(300), 12 * 60);
        assert_eq!(*states.lock().unwrap(), [false; 4]);
        std::fs::write(dir.path().join("sensors/t2_ovf"), "NONE").unwrap();
        controller.schedule_tick(now + Duration::from_secs(330), 12 * 60);
        assert!(states.lock().unwrap()[2]);
        controller.tick_at(now + Duration::from_secs(341), 12);
        assert!(!states.lock().unwrap()[2]);
        controller.schedule_tick(now + Duration::from_secs(342), 12 * 60);
        assert!(!states.lock().unwrap()[2]);
    }

    #[test]
    fn schedule_validation_rejects_identical_times_and_unsafe_cycles() {
        let mut config = Config::new();
        let mut schedules = ScheduleConfig::default();
        schedules.lighting.end_minute = schedules.lighting.start_minute;
        let patch = SettingsPatch {
            setup: None,
            pump: None,
            ph: None,
            schedules: Some(schedules),
        };
        assert!(patch.apply(&mut config).is_err());
        let mut schedules = ScheduleConfig::default();
        schedules.watering.every_minutes = 1;
        schedules.watering.run_seconds = 60;
        schedules.watering.enabled = true;
        let patch = SettingsPatch {
            setup: None,
            pump: None,
            ph: None,
            schedules: Some(schedules),
        };
        assert!(patch.apply(&mut config).is_err());
    }

    #[test]
    fn stopping_one_output_also_stops_paused_scheduled_runs_and_pause_survives_restart() {
        let (dir, mut controller, states, _) = fixture();
        let now = Instant::now();
        controller.schedules.lighting.enabled = true;
        controller.schedule_tick(now, 12 * 60);
        controller.start_at(3, Some(10), now, 12).unwrap();
        controller.stop_from_dashboard(Some(3)).unwrap();
        controller.tick_at(now, 12);
        assert_eq!(*states.lock().unwrap(), [false; 4]);
        let mut restarted = OutputController::new(
            dir.path(),
            PumpConfig::default(),
            crate::DashboardConfig::default().relay_roles,
            Box::new(FakeBoard::default()),
        );
        restarted.configure_schedules(controller.schedules.clone());
        assert!(restarted.status().automation_paused);
        restarted.schedule_tick(now, 12 * 60);
        assert!(!restarted.is_running());
    }

    #[test]
    fn scheduled_watering_stops_when_the_window_closes_or_schedule_is_disabled() {
        for disable in [true, false] {
            let (_dir, mut controller, states, _) = fixture();
            let now = Instant::now();
            controller.schedules.watering.enabled = true;
            controller.next_water = now;
            controller.schedule_tick(now, 12 * 60);
            assert!(states.lock().unwrap()[2]);
            if disable {
                controller.schedules.watering.enabled = false;
            }
            controller.schedule_tick(now, if disable { 12 * 60 } else { 23 * 60 });
            assert!(!states.lock().unwrap()[2]);
        }
    }
}
