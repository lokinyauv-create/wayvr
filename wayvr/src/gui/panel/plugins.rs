//! Runs the user's own watch buttons (see `wlx_common::watch_buttons`): a press runs the
//! button's `press` command, and a background thread polls each `active` command
//! so the watch can light the button up.

use std::{
    collections::HashMap,
    process::{Command, Stdio},
    sync::{
        Mutex, Once,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use wlx_common::watch_buttons::{WatchPlugin, plugins_dir};

struct Registry {
    plugins: HashMap<String, WatchPlugin>,
    active: HashMap<String, bool>,
    next_check: HashMap<String, Instant>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);
static WAKE: AtomicBool = AtomicBool::new(false);
static WORKER: Once = Once::new();

const POLL: Duration = Duration::from_millis(200);

/// Tells the poller which buttons exist now (called whenever the watch rebuilds its buttons)
pub fn set_plugins(list: &[(String, WatchPlugin)]) {
    if let Ok(mut reg) = REGISTRY.lock() {
        let old = reg.take();
        let mut active = HashMap::new();
        if let Some(old) = old {
            for (id, _) in list {
                if let Some(v) = old.active.get(id) {
                    active.insert(id.clone(), *v);
                }
            }
        }
        *reg = Some(Registry {
            plugins: list.iter().cloned().collect(),
            active,
            next_check: HashMap::new(),
        });
    }
    WAKE.store(true, Ordering::Relaxed);
    WORKER.call_once(|| {
        let _ = thread::Builder::new()
            .name("watch-buttons".into())
            .spawn(worker);
    });
}

pub fn has_state(id: &str) -> bool {
    REGISTRY
        .lock()
        .ok()
        .and_then(|reg| reg.as_ref()?.plugins.get(id).map(|p| p.active.is_some()))
        .unwrap_or(false)
}

pub fn is_active(id: &str) -> bool {
    REGISTRY
        .lock()
        .ok()
        .and_then(|reg| reg.as_ref()?.active.get(id).copied())
        .unwrap_or(false)
}

fn shell(cmd: &str) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(cmd)
        .current_dir(plugins_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Runs the button's `press` command, then re-checks its state right away
pub fn press(id: &str) {
    let Some(cmd) = REGISTRY
        .lock()
        .ok()
        .and_then(|reg| Some(reg.as_ref()?.plugins.get(id)?.press.clone()))
    else {
        log::warn!("Unknown watch button {id}");
        return;
    };

    log::info!("Watch button {id}: running {cmd:?}");
    let id = id.to_string();
    let _ = thread::Builder::new()
        .name("watch-button".into())
        .spawn(move || {
            match shell(&cmd).status() {
                Ok(status) if !status.success() => {
                    log::warn!("Watch button {id}: {cmd:?} exited with {status}");
                }
                Err(e) => log::warn!("Watch button {id}: could not run {cmd:?}: {e}"),
                Ok(_) => {}
            }
            if let Ok(mut reg) = REGISTRY.lock()
                && let Some(reg) = reg.as_mut()
            {
                reg.next_check.remove(&id);
            }
            WAKE.store(true, Ordering::Relaxed);
        });
}

fn worker() {
    loop {
        // which `active` commands are due
        let due: Vec<(String, String, u64)> = {
            let Ok(reg) = REGISTRY.lock() else {
                return;
            };
            let Some(reg) = reg.as_ref() else {
                thread::sleep(POLL);
                continue;
            };
            let now = Instant::now();
            reg.plugins
                .iter()
                .filter_map(|(id, p)| {
                    let cmd = p.active.clone()?;
                    let due = reg.next_check.get(id).is_none_or(|t| *t <= now);
                    due.then(|| (id.clone(), cmd, p.interval_ms.max(100)))
                })
                .collect()
        };

        for (id, cmd, interval) in due {
            let on = shell(&cmd).status().is_ok_and(|s| s.success());
            if let Ok(mut reg) = REGISTRY.lock()
                && let Some(reg) = reg.as_mut()
            {
                reg.active.insert(id.clone(), on);
                reg.next_check
                    .insert(id, Instant::now() + Duration::from_millis(interval));
            }
        }

        // sleep until the next poll, or less if a button was just pressed
        for _ in 0..5 {
            if WAKE.swap(false, Ordering::Relaxed) {
                break;
            }
            thread::sleep(POLL / 5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_runs_command_and_poll_sees_state() {
        let marker = std::env::temp_dir().join("wayvr-plugin-test-marker");
        let _ = std::fs::remove_file(&marker);
        let plugin = WatchPlugin {
            name: "t".into(),
            tooltip: None,
            icon: None,
            press: format!("touch '{}'", marker.display()),
            active: Some(format!("test -f '{}'", marker.display())),
            interval_ms: 100,
        };
        set_plugins(&[("plugin:t".into(), plugin)]);
        assert!(has_state("plugin:t"));
        press("plugin:t");
        thread::sleep(Duration::from_millis(800));
        assert!(marker.exists(), "press did not run");
        assert!(is_active("plugin:t"), "poller did not see the state");
        let _ = std::fs::remove_file(&marker);
    }
}
