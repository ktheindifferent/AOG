// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::path::Path;

use rouille::post_input;
use rouille::session;
use rouille::try_or_400;
use rouille::{Request, Response};

use std::sync::{Mutex, OnceLock};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::aog;
use crate::Config;

// Add Debug Flag and use ./www/ instead of installed dir

pub fn init() {
    let config = match Config::load(0) {
        Ok(config) => config,
        Err(error) => {
            log::error!("Failed to load config: {}", error);
            return;
        }
    };

    let cert = match std::fs::read("/opt/aog/crt/default/aog.local.cert") {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to read certificate: {}", e);
            return;
        }
    };

    let pkey = match std::fs::read("/opt/aog/crt/default/aog.local.key") {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to read private key: {}", e);
            return;
        }
    };

    let bind_address = config
        .https_bind_address
        .clone()
        .unwrap_or_else(|| crate::DEFAULT_BIND_ADDRESS.to_string());
    let bind_port = config.https_bind_port.unwrap_or(8443);
    log::info!("Starting HTTPS server on {}:{}", bind_address, bind_port);
    let state = Arc::new(WebState::new(
        config,
        Path::new("/opt/aog"),
        aog::ph_sensor::active_sensor(),
        Box::new(aog::web_control::QuadBoard),
    ));
    let _ = RUNTIME_WEB_STATE.set(Arc::clone(&state));
    aog::web_control::start_watchdog(Arc::clone(&state.outputs));
    aog::sensor_history::start_recording(Arc::clone(&state.history));

    rouille::Server::new_ssl(
        (bind_address.as_str(), bind_port),
        move |request| handle_web_request(request, &state, Path::new("/opt/aog/www")),
        cert,
        pkey,
    )
    .map_err(|e| log::error!("Failed to start HTTPS server: {}", e))
    .ok()
    .map(|server| server.run());
}

const SESSION_LIFETIME: Duration = Duration::from_secs(3600);
struct WebSession {
    created: Instant,
    csrf: String,
}
type WebSessions = Arc<Mutex<HashMap<String, WebSession>>>;

struct WebState {
    config: Mutex<Config>,
    sessions: WebSessions,
    authentication_limits: Mutex<RateLimiter>,
    password_work: Mutex<()>,
    directory: std::path::PathBuf,
    ph: Arc<aog::ph_sensor::PhSensor>,
    history: Arc<aog::sensor_history::SensorHistory>,
    outputs: Arc<Mutex<aog::web_control::OutputController>>,
}

static RUNTIME_WEB_STATE: OnceLock<Arc<WebState>> = OnceLock::new();

pub(super) fn current_password_hash() -> Result<String, Box<dyn std::error::Error>> {
    if let Some(state) = RUNTIME_WEB_STATE.get() {
        Ok(state.config.lock().unwrap_or_else(|error| error.into_inner()).encrypted_password.clone())
    } else {
        Ok(Config::load(0)?.encrypted_password)
    }
}

/// Console credential changes and dashboard saves must share the same copy of
/// configuration, or a later settings save can silently undo a token change.
pub(super) fn update_credentials(
    update: impl FnOnce(&mut Config) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(state) = RUNTIME_WEB_STATE.get() {
        let mut config = state.config.lock().unwrap_or_else(|error| error.into_inner());
        let mut updated = config.clone();
        update(&mut updated)?;
        updated.save_in(&state.directory)?;
        let password_changed = updated.encrypted_password != config.encrypted_password;
        *config = updated;
        if password_changed {
            state.sessions.lock().unwrap_or_else(|error| error.into_inner()).clear();
        }
        Ok(())
    } else {
        // Standalone maintenance utilities still operate without starting the
        // server or touching hardware. Their existing restart requirement applies.
        let mut config = Config::load(0)?;
        update(&mut config)?;
        config.save()
    }
}

impl WebState {
    fn new(
        config: Config,
        directory: &Path,
        ph: Arc<aog::ph_sensor::PhSensor>,
        board: Box<dyn aog::web_control::RelayBoard>,
    ) -> Self {
        ph.configure(config.ph_config.clone().unwrap_or_default());
        let mut outputs = aog::web_control::OutputController::new(
            directory,
            config.pump_config.clone().unwrap_or_default(),
            config.dashboard_config.relay_roles,
            board,
        );
        outputs.configure_overflow(config.dashboard_config.overflow.clone());
        outputs.configure_schedules(config.schedules.clone());
        Self {
            config: Mutex::new(config),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            authentication_limits: Mutex::new(RateLimiter::new(10, 60)),
            password_work: Mutex::new(()),
            directory: directory.into(),
            ph,
            history: Arc::new(aog::sensor_history::SensorHistory::new(directory)),
            outputs: Arc::new(Mutex::new(outputs)),
        }
    }
}

fn request_session_id(request: &Request) -> Option<&str> {
    rouille::input::cookies(request).find_map(|(name, value)| (name == "SID").then_some(value))
}

fn authentication_limit(request: &Request, state: &WebState) -> Option<Response> {
    if state.authentication_limits.lock().unwrap_or_else(|error| error.into_inner())
        .check_rate_limit(request.remote_addr().ip())
    {
        None
    } else {
        Some(api_error(429, "Too many authentication attempts. Try again in one minute.")
            .with_additional_header("Retry-After", "60"))
    }
}

fn password_work(state: &WebState) -> Result<std::sync::MutexGuard<'_, ()>, Response> {
    match state.password_work.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::Poisoned(error)) => Ok(error.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => Err(
            api_error(429, "Another password request is in progress. Try again shortly.")
                .with_additional_header("Retry-After", "2"),
        ),
    }
}

fn authenticate(request: &Request, state: &WebState) -> Response {
    if request.method() != "POST" {
        return api_error(405, "Use POST to sign in").with_additional_header("Allow", "POST");
    }
    if let Some(response) = authentication_limit(request, state) {
        return response;
    }
    let input = try_or_400!(post_input!(request, {
        input_username: String,
        input_password: String,
    }));
    if input.input_username != "admin" {
        return api_error(401, "Invalid username or password");
    }
    let _work = match password_work(state) {
        Ok(guard) => guard,
        Err(response) => return response,
    };
    let expected_hash = state.config.lock().unwrap_or_else(|error| error.into_inner())
        .encrypted_password.clone();
    // Expensive password work must not hold the configuration/session locks
    // used by output requests, status polling, and camera authorization.
    let valid = match aog::auth::verify_password(&input.input_password, &expected_hash) {
        Ok(valid) => valid,
        Err(error) => {
            log::error!("Password verification error: {}", error);
            false
        }
    };
    if !valid {
        return api_error(401, "Invalid username or password");
    }
    let config = state.config.lock().unwrap_or_else(|error| error.into_inner());
    if config.encrypted_password != expected_hash {
        return api_error(409, "The password changed during sign-in. Try your current password again.");
    }
    // Keep config -> sessions lock order so a concurrent password reset cannot
    // create a session using the credential it just revoked.
    let mut sessions = state.sessions.lock().unwrap_or_else(|error| error.into_inner());
    sessions.retain(|_, session| session.created.elapsed() < SESSION_LIFETIME);
    if let Some(id) = request_session_id(request) {
        sessions.remove(id);
    }
    let id = session::generate_session_id();
    sessions.insert(id.clone(), WebSession {
        created: Instant::now(),
        csrf: session::generate_session_id(),
    });
    state.authentication_limits.lock().unwrap_or_else(|error| error.into_inner())
        .requests.remove(&request.remote_addr().ip());
    Response::redirect_302("/index.html")
        .with_additional_header("Set-Cookie", format!(
            "SID={}; Max-Age=3600; Path=/; HttpOnly; Secure; SameSite=Lax", id,
        ))
        .with_no_cache()
}

fn password_session(request: &Request, state: &WebState) -> Result<(), Response> {
    let sessions = state.sessions.lock().unwrap_or_else(|error| error.into_inner());
    let session = request_session_id(request).and_then(|id| sessions.get(id))
        .filter(|session| session.created.elapsed() < SESSION_LIFETIME)
        .ok_or_else(|| api_error(401, "Your session expired. Sign in again."))?;
    if request.method() != "GET" && request.header("X-AOG-CSRF") != Some(session.csrf.as_str()) {
        return Err(api_error(403, "Invalid request token. Reload the page and try again."));
    }
    Ok(())
}

fn change_account_password(request: &Request, state: &WebState, body: &Option<serde_json::Value>) -> Response {
    if let Err(response) = password_session(request, state) {
        return response;
    }
    if request.method() != "POST" {
        return api_error(405, "Use POST to change the password").with_additional_header("Allow", "POST");
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PasswordChange {
        current_password: String,
        new_password: String,
        confirm_password: String,
    }
    let input: PasswordChange = match json_input(body) {
        Ok(input) => input,
        Err(response) => return response,
    };
    if input.new_password != input.confirm_password {
        return api_error(400, "New passwords do not match");
    }
    if let Err(error) = aog::auth::validate_password_strength(&input.new_password) {
        return api_error(400, error);
    }
    if let Some(response) = authentication_limit(request, state) {
        return response;
    }
    let _work = match password_work(state) {
        Ok(guard) => guard,
        Err(response) => return response,
    };
    let expected_hash = state.config.lock().unwrap_or_else(|error| error.into_inner())
        .encrypted_password.clone();
    if !aog::auth::verify_password(&input.current_password, &expected_hash).unwrap_or(false) {
        return api_error(403, "Current password is incorrect");
    }
    let hash = match aog::auth::hash_password(&input.new_password) {
        Ok(hash) => hash,
        Err(_) => return api_error(500, "Could not secure the new password"),
    };
    let mut config = state.config.lock().unwrap_or_else(|error| error.into_inner());
    if config.encrypted_password != expected_hash {
        return api_error(409, "The password changed during this request. Try again with the current password.");
    }
    if let Err(response) = password_session(request, state) {
        return response;
    }
    // Copy the current config only after hashing, preserving any setup saves
    // or token rotation that completed while the password was being checked.
    let mut updated = config.clone();
    updated.encrypted_password = hash;
    if let Err(error) = updated.save_in(&state.directory) {
        return api_error(500, format!("Could not save password: {}", error));
    }
    *config = updated;
    state.sessions.lock().unwrap_or_else(|error| error.into_inner()).clear();
    state.authentication_limits.lock().unwrap_or_else(|error| error.into_inner())
        .requests.remove(&request.remote_addr().ip());
    Response::json(&serde_json::json!({"message":"Password changed. Sign in with your new password."}))
        .with_no_cache()
        .with_additional_header("Set-Cookie", "SID=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax")
}

fn handle_web_request(request: &Request, state: &WebState, www_root: &Path) -> Response {
    // Read untrusted network bodies before taking application locks. Otherwise
    // a client that stalls mid-upload can block every dashboard request and,
    // for output actions, the watchdog's timers and overflow checks.
    if request.url() == "/authenticate" {
        return authenticate(request, state);
    }
    let body = if request.method() == "POST" && matches!(request.url().as_str(),
        "/api/settings" | "/api/account/password" | "/api/ph/calibrate"
            | "/api/ph/calibration/remove" | "/api/outputs/start" | "/api/outputs/stop"
    ) {
        match json_body::<serde_json::Value>(request) {
            Ok(body) => Some(body),
            Err(response) => return response,
        }
    } else {
        None
    };
    if request.url() == "/api/account/password" {
        return change_account_password(request, state, &body);
    }
    let mut config = state
        .config
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let session_id = request_session_id(request);
    let mut sessions = state
        .sessions
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let now = Instant::now();
    sessions.retain(|_, session| now.duration_since(session.created) < SESSION_LIFETIME);

    if request.url() == "/logout" {
        if let Some(id) = session_id {
            sessions.remove(id);
        }
        return Response::redirect_302("/login.html")
            .with_additional_header(
                "Set-Cookie",
                "SID=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax",
            )
            .with_no_cache();
    }
    let csrf = session_id
        .and_then(|id| sessions.get(id))
        .map(|session| session.csrf.clone());
    let session_authenticated = csrf.is_some();
    drop(sessions);

    if request.url() == "/api/session" {
        return match csrf {
            Some(csrf) => {
                Response::json(&serde_json::json!({"username":"admin", "csrf_token":csrf}))
                    .with_no_cache()
            }
            None => api_error(401, "Sign in to continue"),
        };
    }
    if request.url().starts_with("/api/cameras/") {
        if !session_authenticated {
            return api_error(401, "Your session expired. Sign in again.");
        }
        if request.method() != "GET" {
            return api_error(405, "Use GET for camera streams");
        }
        let path = request.url();
        let Some(id) = path
            .strip_prefix("/api/cameras/")
            .and_then(|rest| rest.strip_suffix("/stream"))
        else {
            return api_error(404, "Unknown camera endpoint");
        };
        let sessions = Arc::clone(&state.sessions);
        let session_id = session_id.unwrap().to_owned();
        // Streaming must never hold the configuration lock used by relay controls.
        drop(config);
        let authorized = Box::new(move || {
            sessions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&session_id)
                .is_some_and(|session| session.created.elapsed() < SESSION_LIFETIME)
        });
        return match aog::video::stream(id, authorized) {
            Ok(stream) => {
                let mut response = Response::empty_204()
                    .with_status_code(200)
                    .with_additional_header(
                        "Content-Type",
                        "multipart/x-mixed-replace; boundary=aog-frame",
                    )
                    .with_additional_header("X-Content-Type-Options", "nosniff")
                    .with_additional_header("X-Accel-Buffering", "no")
                    .with_no_cache();
                response.data = rouille::ResponseBody::from_reader(stream);
                response
            }
            Err(error) => api_error(503, error),
        };
    }
    if matches!(
        request.url().as_str(),
        "/api/air-quality/export" | "/api/air-quality/report"
    ) {
        if !session_authenticated {
            return api_error(401, "Your session expired. Sign in again.");
        }
        if request.method() != "GET" {
            return api_error(405, "Use GET for exports and reports");
        }
        let setup_name = config.dashboard_config.setup_name.clone();
        drop(config);
        return aog::air_quality::handle(request, &state.history, &setup_name);
    }
    if matches!(
        request.url().as_str(),
        "/api/settings"
            | "/api/cameras"
            | "/api/ph"
            | "/api/history"
            | "/api/ph/calibrate"
            | "/api/ph/calibration/remove"
            | "/api/outputs"
            | "/api/outputs/start"
            | "/api/outputs/stop"
            | "/api/outputs/stop-all"
            | "/api/schedules/pause"
            | "/api/schedules/resume"
            | "/api/account/password"
    ) {
        if !session_authenticated {
            return api_error(401, "Your session expired. Sign in again.");
        }
        if request.method() != "GET" && request.header("X-AOG-CSRF") != csrf.as_deref() {
            return api_error(403, "Invalid request token. Reload the page and try again.");
        }
        return handle_dashboard_api(request, state, &mut config, body);
    }

    if request.url().contains("/api/dat/") {
        if !session_authenticated {
            return api_error(401, "Sign in to view camera images");
        }
        if let Some(request) = request.remove_prefix("/api/dat/") {
            return rouille::match_assets(&request, "/opt/aog/dat")
                .with_additional_header("Access-Control-Allow-Origin", "*")
                .with_no_cache();
        } else {
            return Response::text("err".to_string())
                .with_additional_header("Access-Control-Allow-Origin", "*");
        }
    }

    if matches!(request.url().as_str(), "/api/stats" | "/api/alerts/overflow" | "/api/errors") {
        if !session_authenticated {
            return api_error(401, "Sign in to view device status");
        }
        if request.method() != "GET" {
            return api_error(405, "Use GET for device status");
        }
    }

    // New API endpoint for overflow alerts
    if request.url() == "/api/alerts/overflow" {
        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct OverflowAlert {
            tank1_overflow: bool,
            tank2_overflow: bool,
            sensor_error: bool,
            error_message: String,
            timestamp: u64,
            critical: bool,
        }

        let sensors = state.directory.join("sensors");
        let readings = aog::sensors::overflow_readings_in(&sensors);
        let sensor_error = readings.contains(&aog::sensors::OverflowReading::Unavailable);

        let error_message = if sensor_error {
            match std::fs::read_to_string(sensors.join("overflow_error")) {
                Ok(msg) => msg,
                Err(_) => "Overflow reading missing, stale, or invalid".to_string(),
            }
        } else {
            "".to_string()
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let tank1_overflow = readings[0] == aog::sensors::OverflowReading::Overflow;
        let tank2_overflow = readings[1] == aog::sensors::OverflowReading::Overflow;
        let critical = tank1_overflow || tank2_overflow || sensor_error;

        let response = Response::json(&OverflowAlert {
            tank1_overflow,
            tank2_overflow,
            sensor_error,
            error_message,
            timestamp,
            critical,
        });

        return response.with_no_cache();
    }

    // Error monitoring dashboard endpoint
    if request.url() == "/api/errors" {
        let dashboard = crate::aog::error_monitor::GLOBAL_ERROR_MONITOR.get_dashboard();
        let response = Response::json(&dashboard);
        return response.with_no_cache();
    }

    if request.url() == "/api/stats" {
        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct WebApiStats {
            pm25: String,
            pm10: String,
            co2: String,
            tvoc: String,
            temp: String,
            hum: String,
            t1_ovf: String,
            t2_ovf: String,
            overflow_error: bool,
            ph: String,
            ph_status: Option<crate::aog::ph_sensor::PhSensorStatus>,
            device_time: String,
        }

        // Live history exists before calibration; never hide its status behind
        // the presence of a calibration file.
        let ph_status = state.ph.get_status();
        let ph = ph_status.current_ph.map(|value| format!("{:.2}", value)).unwrap_or_else(|| "N/A".into());
        let readings = aog::sensor_history::current_readings(&state.directory);
        let value = |key, suffix| {
            readings.iter().find(|reading| reading.0 == key).and_then(|reading| reading.1)
                .map(|value| format!("{}{}", value, suffix)).unwrap_or_else(|| "N/A".into())
        };
        let overflow = aog::sensors::overflow_readings_in(&state.directory.join("sensors"));
        let overflow_error = overflow.contains(&aog::sensors::OverflowReading::Unavailable);
        let response = Response::json(&WebApiStats {
            co2: value("co2", "ppm"),
            tvoc: value("tvoc", "ppb"),
            temp: value("temp", "C"),
            hum: value("hum", "%"),
            pm25: value("pm25", ""),
            pm10: value("pm10", ""),
            t1_ovf: overflow[0].as_str().into(),
            t2_ovf: overflow[1].as_str().into(),
            overflow_error: overflow_error,
            ph,
            ph_status: Some(ph_status),
            device_time: chrono::Local::now().to_rfc3339(),
        });
        return response.with_no_cache();
    }

    // catchall regardless of auth status
    let url = request.url();
    let public_asset = Path::new(&url)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext,
                "css" | "js" | "png" | "jpg" | "jpeg" | "svg" | "ico" | "ttf" | "woff" | "woff2"
            )
        });
    if url == "/login.html" || public_asset {
        let response = rouille::match_assets(request, www_root);
        if response.is_success() {
            return response
                .with_additional_header("Access-Control-Allow-Origin", "*")
                .with_no_cache();
        } else {
            return Response::html("404 error")
                .with_status_code(404)
                .with_additional_header("Access-Control-Allow-Origin", "*");
        }
    }

    if session_authenticated {
        if request.url() == "/" {
            return Response::redirect_302("/index.html");
        }
        let response = rouille::match_assets(request, www_root);
        if response.is_success() {
            response
                .with_additional_header("Access-Control-Allow-Origin", "*")
                .with_no_cache()
        } else {
            Response::html("404 error")
                .with_status_code(404)
                .with_additional_header("Access-Control-Allow-Origin", "*")
        }
    } else {
        //unathuenticated

        Response::redirect_302("/login.html")
    }
}

fn api_error(status: u16, message: impl std::fmt::Display) -> Response {
    Response::json(&serde_json::json!({"error": message.to_string()}))
        .with_status_code(status)
        .with_no_cache()
}

fn json_body<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, Response> {
    use std::io::Read;
    if request
        .header("Content-Type")
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err(api_error(415, "Send JSON data"));
    }
    let data = request
        .data()
        .ok_or_else(|| api_error(400, "Missing request body"))?;
    let mut bytes = Vec::new();
    data.take(16_385)
        .read_to_end(&mut bytes)
        .map_err(|_| api_error(400, "Cannot read request"))?;
    if bytes.len() > 16_384 {
        return Err(api_error(413, "Request is too large"));
    }
    serde_json::from_slice(&bytes).map_err(|_| api_error(400, "Invalid or incomplete JSON data"))
}

fn json_input<T: serde::de::DeserializeOwned>(body: &Option<serde_json::Value>) -> Result<T, Response> {
    let value = body.as_ref().ok_or_else(|| api_error(400, "Missing request body"))?;
    serde_json::from_value(value.clone()).map_err(|_| api_error(400, "Invalid or incomplete JSON data"))
}

fn settings_response(config: &Config) -> Response {
    Response::json(&serde_json::json!({
        "setup": config.dashboard_config,
        "schedules": config.schedules,
        "pump": config.pump_config.clone().unwrap_or_default(),
        "ph": config.ph_config.clone().unwrap_or_default(),
        "device_time": chrono::Local::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "network": {"dashboard_port": config.https_bind_port.unwrap_or(8443), "command_port": config.command_api_bind_port.unwrap_or(9443), "command_token_configured": config.command_api_token.as_ref().is_some_and(|token| !token.is_empty())},
    })).with_no_cache()
}

fn handle_dashboard_api(request: &Request, state: &WebState, config: &mut Config, body: Option<serde_json::Value>) -> Response {
    let path = request.url();
    if request.method() == "GET" {
        return match path.as_str() {
            "/api/cameras" => Response::json(&aog::video::status()).with_no_cache(),
            "/api/settings" => settings_response(config),
            "/api/history" => {
                let hours = request.get_param("hours").unwrap_or_else(|| "24".into());
                match hours.parse::<u64>() {
                    Ok(hours @ (1 | 6 | 24 | 168)) => {
                        Response::json(&state.history.snapshot(hours)).with_no_cache()
                    }
                    _ => api_error(400, "History range must be 1, 6, 24, or 168 hours"),
                }
            }
            "/api/ph" => Response::json(&state.ph.snapshot()).with_no_cache(),
            "/api/outputs" => {
                Response::json(&state.outputs.lock().unwrap().status()).with_no_cache()
            }
            _ => api_error(405, "Use POST for this action"),
        };
    }
    if request.method() != "POST" {
        return api_error(405, "Method not allowed");
    }
    if path == "/api/settings" {
        let patch: aog::web_control::SettingsPatch = match json_input(&body) {
            Ok(value) => value,
            Err(response) => return response,
        };
        let pump_changed = patch.pump.is_some();
        let schedules_changed = patch.schedules.is_some();
        let mut updated = config.clone();
        if let Err(error) = patch.apply(&mut updated) {
            return api_error(400, error);
        }
        let assignments_changed = updated.dashboard_config.relay_roles
            != config.dashboard_config.relay_roles
            || updated.dashboard_config.overflow != config.dashboard_config.overflow;
        let mut outputs = state.outputs.lock().unwrap();
        if pump_changed && outputs.is_running() {
            return api_error(
                409,
                "Stop active outputs before changing the external safety switch",
            );
        }
        if assignments_changed {
            if let Err(error) = outputs.prepare_reassignment() {
                return api_error(409, error);
            }
        }
        if let Err(error) = updated.save_in(&state.directory) {
            return api_error(500, format!("Could not save settings: {}", error));
        }
        if pump_changed {
            let _ = outputs.configure(updated.pump_config.clone().unwrap_or_default());
        }
        if assignments_changed {
            outputs.assign_roles(updated.dashboard_config.relay_roles);
            outputs.configure_overflow(updated.dashboard_config.overflow.clone());
        }
        if schedules_changed {
            outputs.configure_schedules(updated.schedules.clone());
        }
        state
            .ph
            .configure(updated.ph_config.clone().unwrap_or_default());
        *config = updated;
        return settings_response(config);
    }
    if path == "/api/ph/calibrate" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CalibrationInput {
            buffer_ph: f32,
            raw_value: Option<f32>,
        }
        let input: CalibrationInput = match json_input(&body) {
            Ok(value) => value,
            Err(response) => return response,
        };
        let raw_value = match input.raw_value {
            Some(raw) => raw,
            None => match state.ph.read_raw_value() {
                Ok(raw) => raw,
                Err(error) => return api_error(409, error),
            },
        };
        return match state.ph.calibrate(input.buffer_ph, raw_value) {
            Ok(()) => {
                let mut snapshot = state.ph.snapshot();
                snapshot["message"] = serde_json::json!("Calibration saved and applied. Readings and charts now use the updated calibration.");
                Response::json(&snapshot).with_no_cache()
            }
            Err(error) => api_error(400, error),
        };
    }
    if path == "/api/ph/calibration/remove" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RemovePoint {
            buffer_ph: u8,
        }
        let input: RemovePoint = match json_input(&body) {
            Ok(value) => value,
            Err(response) => return response,
        };
        return match state.ph.remove_calibration_point(input.buffer_ph) {
            Ok(()) => {
                let mut snapshot = state.ph.snapshot();
                snapshot["message"] = serde_json::json!(
                    "Calibration point removed. Readings and charts have been updated."
                );
                Response::json(&snapshot).with_no_cache()
            }
            Err(error) => api_error(400, error),
        };
    }
    let mut outputs = state.outputs.lock().unwrap();
    let result = match path.as_str() {
        "/api/outputs/start" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Start {
                channel: u8,
                seconds: Option<u64>,
            }
            let input: Start = match json_input(&body) {
                Ok(value) => value,
                Err(response) => return response,
            };
            outputs.start(input.channel, input.seconds)
        }
        "/api/outputs/stop" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Stop {
                channel: u8,
            }
            let input: Stop = match json_input(&body) {
                Ok(value) => value,
                Err(response) => return response,
            };
            outputs.stop_from_dashboard(Some(input.channel))
        }
        "/api/outputs/stop-all" | "/api/schedules/pause" => outputs.stop_from_dashboard(None),
        "/api/schedules/resume" => outputs.resume_schedules(),
        _ => return api_error(405, "This endpoint is read-only"),
    };
    match result {
        Ok(()) => Response::json(&outputs.status()).with_no_cache(),
        Err(error) => api_error(409, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn test_config() -> &'static Config {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(|| {
            let mut config = Config::new();
            config.encrypted_password = aog::auth::hash_password("TestPassword123!").unwrap();
            config
        })
    }

    fn web_request(method: &str, path: &str, cookie: Option<&str>, body: &str) -> Request {
        let mut headers = vec![(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        )];
        if let Some(cookie) = cookie {
            headers.push(("Cookie".to_owned(), cookie.to_owned()));
        }
        Request::fake_https(method, path, headers, body.as_bytes().to_vec())
    }

    fn header(response: &Response, name: &str) -> Option<String> {
        response
            .headers
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then(|| value.to_string()))
    }

    struct TestBoard;
    impl aog::web_control::RelayBoard for TestBoard {
        fn states(&mut self) -> Result<[bool; 4], String> {
            Ok([false; 4])
        }
        fn set(&mut self, _: u8, _: bool) -> Result<(), String> {
            Err("No live hardware in HTTP tests".into())
        }
        fn all_off(&mut self) -> Result<(), String> {
            Ok(())
        }
    }
    fn state(directory: &Path) -> WebState {
        std::fs::write(directory.join("index.html"), "dashboard").unwrap();
        let config = test_config().clone();
        config.save_in(directory).unwrap();
        WebState::new(
            config,
            directory,
            Arc::new(aog::ph_sensor::PhSensor::new_in(
                aog::ph_sensor::PhSensorType::Arduino,
                directory,
            )),
            Box::new(TestBoard),
        )
    }
    fn login(state: &WebState, directory: &Path, password: &str) -> Response {
        let body = format!("input_username=admin&input_password={}", password);
        handle_web_request(
            &web_request("POST", "/authenticate", Some("SID=chosen-by-client"), &body),
            state,
            directory,
        )
    }
    fn authenticated(state: &WebState) -> (String, String) {
        let id = "valid-session".to_string();
        let csrf = "test-request-token".to_string();
        state.sessions.lock().unwrap().insert(
            id.clone(),
            WebSession {
                created: Instant::now(),
                csrf: csrf.clone(),
            },
        );
        (format!("SID={}", id), csrf)
    }
    fn json_request(
        path: &str,
        cookie: Option<&str>,
        csrf: Option<&str>,
        body: serde_json::Value,
    ) -> Request {
        let mut headers = vec![("Content-Type".into(), "application/json".into())];
        if let Some(cookie) = cookie {
            headers.push(("Cookie".into(), cookie.into()));
        }
        if let Some(csrf) = csrf {
            headers.push(("X-AOG-CSRF".into(), csrf.into()));
        }
        Request::fake_https("POST", path, headers, serde_json::to_vec(&body).unwrap())
    }
    fn json_response(response: Response) -> serde_json::Value {
        let (reader, _) = response.data.into_reader_and_size();
        serde_json::from_reader(reader).unwrap()
    }
    #[test]
    fn login_cookie_authenticates_later_requests_and_logout_revokes_it() {
        let root = tempfile::tempdir().unwrap();
        let state = state(root.path());
        let request = web_request("GET", "/index.html", None, "");
        assert_eq!(
            header(
                &handle_web_request(&request, &state, root.path()),
                "Location"
            )
            .as_deref(),
            Some("/login.html")
        );
        let response = login(&state, root.path(), "TestPassword123!");
        assert_eq!(response.status_code, 302);
        let set_cookie = header(&response, "Set-Cookie").unwrap();
        assert!(set_cookie.contains("HttpOnly; Secure; SameSite=Lax"));
        let cookie = set_cookie.split(';').next().unwrap();
        assert_ne!(cookie, "SID=chosen-by-client");
        let request = web_request("GET", "/index.html", Some(cookie), "");
        assert_eq!(
            handle_web_request(&request, &state, root.path()).status_code,
            200
        );
        let request = web_request("GET", "/", Some(cookie), "");
        assert_eq!(
            header(
                &handle_web_request(&request, &state, root.path()),
                "Location"
            )
            .as_deref(),
            Some("/index.html")
        );
        let request = web_request("GET", "/logout", Some(cookie), "");
        assert!(header(
            &handle_web_request(&request, &state, root.path()),
            "Set-Cookie"
        )
        .unwrap()
        .contains("Max-Age=0"));
        let request = web_request("GET", "/api/settings", Some(cookie), "");
        assert_eq!(
            handle_web_request(&request, &state, root.path()).status_code,
            401
        );
    }
    #[test]
    fn invalid_credentials_expired_sessions_and_asset_like_paths_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let state = state(root.path());
        authenticated(&state);
        state.sessions.lock().unwrap().insert(
            "expired".into(),
            WebSession {
                created: Instant::now() - SESSION_LIFETIME - Duration::from_secs(1),
                csrf: "old".into(),
            },
        );
        for cookie in ["SID=expired", "SID=valid", "SID=", "SID=unknown"] {
            let request = web_request("GET", "/index.html", Some(cookie), "");
            assert_eq!(
                header(
                    &handle_web_request(&request, &state, root.path()),
                    "Location"
                )
                .as_deref(),
                Some("/login.html")
            );
        }
        assert_eq!(login(&state, root.path(), "wrong").status_code, 401);
        let request = web_request("GET", "/fake.js/../index.html", None, "");
        assert_eq!(
            header(
                &handle_web_request(&request, &state, root.path()),
                "Location"
            )
            .as_deref(),
            Some("/login.html")
        );
    }
    #[test]
    fn settings_require_session_and_csrf_and_never_expose_secrets() {
        let root = tempfile::tempdir().unwrap();
        let state = state(root.path());
        let (cookie, csrf) = authenticated(&state);
        let body = serde_json::json!({"setup":{"setup_name":"Kitchen grow", "relay_labels":["Light","Drain","Fill","Aux"]}});
        for (cookie, token, status) in [
            (None, None, 401),
            (Some(cookie.as_str()), None, 403),
            (Some(cookie.as_str()), Some("wrong"), 403),
        ] {
            let request = json_request("/api/settings", cookie, token, body.clone());
            assert_eq!(
                handle_web_request(&request, &state, root.path()).status_code,
                status
            );
        }
        let request = json_request("/api/settings", Some(&cookie), Some(&csrf), body);
        let response = handle_web_request(&request, &state, root.path());
        assert_eq!(response.status_code, 200);
        let json = json_response(response);
        assert_eq!(json["setup"]["setup_name"], "Kitchen grow");
        assert!(!json.to_string().contains("encrypted_password"));
        assert!(!json.to_string().contains("$argon2"));
        let saved = Config::load_in(root.path(), 0).unwrap();
        assert_eq!(saved.dashboard_config.setup_name, "Kitchen grow");
        assert_eq!(saved.encrypted_password, test_config().encrypted_password);
        let before = std::fs::read(root.path().join("data.json")).unwrap();
        let mut ph = crate::PhConfig::default();
        ph.optimal_min = 12.0;
        let request = json_request(
            "/api/settings",
            Some(&cookie),
            Some(&csrf),
            serde_json::json!({"ph":ph}),
        );
        assert_eq!(
            handle_web_request(&request, &state, root.path()).status_code,
            400
        );
        assert_eq!(
            std::fs::read(root.path().join("data.json")).unwrap(),
            before
        );
    }
    #[test]
    fn schedule_changes_persist_and_output_actions_require_csrf() {
        let root = tempfile::tempdir().unwrap();
        let state = state(root.path());
        let (cookie, csrf) = authenticated(&state);
        for path in [
            "/api/outputs/start",
            "/api/outputs/stop-all",
            "/api/schedules/pause",
            "/api/schedules/resume",
            "/api/account/password",
            "/api/ph/calibrate",
        ] {
            let request = json_request(path, Some(&cookie), None, serde_json::json!({}));
            assert_eq!(
                handle_web_request(&request, &state, root.path()).status_code,
                403
            );
        }
        let mut schedules = crate::ScheduleConfig::default();
        schedules.lighting.enabled = true;
        schedules.lighting.start_minute = 1320;
        schedules.lighting.end_minute = 360;
        schedules.watering.enabled = true;
        schedules.watering.every_minutes = 15;
        let request = json_request(
            "/api/settings",
            Some(&cookie),
            Some(&csrf),
            serde_json::json!({"schedules": schedules}),
        );
        assert_eq!(
            handle_web_request(&request, &state, root.path()).status_code,
            200
        );
        let reloaded = Config::load_in(root.path(), 0).unwrap();
        assert!(reloaded.schedules.lighting.enabled);
        assert_eq!(reloaded.schedules.lighting.start_minute, 1320);
        assert_eq!(reloaded.schedules.watering.every_minutes, 15);
        let request = json_request(
            "/api/schedules/pause",
            Some(&cookie),
            Some(&csrf),
            serde_json::json!({}),
        );
        assert_eq!(
            handle_web_request(&request, &state, root.path()).status_code,
            200
        );
        assert!(root.path().join("automation_paused").exists());
    }
    #[test]
    fn password_change_persists_immediately_and_revokes_all_sessions() {
        let root = tempfile::tempdir().unwrap();
        let state = state(root.path());
        let (cookie, csrf) = authenticated(&state);
        let make = |old: &str, new: &str, confirm: &str| {
            json_request(
                "/api/account/password",
                Some(&cookie),
                Some(&csrf),
                serde_json::json!({"current_password":old,"new_password":new,"confirm_password":confirm}),
            )
        };
        assert_eq!(
            handle_web_request(
                &make("wrong", "NewPassword456!", "NewPassword456!"),
                &state,
                root.path()
            )
            .status_code,
            403
        );
        assert_eq!(
            handle_web_request(
                &make("TestPassword123!", "short", "short"),
                &state,
                root.path()
            )
            .status_code,
            400
        );
        assert_eq!(
            handle_web_request(
                &make("TestPassword123!", "NewPassword456!", "different"),
                &state,
                root.path()
            )
            .status_code,
            400
        );
        let response = handle_web_request(
            &make("TestPassword123!", "NewPassword456!", "NewPassword456!"),
            &state,
            root.path(),
        );
        assert_eq!(response.status_code, 200);
        assert!(state.sessions.lock().unwrap().is_empty());
        assert_eq!(
            login(&state, root.path(), "TestPassword123!").status_code,
            401
        );
        assert_eq!(
            login(&state, root.path(), "NewPassword456!").status_code,
            302
        );
        let saved = Config::load_in(root.path(), 0).unwrap();
        assert!(aog::auth::verify_password("NewPassword456!", &saved.encrypted_password).unwrap());
    }
    #[test]
    fn ph_history_is_available_without_calibration_and_empty_history_is_explicit() {
        let root = tempfile::tempdir().unwrap();
        let mut history = aog::ph_sensor::PhHistory::default();
        history.add_reading(aog::ph_sensor::PhReading {
            ph_value: 0.0,
            raw_value: 0.0,
            temperature: 25.0,
            timestamp: 123,
            alert_level: aog::ph_sensor::PhAlertLevel::Critical,
        });
        std::fs::write(
            root.path().join("ph_history.json"),
            serde_json::to_vec(&history).unwrap(),
        )
        .unwrap();
        let state = state(root.path());
        let (cookie, _) = authenticated(&state);
        assert!(!root.path().join("ph_calibration.json").exists());
        let request = web_request("GET", "/api/ph", Some(&cookie), "");
        let data = json_response(handle_web_request(&request, &state, root.path()));
        // A saved sample remains history, not a live reading from the sensor.
        assert!(data["status"]["current_ph"].is_null());
        assert!(data["raw_value"].is_null());
        assert_eq!(data["history"].as_array().unwrap().len(), 1);
        assert_eq!(data["history"][0]["ph_value"], 0.0);
        assert_eq!(data["status"]["calibration_valid"], false);
        state.ph.configure(crate::PhConfig {
            history_size: 0,
            ..crate::PhConfig::default()
        });
        let request = web_request("GET", "/api/ph", Some(&cookie), "");
        let data = json_response(handle_web_request(&request, &state, root.path()));
        assert!(data["status"]["current_ph"].is_null());
        assert_eq!(data["history"], serde_json::json!([]));
    }

    fn command_request(remote: &str, authorization: Option<&str>) -> Request {
        let headers = authorization
            .map(|value| vec![("Authorization".to_owned(), value.to_owned())])
            .unwrap_or_default();
        Request::fake_https_from(
            remote.parse().unwrap(),
            "POST",
            "/api/command",
            headers,
            Vec::new(),
        )
    }

    #[test]
    fn lan_commands_require_a_configured_valid_token() {
        for remote in ["192.168.86.10:50000", "[2001:db8::1]:50000"] {
            let request = command_request(remote, None);
            assert!(!command_token_valid(&request, None));
            assert!(!command_token_valid(&request, Some("secret")));
            let request = command_request(remote, Some("Bearer wrong"));
            assert!(!command_token_valid(&request, Some("secret")));
            let request = command_request(remote, Some("Bearer secret"));
            assert!(command_token_valid(&request, Some("secret")));
            let request = command_request(remote, Some(""));
            assert!(!command_token_valid(&request, Some("")));
        }
        for remote in ["127.0.0.1:50000", "[::1]:50000"] {
            let request = command_request(remote, None);
            assert!(command_token_valid(&request, None));
            assert!(!command_token_valid(&request, Some("secret")));
        }
    }

    #[test]
    fn rate_limit_survives_reconnections_and_expires() {
        let limiter = Mutex::new(RateLimiter::new(10, 60));
        for port in 50000..50010 {
            let request =
                command_request(&format!("192.168.86.10:{}", port), Some("Bearer secret"));
            assert!(check_command_access(&request, Some("secret"), &limiter).is_none());
        }
        let request = command_request("192.168.86.10:51000", Some("Bearer secret"));
        assert_eq!(
            check_command_access(&request, Some("secret"), &limiter)
                .unwrap()
                .status_code,
            429
        );
        let other = command_request("192.168.86.11:51000", Some("Bearer secret"));
        assert!(check_command_access(&other, Some("secret"), &limiter).is_none());
        for requests in limiter.lock().unwrap().requests.values_mut() {
            requests.fill(Instant::now() - Duration::from_secs(61));
        }
        assert!(check_command_access(&request, Some("secret"), &limiter).is_none());
        assert_eq!(limiter.lock().unwrap().requests.len(), 1);
    }

    #[test]
    fn command_endpoint_rejects_wrong_paths_and_methods() {
        let limiter = Mutex::new(RateLimiter::new(10, 60));
        let request = web_request("GET", "/api/command", None, "");
        assert_eq!(
            check_command_access(&request, None, &limiter)
                .unwrap()
                .status_code,
            405
        );
        let request = web_request("POST", "/other", None, "");
        assert_eq!(
            check_command_access(&request, None, &limiter)
                .unwrap()
                .status_code,
            404
        );
    }
}

// Rate limiting for API endpoints
struct RateLimiter {
    requests: HashMap<std::net::IpAddr, Vec<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window_seconds: u64) -> Self {
        RateLimiter {
            requests: HashMap::new(),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    fn check_rate_limit(&mut self, client_id: std::net::IpAddr) -> bool {
        let now = Instant::now();
        self.requests.retain(|_, requests| {
            requests.retain(|time| now.duration_since(*time) < self.window);
            !requests.is_empty()
        });

        // Get or create request history for this client
        let requests = self.requests.entry(client_id).or_default();

        // Check if limit exceeded
        if requests.len() >= self.max_requests {
            return false;
        }

        // Add current request
        requests.push(now);
        true
    }
}

// LAN command requests require a token; tokenless access remains local only.
fn command_token_valid(request: &Request, expected_token: Option<&str>) -> bool {
    match expected_token {
        Some(expected) if !expected.is_empty() => request
            .header("Authorization")
            .is_some_and(|header| header.strip_prefix("Bearer ").unwrap_or(header) == expected),
        Some(_) => false,
        None => request.remote_addr().ip().is_loopback(),
    }
}

fn check_command_access(
    request: &Request,
    api_token: Option<&str>,
    limiter: &Mutex<RateLimiter>,
) -> Option<Response> {
    if request.url() != "/api/command" {
        return Some(Response::empty_404());
    }
    if request.method() != "POST" {
        return Some(
            Response::text("Method not allowed")
                .with_status_code(405)
                .with_additional_header("Allow", "POST"),
        );
    }
    if !command_token_valid(request, api_token) {
        return Some(Response::text("Unauthorized: Invalid API token").with_status_code(401));
    }
    // Source ports change on reconnect; the limit belongs to the client IP.
    let client_id = request.remote_addr().ip();
    if !limiter
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .check_rate_limit(client_id)
    {
        return Some(
            Response::text("Too Many Requests")
                .with_status_code(429)
                .with_additional_header("Retry-After", "60"),
        );
    }
    None
}

pub fn init_command_api() {
    let config = Arc::new(Mutex::new(match Config::load(0) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load config: {}", e);
            return;
        }
    }));

    let cert = match std::fs::read("/opt/aog/crt/default/aog.local.cert") {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to read certificate: {}", e);
            return;
        }
    };

    let pkey = match std::fs::read("/opt/aog/crt/default/aog.local.key") {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to read private key: {}", e);
            return;
        }
    };

    // Honor the configured interface and port.
    let bind_config = match config.lock() {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!(
                "Failed to acquire config lock for command API: {} - using defaults",
                e
            );
            return;
        }
    };
    let bind_address = bind_config
        .command_api_bind_address
        .clone()
        .unwrap_or_else(|| crate::DEFAULT_BIND_ADDRESS.to_string());
    let bind_port = bind_config.command_api_bind_port.unwrap_or(9443);
    let bind_addr = format!("{}:{}", bind_address, bind_port);
    drop(bind_config);

    // Initialize rate limiter: 10 requests per 60 seconds
    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(10, 60)));

    log::info!("Starting Command API server on {}", bind_addr);

    rouille::Server::new_ssl(
        (bind_address.as_str(), bind_port),
        move |request| {
            {
                // Observe token rotation immediately instead of retaining the
                // credential that happened to be configured at server startup.
                let api_token = RUNTIME_WEB_STATE.get().map(|state| {
                    state.config.lock().unwrap_or_else(|error| error.into_inner()).command_api_token.clone()
                }).unwrap_or_else(|| {
                    config.lock().unwrap_or_else(|error| error.into_inner()).command_api_token.clone()
                });
                if let Some(response) =
                    check_command_access(request, api_token.as_deref(), &rate_limiter)
                {
                    return response;
                }

                #[derive(Serialize, Deserialize, Debug, Clone)]
                struct CommandStatus {
                    status: String,
                    output: Option<String>,
                }

                let input = try_or_400!(post_input!(request, {
                    input_command: String,
                }));

                // The shared exact parser is the allowlist. Unknown commands
                // return an error instead of a fabricated successful result.
                // Execute command and capture output
                let output =
                    match aog::command_exec::execute_with_output(input.input_command.clone()) {
                        Ok(out) => Some(out),
                        Err(e) => {
                            log::error!("Command execution failed: {}", e);
                            return Response::json(&CommandStatus {
                                status: "error".into(),
                                output: Some(e.to_string()),
                            })
                            .with_status_code(400);
                        }
                    };

                // let arduino_response = crate::aog::sensors::get_arduino_raw();
                let response = Response::json(&CommandStatus {
                    status: "success".to_string(),
                    output,
                });
                return response;
            }
        },
        cert,
        pkey,
    )
    .map_err(|e| log::error!("Failed to start HTTPS server: {}", e))
    .ok()
    .map(|server| server.run());
}
