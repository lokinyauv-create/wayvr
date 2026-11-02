use std::{fs::File, io::BufReader};

use serde::{Deserialize, Deserializer, de::Error as _};
use wgui::{
	drawing,
	log::LogErr,
	palette::{PALETTES, WguiColorPalette},
};

use crate::config_io;

pub fn load_palette(want_palette: &str) -> WguiColorPalette {
	if is_custom_palette(want_palette) {
		if let Ok(palette) = load_custom_palette(want_palette).log_warn("Could not load custom color palette") {
			return palette;
		}
	} else {
		for (name, palette) in PALETTES {
			if *name == want_palette {
				return (*palette).clone();
			}
		}
		log::warn!("No color palette found with name: {want_palette}")
	}
	PALETTES[0].1.clone()
}

fn is_custom_palette(name: &str) -> bool {
	name
		.as_bytes()
		.get(name.len().saturating_sub(5)..)
		.is_some_and(|suffix| suffix.eq_ignore_ascii_case(b".json"))
}

pub fn list_palette_files() -> Vec<String> {
	let path = config_io::get_palettes_root();
	let Ok(entries) = std::fs::read_dir(path) else {
		return Vec::new();
	};

	let mut files: Vec<_> = entries
		.filter_map(Result::ok)
		.filter_map(|entry| {
			let path = entry.path();

			if !path.is_file() || !path.extension()?.to_str()?.eq_ignore_ascii_case("json") {
				return None;
			}

			Some(path.file_name()?.to_str()?.to_owned())
		})
		.collect();

	files.sort_unstable();
	files
}

pub fn load_custom_palette(name: &str) -> anyhow::Result<WguiColorPalette> {
	let path = config_io::get_palettes_root().join(name);
	let file = File::open(path)?;
	let reader = BufReader::new(file);
	let scp = serde_json::from_reader::<_, SerializedWguiColorPalette>(reader)?;
	Ok(scp.into_palette())
}

fn deserialize_color<'de, D>(deserializer: D) -> Result<drawing::Color, D::Error>
where
	D: Deserializer<'de>,
{
	let value = String::deserialize(deserializer)?;

	drawing::Color::from_hex(&value).ok_or_else(|| D::Error::custom(format!("invalid hexadecimal color: {value:?}")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SerializedWguiColorPalette {
	#[serde(deserialize_with = "deserialize_color")]
	primary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_primary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	secondary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_secondary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	tertiary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_tertiary: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	danger: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_danger: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	background: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_background: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	background_variant: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_background_variant: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	background_contrast: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	on_background_contrast: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	outline: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	shadow: drawing::Color,

	#[serde(deserialize_with = "deserialize_color")]
	highlight: drawing::Color,
}

impl SerializedWguiColorPalette {
	pub fn into_palette(self) -> WguiColorPalette {
		// order must stay the same as per WguiColorName
		WguiColorPalette::from_inner([
			self.primary,
			self.on_primary,
			self.secondary,
			self.on_secondary,
			self.tertiary,
			self.on_tertiary,
			self.danger,
			self.on_danger,
			self.background,
			self.on_background,
			self.background_variant,
			self.on_background_variant,
			self.background_contrast,
			self.on_background_contrast,
			self.outline,
			self.shadow,
			self.highlight,
		])
	}
}

/// The colors a user picks in the dashboard's theme editor; the rest of a palette is mixed from these
#[derive(Clone, Copy)]
pub struct ThemeColors {
	pub primary: drawing::Color,
	pub secondary: drawing::Color,
	pub tertiary: drawing::Color,
	pub background: drawing::Color,
	pub text: drawing::Color,
	pub danger: drawing::Color,
}

const BLACK: drawing::Color = drawing::Color::new(0.0, 0.0, 0.0, 1.0);
const WHITE: drawing::Color = drawing::Color::new(1.0, 1.0, 1.0, 1.0);

fn luminance(c: drawing::Color) -> f32 {
	0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

/// Readable text color on top of `c`, tinted with its hue
fn on_color(c: drawing::Color) -> drawing::Color {
	if luminance(c) > 0.45 {
		c.lerp(&BLACK, 0.82)
	} else {
		c.lerp(&WHITE, 0.88)
	}
}

impl ThemeColors {
	pub fn from_palette(p: &WguiColorPalette) -> Self {
		use wgui::color::WguiColorName as N;
		Self {
			primary: p[N::Primary],
			secondary: p[N::Secondary],
			tertiary: p[N::Tertiary],
			background: p[N::Background],
			text: p[N::OnBackground],
			danger: p[N::Danger],
		}
	}

	pub fn to_palette(&self) -> WguiColorPalette {
		let bg = self.background;
		let dark = luminance(bg) < 0.5;
		// order must stay the same as per WguiColorName
		WguiColorPalette::from_inner([
			self.primary,
			on_color(self.primary),
			self.secondary,
			on_color(self.secondary),
			self.tertiary,
			on_color(self.tertiary),
			self.danger,
			on_color(self.danger),
			bg,
			self.text,
			bg.lerp(&self.text, 0.12),
			self.text.lerp(&bg, 0.2),
			bg.lerp(&BLACK, if dark { 0.3 } else { 0.08 }),
			self.text,
			bg.lerp(&self.text, 0.25),
			bg.lerp(&BLACK, 0.1),
			if dark { WHITE } else { BLACK },
		])
	}
}

/// Writes a palette in the format `load_custom_palette` reads. `name` ends with `.json`
pub fn save_custom_palette(name: &str, palette: &WguiColorPalette) -> anyhow::Result<()> {
	use wgui::color::WguiColorName as N;
	let fields = [
		("primary", N::Primary),
		("on_primary", N::OnPrimary),
		("secondary", N::Secondary),
		("on_secondary", N::OnSecondary),
		("tertiary", N::Tertiary),
		("on_tertiary", N::OnTertiary),
		("danger", N::Danger),
		("on_danger", N::OnDanger),
		("background", N::Background),
		("on_background", N::OnBackground),
		("background_variant", N::BackgroundVariant),
		("on_background_variant", N::OnBackgroundVariant),
		("background_contrast", N::BackgroundContrast),
		("on_background_contrast", N::OnBackgroundContrast),
		("outline", N::Outline),
		("shadow", N::Shadow),
		("highlight", N::Highlight),
	];
	let map: serde_json::Map<String, serde_json::Value> = fields
		.iter()
		.map(|(key, name)| ((*key).to_string(), serde_json::Value::String(palette[*name].to_hex_rgb())))
		.collect();

	let root = config_io::get_palettes_root();
	std::fs::create_dir_all(&root)?;
	std::fs::write(root.join(name), serde_json::to_string_pretty(&map)?)?;
	Ok(())
}

/// First free "Моя тема N.json"
pub fn new_custom_palette_name() -> String {
	let existing = list_palette_files();
	(1..)
		.map(|n| format!("Моя тема {n}.json"))
		.find(|name| !existing.contains(name))
		.unwrap_or_default()
}
