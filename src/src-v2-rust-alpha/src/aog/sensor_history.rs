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

// The order is also the order of values in the on-disk v1 records.
const METRICS: [Metric; 6] = [
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
];

/// Current readings for the LCD use the same validity and freshness rules as
/// the dashboard and exports, without loading or changing recorded history.
pub(super) fn current_readings(directory: &Path) -> [(&'static str, Option<f64>); 6] {
    std::array::from_fn(|index| {
        let metric = &METRICS[index];
        (metric.key, read_latest(directory, metric).map(|reading| reading.0))
    })
}

fn read_latest(directory: &Path, metric: &Metric) -> Option<(f64, i64)> {
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
    pub values: [Option<f64>; 6],
}

struct State {
    samples: VecDeque<Sample>,
    error: Option<String>,
}

pub struct SensorHistory {
    directory: PathBuf,
    state: Mutex<State>,
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
    pub values: [Option<f64>; 6],
    pub observed_at: [Option<i64>; 6],
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
        let logs = directory.join("sensor_history/v1");
        let mut samples = Vec::new();
        let mut error = None;
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
                                for line in BufReader::new(file).lines() {
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
                                    // A partial final record after a power loss must not
                                    // prevent loading the remaining valid history.
                                    let Ok(mut sample) = serde_json::from_str::<Sample>(&line)
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
        samples.sort_by_key(|sample| sample.timestamp);
        samples.dedup_by_key(|sample| sample.timestamp);
        let excess = samples.len().saturating_sub(MAX_SAMPLES);
        samples.drain(..excess);
        Self {
            directory: directory.into(),
            state: Mutex::new(State {
                samples: samples.into(),
                error,
            }),
        }
    }

    fn read_value(&self, metric: &Metric) -> Option<f64> {
        self.read_latest(metric).map(|reading| reading.0)
    }

    fn read_latest(&self, metric: &Metric) -> Option<(f64, i64)> {
        read_latest(&self.directory, metric)
    }

    fn record(&self, now: DateTime<Utc>, clean_logs: bool) -> io::Result<bool> {
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
            if state
                .samples
                .back()
                .is_some_and(|last| sample.timestamp - last.timestamp < SAMPLE_SECONDS as i64)
            {
                return Ok(false);
            }
            state.samples.push_back(sample.clone());
            while state.samples.len() > MAX_SAMPLES {
                state.samples.pop_front();
            }
        }
        let logs = self.directory.join("sensor_history/v1");
        fs::create_dir_all(&logs)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs.join(format!("{}.jsonl", now.format("%Y-%m-%d"))))?;
        // Prefix a newline as well, so a torn record cannot absorb this record.
        let record = format!("\n{}\n", serde_json::to_string(&sample)?);
        file.write_all(record.as_bytes())?;
        file.sync_data()?;
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

    /// Full-resolution saved samples, never the chart's reduced point set.
    pub fn export(
        &self,
        start: i64,
        end: i64,
        include_current: bool,
        setup_name: &str,
    ) -> AirQualityExport {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let samples = state
            .samples
            .iter()
            .filter(|sample| (start..=end).contains(&sample.timestamp))
            .cloned()
            .collect();
        let error = state.error.clone();
        drop(state);
        let current = include_current.then(|| {
            let readings: [Option<(f64, i64)>; 6] =
                std::array::from_fn(|index| self.read_latest(&METRICS[index]));
            CurrentReadings {
                timestamp: Utc::now().timestamp(),
                values: std::array::from_fn(|index| readings[index].map(|reading| reading.0)),
                observed_at: std::array::from_fn(|index| readings[index].map(|reading| reading.1)),
            }
        });
        AirQualityExport {
            schema_version: 1,
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
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let samples: Vec<_> = state
            .samples
            .iter()
            .filter(|sample| (start..=now).contains(&sample.timestamp))
            .collect();
        let series = METRICS
            .iter()
            .enumerate()
            .map(|(index, metric)| {
                let latest = state
                    .samples
                    .iter()
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
                let available = state.samples.back().is_some_and(|sample| {
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
                        None
                    }
                    Ok(false) => worker
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .error
                        .clone(),
                    Err(error) => {
                        log::warn!("Cannot save sensor history: {}", error);
                        Some(format!(
                            "History is available in memory but could not be saved: {}",
                            error
                        ))
                    }
                };
                worker
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .error = error;
                std::thread::sleep(Duration::from_secs(SAMPLE_SECONDS));
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
