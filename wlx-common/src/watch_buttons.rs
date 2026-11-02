//! Buttons on the watch: the built-in ones and the user's own ones ("plugins"),
//! one JSON file each in `~/.config/wayvr/watch-buttons/`. Another program can add
//! a button just by dropping such a file there. Example `discord-mute.json`:
//!
//! ```json
//! {
//!   "name": "Discord",
//!   "tooltip": "Вимкнути мікрофон у Discord",
//!   "icon": "discord-mute.svg",
//!   "press": "./discord-mute.sh toggle",
//!   "active": "./discord-mute.sh is-muted"
//! }
//! ```
//!
//! `press` runs on a click, `active` is polled every `interval_ms`: exit code 0 lights the
//! button up. Both run with `sh -c` inside the `watch-buttons` folder. `icon` is an SVG file
//! (relative to that folder or absolute); without it the button shows only its name.
//!
//! Which buttons are shown and in what order lives in `GeneralConfig::watch_buttons_order`
//! and `GeneralConfig::watch_buttons_hidden`; the dashboard edits them.

use std::path::PathBuf;

use serde::Deserialize;

use crate::{config::GeneralConfig, config_io};

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WatchPlugin {
	/// short text under the icon
	pub name: String,
	#[serde(default)]
	pub tooltip: Option<String>,
	#[serde(default)]
	pub icon: Option<String>,
	pub press: String,
	#[serde(default)]
	pub active: Option<String>,
	#[serde(default = "def_interval")]
	pub interval_ms: u64,
}

const fn def_interval() -> u64 {
	1000
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchButton {
	/// `center`, `keyboard`, ... for built-in buttons, `plugin:<file name>` for the user's own
	pub id: String,
	/// name for the dashboard list
	pub title: String,
	pub plugin: Option<WatchPlugin>,
}

/// Built-in buttons in their default order: id and the name shown in the dashboard
pub const BUILTIN: &[(&str, &str)] = &[
	("center", "Центрувати (монітори й клавіатуру туди, куди дивишся)"),
	("keyboard", "Клавіатура"),
	("trackers", "Калібрування трекерів"),
	("dashboard", "Параметри (ця панель)"),
	("edit", "Редагування вікон"),
];

pub const PLUGIN_PREFIX: &str = "plugin:";

pub fn plugins_dir() -> PathBuf {
	config_io::get_config_root().join("watch-buttons")
}

impl WatchPlugin {
	/// Absolute path of the icon file, if it has one
	pub fn icon_path(&self) -> Option<PathBuf> {
		let icon = self.icon.as_deref().filter(|i| !i.is_empty())?;
		Some(plugins_dir().join(icon))
	}
}

/// The user's own buttons, sorted by file name
pub fn load_plugins() -> Vec<(String, WatchPlugin)> {
	let Ok(entries) = std::fs::read_dir(plugins_dir()) else {
		return Vec::new();
	};

	let mut plugins: Vec<(String, WatchPlugin)> = entries
		.filter_map(Result::ok)
		.filter_map(|entry| {
			let path = entry.path();
			if !path.extension()?.to_str()?.eq_ignore_ascii_case("json") {
				return None;
			}
			let stem = path.file_stem()?.to_str()?.to_owned();
			let text = std::fs::read_to_string(&path).ok()?;
			match serde_json::from_str::<WatchPlugin>(&text) {
				Ok(plugin) => Some((format!("{PLUGIN_PREFIX}{stem}"), plugin)),
				Err(e) => {
					log::warn!("Watch button {}: {e}", path.display());
					None
				}
			}
		})
		.collect();

	plugins.sort_by(|a, b| a.0.cmp(&b.0));
	plugins
}

/// Every button there is, in the user's order: first the ones from `watch_buttons_order`,
/// then the rest (new plugins included) in the default order.
pub fn all_buttons(config: &GeneralConfig) -> Vec<WatchButton> {
	let mut rest: Vec<WatchButton> = BUILTIN
		.iter()
		.map(|(id, title)| WatchButton {
			id: (*id).to_string(),
			title: (*title).to_string(),
			plugin: None,
		})
		.chain(load_plugins().into_iter().map(|(id, plugin)| WatchButton {
			id,
			title: plugin.tooltip.clone().unwrap_or_else(|| plugin.name.clone()),
			plugin: Some(plugin),
		}))
		.collect();

	let mut ordered = Vec::with_capacity(rest.len());
	for id in &config.watch_buttons_order {
		if let Some(pos) = rest.iter().position(|b| *b.id == **id) {
			ordered.push(rest.remove(pos));
		}
	}
	ordered.append(&mut rest);
	ordered
}

pub fn is_shown(config: &GeneralConfig, id: &str) -> bool {
	!config.watch_buttons_hidden.iter().any(|h| &**h == id)
}

/// The buttons the watch shows, in order
pub fn shown_buttons(config: &GeneralConfig) -> Vec<WatchButton> {
	all_buttons(config)
		.into_iter()
		.filter(|b| is_shown(config, &b.id))
		.collect()
}
