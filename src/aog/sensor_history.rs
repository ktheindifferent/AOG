//! Persistent environmental history, sampled independently of dashboard clients.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const SAMPLE_SECONDS: u64 = 60;
const SAVE_RETRY_SECONDS: u64 = 10;
const SAVE_BATCH_SIZE: usize = 60;
const RETENTION_HOURS: i64 = 7 * 24;
const MAX_SAMPLES: usize = RETENTION_HOURS as usize * 60 + 1;
const CHART_BUCKETS: usize = 240;

struct Metric {
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    suffix: &'static str,
    minimum: f64,
    maximum: f64,
    freshness: u64,
}

// The original six columns keep their order; schema v2 appends D6 and D8.
const METRICS: [Metric; 8] = [
    Metric {
        key: "co2",
        label: "CO₂",
        unit: "ppm",
        suffix: "ppm",
        minimum: 0.0,
        maximum: 1_000_000.0,
        freshness: 15,
    },
    Metric {
        key: "tvoc",
        label: "TVOC",
        unit: "ppb",
        suffix: "ppb",
        minimum: 0.0,
        maximum: 100_000.0,
        freshness: 15,
    },
    Metric {
        key: "pm25",
        label: "PM2.5",
        unit: "µg/m³",
        suffix: "",
        minimum: 0.0,
        maximum: 999.9,
        freshness: 120,
    },
    Metric {
        key: "pm10",
        label: "PM10",
        unit: "µg/m³",
        suffix: "",
        minimum: 0.0,
        maximum: 999.9,
        freshness: 120,
    },
    Metric {
        key: "temp",
        label: "Temperature",
        unit: "°C",
        suffix: "C",
        minimum: -80.0,
        maximum: 85.0,
        freshness: 15,
    },
    Metric {
        key: "hum",
        label: "Humidity",
        unit: "%",
        suffix: "%",
        minimum: 0.0,
        maximum: 100.0,
        freshness: 15,
    },
    Metric {key: "water_temp_d6", label: "Water · D6", unit: "°C", suffix: "C", minimum: -55.0, maximum: 125.0, freshness: 15},
    Metric {key: "water_temp_d8", label: "Water · D8", unit: "°C", suffix: "C", minimum: -55.0, maximum: 125.0, freshness: 15},
];

/// Current readings for the LCD use the same validity and freshness rules as
/// the dashboard and exports, without loading or changing recorded history.
pub(super) fn current_readings(directory: &Path) -> [(&'static str, Option<f64>); 8] {
    std::array::from_fn(|index| {
        let metric = &METRICS[index];
        (metric.key, read_latest(directory, metric).map(|reading| reading.0))
    })
}

fn read_latest(directory: &Path, metric: &Metric) -> Option<(f64, i64)> {
    if metric.key.starts_with("water_temp_") {
        return super::device_link::water_reading(directory, if metric.key.ends_with('6') {6} else {8});
    }
    // Open first so the metadata and value refer to the same atomically
    // replaced sensor file, even when a new reading arrives during sampling.
    let file = File::open(directory.join("sensors").join(metric.key)).ok()?;
    let modified = file.metadata().ok()?.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()?.as_secs() > metric.freshness {
        return None;
    }
    let mut raw = String::new();
    use std::io::Read;
    file.take(128).read_to_string(&mut raw).ok()?;
    let raw = raw.trim();
    let value = raw
        .strip_suffix(metric.suffix)
        .unwrap_or(raw)
        .trim()
        .parse::<f64>()
        .ok()?;
    let timestamp = modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    (value.is_finite() && (metric.minimum..=metric.maximum).contains(&value))
        .then_some((value, timestamp))
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Sample {
    pub timestamp: i64,
    #[serde(deserialize_with = "read_values")]
    pub values: [Option<f64>; 8],
}

// Existing installed environmental history is retained. This schema version is
// unrelated to the removed Python application; six-column records remain readable.
fn read_values<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[Option<f64>; 8], D::Error> {
    let values = Vec::<Option<f64>>::deserialize(d)?;
    if values.len() != 6 && values.len() != 8 { return Err(serde::de::Error::custom("Expected 6 or 8 sensor values")); }
    Ok(std::array::from_fn(|i| values.get(i).copied().flatten()))
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    #[test] fn preserves_existing_six_column_history() {
        let sample: Sample=serde_json::from_str(r#"{"timestamp":123,"values":[800,10,2,3,24,60]}"#).unwrap();
        assert_eq!(sample.values,[Some(800.0),Some(10.0),Some(2.0),Some(3.0),Some(24.0),Some(60.0),None,None]);
    }
    #[test] fn recovered_water_values_reach_history_and_exports_without_becoming_live() {
        let directory=tempfile::tempdir().unwrap();let now=Utc::now().timestamp();
        let history=SensorHistory::new(directory.path());
        let fields=super::super::device_link::fields("BOOT_ID: 1234567890abcdef\nSEQ: 1\nUPTIME_S: 300\nSAMPLE_UPTIME_S: 60\nTIMESTAMP: 0\nWATER_D6: 21.75C\nWATER_D8: 23.5C");
        super::super::device_link::absorb(directory.path(),"DUAL_OVF_SENSOR",&fields).unwrap();
        let exported=history.export(now-3600,now,true,"Test setup");
        assert_eq!(exported.metrics.len(),8);assert_eq!(exported.samples.len(),1);
        assert_eq!(exported.samples[0].values[6],Some(21.75));
        assert!(exported.current.unwrap().values[6].is_none());
        let snapshot=history.snapshot(1);
        assert_eq!(snapshot.series[7].count,1);assert!(!snapshot.series[7].available);
        assert_eq!(snapshot.series[7].latest.as_ref().unwrap().value,Some(23.5));
    }
}

struct State {
    samples: VecDeque<Sample>,
    pending: VecDeque<Sample>,
    load_error: Option<String>,
    error: Option<String>,
}

pub struct SensorHistory {
    directory: PathBuf,
    state: Mutex<State>,
    recording: Mutex<()>,
}

#[derive(Clone, Serialize)]
pub struct Point {
    pub timestamp: i64,
    pub value: Option<f64>,
}

#[derive(Serialize)]
pub struct Series {
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    available: bool,
    latest: Option<Point>,
    count: usize,
    minimum: Option<f64>,
    maximum: Option<f64>,
    average: Option<f64>,
    points: Vec<Point>,
}

#[derive(Serialize)]
pub struct HistoryResponse {
    server_time: i64,
    range_hours: u64,
    sample_interval_seconds: u64,
    retention_hours: i64,
    error: Option<String>,
    series: Vec<Series>,
}

#[derive(Serialize)]
pub struct ExportMetric {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
}

#[derive(Serialize)]
pub struct CurrentReadings {
    pub timestamp: i64,
    pub values: [Option<f64>; 8],
    pub observed_at: [Option<i64>; 8],
}

#[derive(Serialize)]
pub struct AirQualityExport {
    pub schema_version: u8,
    pub setup_name: String,
    pub generated_at: i64,
    pub start: i64,
    pub end: i64,
    pub sample_interval_seconds: u64,
    pub retention_hours: i64,
    pub metrics: Vec<ExportMetric>,
    pub current: Option<CurrentReadings>,
    pub samples: Vec<Sample>,
    pub error: Option<String>,
}

fn log_date(path: &Path) -> Option<NaiveDate> {
    if path.extension()?.to_str()? != "jsonl" {
        return None;
    }
    NaiveDate::parse_from_str(path.file_stem()?.to_str()?, "%Y-%m-%d").ok()
}

impl SensorHistory {
    pub fn new(directory: &Path) -> Self {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::hours(RETENTION_HOURS);
        let mut samples = Vec::new();
        let mut error = None;
        for version in ["v1", "v2"] {
        let logs = directory.join("sensor_history").join(version);
        if logs.exists() {
            match fs::read_dir(&logs) {
                Ok(entries) => {
                    for entry in entries.filter_map(Result::ok) {
                        let path = entry.path();
                        if !log_date(&path).is_some_and(|date| {
                            date >= cutoff.date_naive() && date <= now.date_naive()
                        }) {
                            continue;
                        }
                        match File::open(&path) {
                            Ok(file) => {
                                for line in BufReader::new(file).split(b'\n') {
                                    let line = match line {
                                        Ok(line) => line,
                                        Err(failure) => {
                                            error = Some(format!(
                                                "Cannot read saved sensor history: {}",
                                                failure
                                            ));
                                            break;
                                        }
                                    };
                                    // A torn record or invalid UTF-8 must not
                                    // hide valid records later in the same file.
                                    let Ok(mut sample) = serde_json::from_slice::<Sample>(&line)
                                    else {
                                        continue;
                                    };
                                    if !(cutoff.timestamp()..=now.timestamp())
                                        .contains(&sample.timestamp)
                                    {
                                        continue;
                                    }
                                    for (index, metric) in METRICS.iter().enumerate() {
                                        sample.values[index] =
                                            sample.values[index].filter(|value| {
                                                value.is_finite()
                                                    && (metric.minimum..=metric.maximum)
                                                        .contains(value)
                                            });
                                    }
                                    samples.push(sample);
                                }
                            }
                            Err(failure) => {
                                error =
                                    Some(format!("Cannot open saved sensor history: {}", failure))
                            }
                        }
                    }
                }
                Err(failure) => error = Some(format!("Cannot load sensor history: {}", failure)),
            }
        }
        }
        samples.sort_by_key(|sample| sample.timestamp);
        samples.dedup_by_key(|sample| sample.timestamp);
        let excess = samples.len().saturating_sub(MAX_SAMPLES);
        samples.drain(..excess);
        Self {
            directory: directory.into(),
            state: Mutex::new(State {
                samples: samples.into(),
                pending: VecDeque::new(),
                load_error: error.clone(),
                error,
            }),
            recording: Mutex::new(()),
        }
    }

    fn read_value(&self, metric: &Metric) -> Option<f64> {
        self.read_latest(metric).map(|reading| reading.0)
    }

    fn read_latest(&self, metric: &Metric) -> Option<(f64, i64)> {
        read_latest(&self.directory, metric)
    }

    fn record(&self, now: DateTime<Utc>, clean_logs: bool) -> io::Result<bool> {
        // Serialize recorders without holding the history snapshot lock during
        // filesystem I/O. Exports remain available during a slow or failed save.
        let _recording = self.recording.lock().unwrap_or_else(|error| error.into_inner());
        let sample = Sample {
            timestamp: now.timestamp(),
            values: std::array::from_fn(|index| self.read_value(&METRICS[index])),
        };
        let cutoff = now - chrono::Duration::hours(RETENTION_HOURS);
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            // Clock corrections and restarts must not create future-dated or
            // duplicate samples; retention is based on elapsed wall-clock time.
            state.samples.retain(|sample| {
                (cutoff.timestamp()..=now.timestamp()).contains(&sample.timestamp)
            });
            state.pending.retain(|sample| {
                (cutoff.timestamp()..=now.timestamp()).contains(&sample.timestamp)
            });
            let due = state
                .samples
                .back()
                .is_none_or(|last| sample.timestamp - last.timestamp >= SAMPLE_SECONDS as i64);
            if due {
                state.samples.push_back(sample.clone());
                state.pending.push_back(sample);
            }
            while state.samples.len() > MAX_SAMPLES {
                state.samples.pop_front();
            }
            while state.pending.len() > MAX_SAMPLES {
                state.pending.pop_front();
            }
        }
        // Retry the original timestamped samples, even when a new sample is
        // not yet due. A temporary failure must not become a permanent gap.
        if !self.flush_pending()? {
            return Ok(false);
        }
        let logs = self.directory.join("sensor_history/v2");
        if clean_logs {
            for entry in fs::read_dir(&logs)? {
                let path = entry?.path();
                if log_date(&path).is_some_and(|date| date < cutoff.date_naive()) {
                    fs::remove_file(path)?;
                }
            }
        }
        Ok(true)
    }

    fn flush_pending(&self) -> io::Result<bool> {
        let pending: Vec<_> = self.state.lock().unwrap_or_else(|error| error.into_inner())
            .pending.iter().take(SAVE_BATCH_SIZE).cloned().collect();
        if pending.is_empty() {
            return Ok(true);
        }
        let logs = self.directory.join("sensor_history/v2");
        fs::create_dir_all(&logs)?;
        let mut index = 0;
        while index < pending.len() {
            let day = DateTime::from_timestamp(pending[index].timestamp, 0)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid history timestamp"))?
                .date_naive();
            let mut records = String::new();
            let mut count = 0;
            while index + count < pending.len()
                && DateTime::from_timestamp(pending[index + count].timestamp, 0)
                    .is_some_and(|time| time.date_naive() == day)
            {
                // Prefix a newline so an old torn record cannot absorb this one.
                records.push('\n');
                records.push_str(&serde_json::to_string(&pending[index + count])?);
                records.push('\n');
                count += 1;
            }
            let mut file = OpenOptions::new().create(true).append(true)
                .open(logs.join(format!("{}.jsonl", day)))?;
            let before = file.metadata()?.len();
            let saved = file.write_all(records.as_bytes())
                .and_then(|_| file.sync_data())
                .and_then(|_| File::open(&logs)?.sync_all());
            if let Err(error) = saved {
                // A partial append must not accumulate duplicate rows on every
                // retry. Keep the queue until the full append is durable.
                if let Err(rollback) = file.set_len(before) {
                    return Err(io::Error::other(format!("{error}; cannot rewind incomplete history append: {rollback}")));
                }
                let _ = file.sync_data();
                return Err(error);
            }
            self.state.lock().unwrap_or_else(|error| error.into_inner())
                .pending.drain(..count);
            index += count;
        }
        Ok(self.state.lock().unwrap_or_else(|error| error.into_inner()).pending.is_empty())
    }

    fn combined_samples(&self) -> Vec<Sample> {
        let mut samples: Vec<_> = self.state.lock().unwrap_or_else(|e| e.into_inner()).samples.iter().cloned().collect();
        let mut buckets: std::collections::BTreeMap<i64, usize> = samples.iter().enumerate().map(|(i,s)| (s.timestamp / 60, i)).collect();
        let config = super::device_link::water_config(&self.directory);
        let mut sources = std::collections::HashMap::<(i64,usize), String>::new();
        for record in super::device_link::samples(&self.directory, Utc::now().timestamp()-RETENTION_HOURS*3600, Utc::now().timestamp()) {
            let bucket = record.timestamp / 60;
            let index = *buckets.entry(bucket).or_insert_with(|| {
                samples.push(Sample {timestamp: record.timestamp, values: [None; 8]}); samples.len()-1
            });
            for column in [0,1,4,5,6,7] {
                if column < 6 && record.device_id != "SENSORKIT_MK1" {continue;}
                if column >= 6 && config[column-6].source.as_ref().is_some_and(|s| s != &record.device_id) {continue;}
                let value = record.values[column];
                if column >= 6 && value.is_some() && config[column-6].source.is_none() {
                    let source = sources.entry((bucket,column)).or_insert_with(|| record.device_id.clone());
                    if source != &record.device_id { *source = "ambiguous".into(); samples[index].values[column] = None; continue; }
                }
                if samples[index].values[column].is_none() {samples[index].values[column] = value;}
            }
        }
        samples.sort_by_key(|s| s.timestamp);
        samples
    }

    /// Full-resolution saved samples, never the chart's reduced point set.
    pub fn export(
        &self,
        start: i64,
        end: i64,
        include_current: bool,
        setup_name: &str,
    ) -> AirQualityExport {
        let combined = self.combined_samples();
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let samples = combined.iter()
            .filter(|sample| (start..=end).contains(&sample.timestamp))
            .cloned()
            .collect();
        let error = state.error.clone();
        drop(state);
        let current = include_current.then(|| {
            let readings: [Option<(f64, i64)>; 8] =
                std::array::from_fn(|index| self.read_latest(&METRICS[index]));
            CurrentReadings {
                timestamp: Utc::now().timestamp(),
                values: std::array::from_fn(|index| readings[index].map(|reading| reading.0)),
                observed_at: std::array::from_fn(|index| readings[index].map(|reading| reading.1)),
            }
        });
        AirQualityExport {
            schema_version: 2,
            setup_name: setup_name.into(),
            generated_at: Utc::now().timestamp(),
            start,
            end,
            sample_interval_seconds: SAMPLE_SECONDS,
            retention_hours: RETENTION_HOURS,
            metrics: METRICS
                .iter()
                .map(|metric| ExportMetric {
                    key: metric.key,
                    label: metric.label,
                    unit: metric.unit,
                })
                .collect(),
            current,
            samples,
            error,
        }
    }

    pub fn snapshot(&self, hours: u64) -> HistoryResponse {
        let now = Utc::now().timestamp();
        let hours = hours.clamp(1, RETENTION_HOURS as u64);
        let start = now - hours as i64 * 3600;
        let current = current_readings(&self.directory);
        let combined = self.combined_samples();
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let samples: Vec<_> = combined.iter()
            .filter(|sample| (start..=now).contains(&sample.timestamp))
            .collect();
        let series = METRICS
            .iter()
            .enumerate()
            .map(|(index, metric)| {
                let latest = combined.iter()
                    .rev()
                    .find(|sample| sample.timestamp <= now && sample.values[index].is_some())
                    .map(|sample| Point {
                        timestamp: sample.timestamp,
                        value: sample.values[index],
                    });
                let values: Vec<_> = samples
                    .iter()
                    .filter_map(|sample| sample.values[index])
                    .collect();
                let available = current[index].1.is_some() && state.samples.back().is_some_and(|sample| {
                    sample.timestamp <= now
                        && now - sample.timestamp <= SAMPLE_SECONDS as i64 * 2
                        && sample.values[index].is_some()
                });
                let points = samples
                    .iter()
                    .map(|sample| Point {
                        timestamp: sample.timestamp,
                        value: sample.values[index],
                    })
                    .collect::<Vec<_>>();
                Series {
                    key: metric.key,
                    label: metric.label,
                    unit: metric.unit,
                    available,
                    latest,
                    count: values.len(),
                    minimum: values.iter().copied().reduce(f64::min),
                    maximum: values.iter().copied().reduce(f64::max),
                    average: (!values.is_empty())
                        .then(|| values.iter().sum::<f64>() / values.len() as f64),
                    points: chart_points(&points, start, now),
                }
            })
            .collect();
        HistoryResponse {
            server_time: now,
            range_hours: hours,
            sample_interval_seconds: SAMPLE_SECONDS,
            retention_hours: RETENTION_HOURS,
            error: state.error.clone(),
            series,
        }
    }
}

// Keep the first/last point and both extrema of each time bucket. Split on
// missing readings BEFORE reducing, so neither spikes nor gaps are hidden.
pub(super) fn chart_points(points: &[Point], start: i64, end: i64) -> Vec<Point> {
    let bucket_seconds = ((end - start) / CHART_BUCKETS as i64).max(SAMPLE_SECONDS as i64);
    let mut result = Vec::new();
    let mut index = 0;
    while index < points.len() {
        if index > 0
            && points[index].timestamp - points[index - 1].timestamp > SAMPLE_SECONDS as i64 * 2
        {
            result.push(Point {
                timestamp: points[index - 1].timestamp + SAMPLE_SECONDS as i64,
                value: None,
            });
        }
        if points[index].value.is_none() {
            result.push(points[index].clone());
            index += 1;
            while index < points.len() && points[index].value.is_none() {
                index += 1;
            }
            continue;
        }
        let first = index;
        let bucket = (points[index].timestamp - start) / bucket_seconds;
        let mut minimum = index;
        let mut maximum = index;
        index += 1;
        while index < points.len()
            && points[index].value.is_some()
            && (points[index].timestamp - start) / bucket_seconds == bucket
            && points[index].timestamp - points[index - 1].timestamp <= SAMPLE_SECONDS as i64 * 2
        {
            if points[index].value < points[minimum].value {
                minimum = index;
            }
            if points[index].value > points[maximum].value {
                maximum = index;
            }
            index += 1;
        }
        let mut selected = vec![first, minimum, maximum, index - 1];
        selected.sort_unstable();
        selected.dedup();
        result.extend(
            selected
                .into_iter()
                .map(|selected| points[selected].clone()),
        );
    }
    result
}

pub fn start_recording(history: Arc<SensorHistory>) {
    let worker = Arc::clone(&history);
    if let Err(error) = std::thread::Builder::new()
        .name("sensor_history".into())
        .spawn(move || {
            let mut cleaned_date = None;
            loop {
                let now = Utc::now();
                let error = match worker.record(now, cleaned_date != Some(now.date_naive())) {
                    Ok(true) => {
                        cleaned_date = Some(now.date_naive());
                        worker.state.lock().unwrap_or_else(|error| error.into_inner()).load_error.clone()
                    }
                    Ok(false) => worker
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .error
                        .clone(),
                    Err(error) => {
                        let state = worker.state.lock().unwrap_or_else(|error| error.into_inner());
                        let message = if state.pending.is_empty() {
                            format!("History storage maintenance will retry: {}", error)
                        } else {
                            format!("History is available in memory; saving will retry: {}", error)
                        };
                        let message = match &state.load_error {
                            Some(previous) => format!("{}; {}", previous, message),
                            None => message,
                        };
                        if state.error.as_ref() != Some(&message) {
                            log::warn!("{}", message);
                        }
                        Some(message)
                    }
                };
                worker
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .error = error;
                std::thread::sleep(Duration::from_secs(SAVE_RETRY_SECONDS));
            }
        })
    {
        history
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .error = Some(format!(
            "Sensor history recording could not start: {}",
            error
        ));
        log::error!("Cannot start sensor history recorder: {}", error);
    }
}
