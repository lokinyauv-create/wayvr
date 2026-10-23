use std::rc::Rc;

use chrono::DateTime;
use chrono::Local;
use chrono_tz::Tz;
use wgui::color::WguiColor;
use wgui::globals::WguiGlobals;
use wgui::{
    drawing,
    event::{self, EventCallback},
    i18n::Translation,
    layout::Layout,
    parser::{CustomAttribsInfoOwned, ParserState},
    widget::{EventResult, label::WidgetLabel},
};

use crate::{
    gui::panel::{log_invalid_attrib, log_missing_attrib},
    state::AppState,
};

fn parse_color(
    attribs: &CustomAttribsInfoOwned,
    key: &str,
    globals: &WguiGlobals,
    default: drawing::Color,
) -> WguiColor {
    let Some(value) = attribs.get_value(key) else {
        return default.into();
    };
    if let Some(color) = drawing::Color::from_hex(value) {
        return color.into();
    }
    if let Some(color) = globals.get().palette.find(value) {
        return color;
    }
    default.into()
}

#[allow(clippy::too_many_lines)]
pub(super) fn setup_custom_label<S: 'static>(
    layout: &mut Layout,
    parser_state: &ParserState,
    attribs: &CustomAttribsInfoOwned,
    app: &AppState,
) {
    const TAG: &str = "label";

    let Some(source) = attribs.get_value("_source") else {
        log_missing_attrib(parser_state, TAG, "_source");
        return;
    };

    let callback: EventCallback<AppState, S> = match source {
        "battery" => {
            let Some(device) = attribs.get_value("_device") else {
                log_missing_attrib(parser_state, TAG, "_device");
                return;
            };
            let Ok(device) = device.parse::<usize>() else {
                let msg = format!("expected integer, found \"{device}\"");
                log_invalid_attrib(parser_state, TAG, "_device", &msg);
                return;
            };

            let state = BatteryLabelState {
                low_color: parse_color(attribs, "_low_color", &layout.state.globals, BAT_LOW),
                normal_color: parse_color(
                    attribs,
                    "_normal_color",
                    &layout.state.globals,
                    BAT_NORMAL,
                ),
                charging_color: parse_color(
                    attribs,
                    "_charging_color",
                    &layout.state.globals,
                    BAT_CHARGING,
                ),
                low_threshold: attribs
                    .get_value("_low_threshold")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(BAT_LOW_THRESHOLD),
                device,
            };
            Box::new(move |common, data, app, _| {
                battery_on_tick(&state, common, data, app);
                Ok(EventResult::Pass)
            })
        }
        "clock" => {
            let Some(display) = attribs.get_value("_display") else {
                log_missing_attrib(parser_state, TAG, "_display");
                return;
            };

            let tz_str = attribs
                .get_value("_timezone")
                .and_then(|tz| {
                    tz.parse::<usize>()
                        .inspect_err(|_| {
                            let msg = format!("expected integer, found \"{tz}\"");
                            log_invalid_attrib(parser_state, TAG, "_timezone", &msg);
                        })
                        .ok()
                })
                .and_then(|tz_idx| {
                    app.session.config.timezones.get(tz_idx).or_else(|| {
                        let msg = format!("timezone index \"{tz_idx}\" is out of range");
                        log_invalid_attrib(parser_state, TAG, "_timezone", &msg);
                        None
                    })
                });

            let format = match display {
                "name" => {
                    let maybe_pretty_tz = tz_str.and_then(|tz_name| {
                        tz_name.split('/').next_back().map(|x| x.replace('_', " "))
                    });

                    let pretty_tz = maybe_pretty_tz.as_ref().map_or("Local", |x| x.as_str());

                    let mut globals = layout.state.globals.get();
                    layout
                        .state
                        .widgets
                        .cast_as::<WidgetLabel>(attribs.widget_id)
                        .unwrap()
                        .set_text_simple(&mut globals, Translation::from_raw_text(pretty_tz));

                    // does not need to be dynamic
                    return;
                }
                "date" => "%x",
                "dow" => "%A",
                "dow_short" => "%a",
                "time" => {
                    if app.session.config.clock_12h {
                        "%I:%M %p"
                    } else {
                        "%H:%M"
                    }
                }
                unk => unk,
            };

            let state = ClockLabelState {
                timezone: tz_str.and_then(|tz| {
                    tz.parse()
                        .inspect_err(|e| log::warn!("Invalid timezone: {e:?}"))
                        .ok()
                }),
                format: format.into(),
                locale: {
                    let i18n = app.wgui_globals.i18n();
                    let locale = i18n.get_locale();
                    match pure_rust_locales::Locale::try_from(locale.to_string().as_str()) {
                        Ok(loc) => loc,
                        Err(_) => pure_rust_locales::Locale::en_US,
                    }
                },
            };

            Box::new(move |common, data, _, _| {
                clock_on_tick(&state, common, data);
                Ok(EventResult::Pass)
            })
        }
        "timer" => {
            let format = attribs.get_value("_format").unwrap_or("%h:%m");

            let state = TimerLabelState {
                start: Local::now(),
                format: format.into(),
            };

            Box::new(move |common, data, _, _| {
                timer_on_tick(&state, common, data);
                Ok(EventResult::Pass)
            })
        }
        "ipd" => Box::new(|common, data, app, _| {
            ipd_on_tick(common, data, app);
            Ok(EventResult::Pass)
        }),
        "fps" => {
            let display = match attribs.get_value("_display").unwrap_or("fps") {
                "fps" => FpsDisplay::Fps,
                "reprojection" => FpsDisplay::Reprojection,
                unk => {
                    log_invalid_attrib(parser_state, TAG, "_display", unk);
                    return;
                }
            };

            let globals = &layout.state.globals;
            let state = FpsLabelState {
                display,
                good_color: parse_color(attribs, "_good_color", globals, FPS_GOOD),
                warn_color: parse_color(attribs, "_warn_color", globals, FPS_WARN),
                bad_color: parse_color(attribs, "_bad_color", globals, FPS_BAD),
            };
            Box::new(move |common, data, app, _| {
                fps_on_tick(&state, common, data, app);
                Ok(EventResult::Pass)
            })
        }
        "trackers" => {
            let display = match attribs.get_value("_display").unwrap_or("status") {
                "status" => TrackersDisplay::Status,
                "override" => TrackersDisplay::Override,
                "stance" => TrackersDisplay::Stance,
                "tpose" => TrackersDisplay::TPose,
                "dots" => TrackersDisplay::Dots,
                unk => {
                    log_invalid_attrib(parser_state, TAG, "_display", unk);
                    return;
                }
            };
            Box::new(move |common, data, _app, _| {
                trackers_on_tick(&display, common, data);
                Ok(EventResult::Pass)
            })
        }
        unk => {
            log_invalid_attrib(parser_state, TAG, "_source", unk);
            return;
        }
    };

    layout.add_event_listener(
        attribs.widget_id,
        wgui::event::EventListenerKind::InternalStateChange,
        callback,
    );
}

const BAT_LOW: drawing::Color = drawing::Color::new(0.69, 0.38, 0.38, 1.);
const BAT_NORMAL: drawing::Color = drawing::Color::new(0.55, 0.84, 0.79, 1.);
const BAT_CHARGING: drawing::Color = drawing::Color::new(0.38, 0.50, 0.62, 1.);
const BAT_LOW_THRESHOLD: u32 = 30;

struct BatteryLabelState {
    device: usize,
    low_color: WguiColor,
    normal_color: WguiColor,
    charging_color: WguiColor,
    low_threshold: u32,
}

fn battery_on_tick(
    state: &BatteryLabelState,
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
    app: &AppState,
) {
    let device = app.input_state.devices.get(state.device);
    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();

    if let Some(device) = device
        && let Some(soc) = device.soc
    {
        let soc = (soc * 100.).min(99.) as u32;
        let num_devices = app.input_state.devices.len();
        let suffix = if num_devices > 3 { "" } else { "%" };
        let text = format!("{soc}{suffix}");
        let color = if device.charging {
            state.charging_color
        } else if soc < state.low_threshold {
            state.low_color
        } else {
            state.normal_color
        };
        label.set_color(common, color, false);
        label.set_text(common, Translation::from_raw_text(&text));
        return;
    }
    label.set_text(common, Translation::default());
}

struct ClockLabelState {
    timezone: Option<Tz>,
    format: Rc<str>,
    locale: chrono::Locale,
}

fn clock_on_tick(
    state: &ClockLabelState,
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
) {
    let date_time = state.timezone.as_ref().map_or_else(
        || {
            format!(
                "{}",
                Local::now().format_localized(&state.format, state.locale)
            )
        },
        |tz| {
            format!(
                "{}",
                Local::now()
                    .with_timezone(tz)
                    .format_localized(&state.format, state.locale)
            )
        },
    );

    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();
    label.set_text(common, Translation::from_raw_text(&date_time));
}

struct TimerLabelState {
    start: DateTime<Local>,
    format: Rc<str>,
}

fn timer_on_tick(
    state: &TimerLabelState,
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
) {
    let duration = Local::now()
        .signed_duration_since(state.start)
        .num_seconds();

    let time = &state
        .format
        .replace("%s", &format!("{:02}", (duration % 60)))
        .replace("%m", &format!("{:02}", ((duration / 60) % 60)))
        .replace("%h", &format!("{:02}", ((duration / 60) / 60)));

    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();
    label.set_text(common, Translation::from_raw_text(time));
}

fn ipd_on_tick(
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
    app: &AppState,
) {
    let text = app.input_state.ipd.to_string();
    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();
    label.set_text(common, Translation::from_raw_text(&text));
}

const FPS_GOOD: drawing::Color = drawing::Color::new(0.55, 0.84, 0.55, 1.);
const FPS_WARN: drawing::Color = drawing::Color::new(0.90, 0.78, 0.40, 1.);
const FPS_BAD: drawing::Color = drawing::Color::new(0.88, 0.42, 0.42, 1.);

enum FpsDisplay {
    Fps,
    Reprojection,
}

struct FpsLabelState {
    display: FpsDisplay,
    good_color: WguiColor,
    warn_color: WguiColor,
    bad_color: WguiColor,
}

fn fps_on_tick(
    state: &FpsLabelState,
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
    app: &AppState,
) {
    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();

    let Some(stats) = app.frame_stats else {
        let text = match state.display {
            FpsDisplay::Fps => "— FPS",
            FpsDisplay::Reprojection => "",
        };
        label.set_color(common, state.good_color, true);
        label.set_text(common, Translation::from_raw_text(text));
        return;
    };

    let (text, color) = match state.display {
        FpsDisplay::Fps => {
            let ratio = if stats.display_hz > 0.0 {
                stats.fps / stats.display_hz
            } else {
                1.0
            };
            let color = if ratio >= 0.9 {
                state.good_color
            } else if ratio >= 0.7 {
                state.warn_color
            } else {
                state.bad_color
            };
            (format!("{:.0} FPS", stats.fps), color)
        }
        FpsDisplay::Reprojection => {
            let pct = stats.reprojected_pct;
            let color = if pct <= 10.0 {
                state.good_color
            } else if pct <= 30.0 {
                state.warn_color
            } else {
                state.bad_color
            };
            (format!("репр. {pct:.0}%"), color)
        }
    };
    label.set_color(common, color, true);
    label.set_text(common, Translation::from_raw_text(&text));
}

enum TrackersDisplay {
    Status,
    Override,
    Stance,
    TPose,
    Dots,
}

fn trackers_on_tick(
    display: &TrackersDisplay,
    common: &mut event::CallbackDataCommon,
    data: &mut event::CallbackData,
) {
    let on_off = |v: bool| if v { "ТАК, працює" } else { "НІ, вимкнено" };
    let text = match super::trackers::status() {
        None => match display {
            TrackersDisplay::Status => "Програма калібрування не запущена".to_string(),
            _ => String::new(),
        },
        Some(s) => match display {
            TrackersDisplay::Status => {
                let mut lines = Vec::new();
                if !s.vr {
                    lines.push("Чекаю на SteamVR…".to_string());
                } else if !s.driver {
                    lines.push("Драйвер не працює — перезапусти SteamVR".to_string());
                }
                if s.trackers.is_empty() {
                    lines.push("Ще не відкалібровано. Т-поза: ноги разом, руки в сторони".to_string());
                }
                for t in &s.trackers {
                    lines.push(format!("{} {}", if t.ok { "●" } else { "○" }, t.role));
                }
                lines.join("\n")
            }
            TrackersDisplay::Override => match s.override_left {
                Some(left) => format!("Трекери в Т-позі ще {left} с — калібруйся у VRChat"),
                None => String::new(),
            },
            TrackersDisplay::Stance => format!("{} см між ногами", s.stance_cm),
            TrackersDisplay::TPose => format!("Т-поза — {}", on_off(s.tpose)),
            TrackersDisplay::Dots => format!("Кульки у VR — {}", on_off(s.dots)),
        },
    };
    let label = data.obj.get_as_mut::<WidgetLabel>().unwrap();
    label.set_text(common, Translation::from_raw_text(&text));
}
