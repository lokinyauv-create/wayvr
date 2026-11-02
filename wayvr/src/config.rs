use config::{Config, File};
use log::error;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use wayvr_ipc::packet_client::WvrProcessLaunchParams;
use wlx_common::{
    astr_containers::AStrMap,
    config::{
        AltModifier, CaptureMethod, DefaultPositioning, GeneralConfig,
        HandsfreeAltTab, HandsfreePointer, InputCaptureMethod, InputEmulationMethod,
        SerializedWindowSet,
    },
    config_io,
    locale::Language,
    overlays::BackendAttribValue,
};

const FALLBACKS: [&str; 1] = [include_str!("res/keyboard.yaml")];

const FILES: [&str; 1] = ["keyboard.yaml"];

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum ConfigType {
    Keyboard,
}

pub fn load_known_yaml<T>(config_type: ConfigType) -> T
where
    T: for<'de> Deserialize<'de>,
{
    let fallback = FALLBACKS[config_type as usize];
    let file_name = FILES[config_type as usize];
    let maybe_override = config_io::load(file_name);

    for yaml in [maybe_override.as_deref(), Some(fallback)].iter().flatten() {
        match serde_yaml::from_str::<T>(yaml) {
            Ok(d) => return d,
            Err(e) => {
                error!("Failed to parse {file_name}, falling back to defaults.");
                error!("{e}");
            }
        }
    }
    // can only get here if internal fallback is broken
    panic!("No usable config found.");
}

const SUPPORTED_EXTESIONS: [&str; 4] = ["yaml", "yml", "json", "json5"];

fn is_supported_config_file(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            for sup in SUPPORTED_EXTESIONS {
                if sup.eq_ignore_ascii_case(ext) {
                    return true;
                }
            }

            log::error!(
                "Unsupported file extension: \".{}\" in {}. Ignoring file.",
                ext,
                path.to_string_lossy()
            );
            debug_assert!(false);
            false
        }
        _ => false,
    }
}

pub fn load_config_with_conf_d<ConfigData>(
    root_config_filename: &str,
    ctype: config_io::ConfigRoot,
) -> ConfigData
where
    ConfigData: for<'de> Deserialize<'de>,
{
    let mut settings_builder = Config::builder();

    // Add files from conf.d directory
    let path_conf_d = ctype.get_conf_d_path();

    for mut base_conf in [config_io::get_config_root(), path_conf_d.clone()] {
        base_conf.push(root_config_filename);
        if base_conf.exists() {
            log::info!("Loading config file: {}", base_conf.to_string_lossy());
            settings_builder = settings_builder.add_source(File::from(base_conf));
        }
    }

    if let Ok(paths_unsorted) = std::fs::read_dir(path_conf_d) {
        let mut paths: Vec<_> = paths_unsorted
            .filter_map(|r| match r {
                Ok(entry) if is_supported_config_file(&entry.path()) => Some(entry),
                Ok(_other) => None,
                Err(e) => {
                    error!("Failed to read conf.d directory: {e}");
                    None
                }
            })
            .collect();
        // Sort paths alphabetically
        paths.sort_by_key(std::fs::DirEntry::path);
        for path in paths {
            log::info!("Loading config file: {}", path.path().to_string_lossy());
            settings_builder = settings_builder.add_source(File::from(path.path()));
        }
    }

    match settings_builder.build() {
        Ok(settings) => match settings.try_deserialize::<ConfigData>() {
            Ok(config) => config,
            Err(e) => {
                panic!("Failed to deserialize settings: {e}");
            }
        },
        Err(e) => {
            panic!("Failed to build settings: {e}");
        }
    }
}

pub fn load_general_config() -> GeneralConfig {
    load_config_with_conf_d::<GeneralConfig>("config.yaml", config_io::ConfigRoot::Generic)
}

#[derive(Serialize)]
pub struct AutoSettings {
    pub ui_animation_speed: f32,
    pub ui_round_multiplier: f32,
    pub ui_gradient_intensity: f32,
    pub click_freeze_time_ms: i32,
    pub invert_scroll_direction_x: bool,
    pub invert_scroll_direction_y: bool,
    pub scroll_speed: f32,
    pub long_press_duration: f32,
    pub notifications_enabled: bool,
    pub notifications_sound_enabled: bool,
    pub keyboard_sound_enabled: bool,
    pub upright_screen_fix: bool,
    pub double_cursor_fix: bool,
    pub enable_watch: bool,
    pub hide_grab_help: bool,
    pub allow_sliding: bool,
    pub focus_follows_mouse_mode: bool,
    pub left_handed_mouse: bool,
    pub block_game_input: bool,
    pub block_game_input_ignore_watch: bool,
    pub block_poses_on_kbd_interaction: bool,
    pub watch_view_angle_min: f32,
    pub watch_view_angle_max: f32,
    pub screen_render_down: bool,
    pub pointer_lerp_factor: f32,
    pub clock_12h: bool,
    pub watch_buttons_order: Vec<Arc<str>>,
    pub watch_buttons_hidden: Vec<Arc<str>>,
    pub opaque_background: bool,
    pub capture_method: CaptureMethod,
    pub keyboard_middle_click_mode: AltModifier,
    pub autostart_apps: Vec<WvrProcessLaunchParams>,
    pub handsfree_pointer: HandsfreePointer,
    pub handsfree_alt_tab: HandsfreeAltTab,
    pub language: Option<Language>,
    pub input_emulation_method: InputEmulationMethod,
    pub tutorial_graduated: bool,
    pub default_overlay_scale: f32,
    pub color_palette: Arc<str>,
    pub snap_angle_deg: f32,
    pub wvr_mouse_acceleration: bool,
    pub wvr_mouse_speed: f32,
    pub wvr_input_capture: InputCaptureMethod,
    pub default_curvature: f32,
    pub default_opacity: f32,
    pub default_positioning: DefaultPositioning,
}

fn get_settings_path() -> PathBuf {
    config_io::ConfigRoot::Generic
        .get_conf_d_path()
        .join("zz-saved-config.json5")
}

pub fn save_settings(config: &GeneralConfig) -> anyhow::Result<()> {
    let conf = AutoSettings {
        ui_animation_speed: config.ui_animation_speed,
        ui_round_multiplier: config.ui_round_multiplier,
        ui_gradient_intensity: config.ui_gradient_intensity,
        click_freeze_time_ms: config.click_freeze_time_ms,
        invert_scroll_direction_x: config.invert_scroll_direction_x,
        invert_scroll_direction_y: config.invert_scroll_direction_y,
        scroll_speed: config.scroll_speed,
        long_press_duration: config.long_press_duration,
        notifications_enabled: config.notifications_enabled,
        notifications_sound_enabled: config.notifications_sound_enabled,
        keyboard_sound_enabled: config.keyboard_sound_enabled,
        upright_screen_fix: config.upright_screen_fix,
        double_cursor_fix: config.double_cursor_fix,
        enable_watch: config.enable_watch,
        hide_grab_help: config.hide_grab_help,
        allow_sliding: config.allow_sliding,
        focus_follows_mouse_mode: config.focus_follows_mouse_mode,
        left_handed_mouse: config.left_handed_mouse,
        block_game_input: config.block_game_input,
        block_game_input_ignore_watch: config.block_game_input_ignore_watch,
        block_poses_on_kbd_interaction: config.block_poses_on_kbd_interaction,
        watch_view_angle_min: config.watch_view_angle_min,
        watch_view_angle_max: config.watch_view_angle_max,
        screen_render_down: config.screen_render_down,
        pointer_lerp_factor: config.pointer_lerp_factor,
        clock_12h: config.clock_12h,
        watch_buttons_order: config.watch_buttons_order.clone(),
        watch_buttons_hidden: config.watch_buttons_hidden.clone(),
        opaque_background: config.opaque_background,
        capture_method: config.capture_method,
        keyboard_middle_click_mode: config.keyboard_middle_click_mode,
        autostart_apps: config.autostart_apps.clone(),
        handsfree_pointer: config.handsfree_pointer,
        handsfree_alt_tab: config.handsfree_alt_tab,
        language: config.language,
        input_emulation_method: config.input_emulation_method,
        tutorial_graduated: config.tutorial_graduated,
        default_overlay_scale: config.default_overlay_scale,
        color_palette: config.color_palette.clone(),
        snap_angle_deg: config.snap_angle_deg,
        wvr_mouse_acceleration: config.wvr_mouse_acceleration,
        wvr_mouse_speed: config.wvr_mouse_speed,
        wvr_input_capture: config.wvr_input_capture,
        default_curvature: config.default_curvature,
        default_opacity: config.default_opacity,
        default_positioning: config.default_positioning,
    };

    let json = serde_json::to_string_pretty(&conf).unwrap(); // want panic
    std::fs::write(get_settings_path(), json)?;

    log::info!("Saved settings.");
    Ok(())
}

// Config that is saved after manipulating overlays

#[derive(Serialize)]
pub struct AutoState {
    pub sets: Vec<SerializedWindowSet>,
    pub spawn_overlays: Vec<Arc<str>>,
    pub last_set: u32,
    pub attribs: AStrMap<Vec<BackendAttribValue>>,
}

fn get_state_path() -> PathBuf {
    config_io::ConfigRoot::Generic
        .get_conf_d_path()
        .join("zz-saved-state.json5")
}

pub fn save_state(config: &GeneralConfig) -> anyhow::Result<()> {
    let conf = AutoState {
        sets: config.sets.clone(),
        last_set: config.last_set,
        spawn_overlays: config.spawn_overlays.clone(),
        attribs: config.attribs.clone(),
    };

    let json = serde_json::to_string_pretty(&conf).unwrap(); // want panic
    std::fs::write(get_state_path(), json)?;

    log::info!("State was saved successfully.");
    Ok(())
}

pub fn none_if_0(val: f32) -> Option<f32> {
    if val.abs() < f32::EPSILON {
        None
    } else {
        Some(val)
    }
}
