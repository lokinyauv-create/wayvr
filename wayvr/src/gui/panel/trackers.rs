//! Link to the "Калібрування трекерів" program (fbt-calibrator): it writes its state into
//! `status.json` and picks up one small file per button press from `commands/`.

use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const REFRESH: Duration = Duration::from_millis(250);
/// The program rewrites the status every 250 ms; older than this means it is not running
const STALE_SECS: f64 = 3.0;

#[derive(Deserialize, Default, Clone)]
pub struct TrackerInfo {
    pub role: String,
    pub ok: bool,
}

#[derive(Deserialize, Default, Clone)]
pub struct TrackerStatus {
    pub ts: f64,
    pub vr: bool,
    pub driver: bool,
    pub trackers: Vec<TrackerInfo>,
    pub override_left: Option<u32>,
    pub tpose: bool,
    pub dots: bool,
    pub stance_cm: i32,
}

fn cache_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".cache/fbt-calibrator")
}

struct Cache {
    read_at: Option<Instant>,
    status: Option<Arc<TrackerStatus>>,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    read_at: None,
    status: None,
});

/// Current state of the program, or `None` if it is not running
pub fn status() -> Option<Arc<TrackerStatus>> {
    let mut cache = CACHE.lock().ok()?;
    if cache.read_at.is_none_or(|t| t.elapsed() >= REFRESH) {
        cache.read_at = Some(Instant::now());
        let parsed = std::fs::read_to_string(cache_dir().join("status.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<TrackerStatus>(&text).ok());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        cache.status = parsed
            .filter(|s| now - s.ts < STALE_SECS)
            .map(Arc::new);
    }
    cache.status.clone()
}

/// Asks the program to do something (`override`, `reset`, `toggle tpose`, `stance +2`, ...)
pub fn send(command: &str) {
    let dir = cache_dir().join("commands");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("Could not create {}: {e:?}", dir.display());
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".{stamp}.tmp"));
    let file = dir.join(format!("{stamp}.cmd"));
    // the program must not see a half-written file: write aside, then rename
    if let Err(e) = std::fs::write(&tmp, command).and_then(|()| std::fs::rename(&tmp, &file)) {
        log::warn!("Could not send tracker command {command:?}: {e:?}");
    }
}
