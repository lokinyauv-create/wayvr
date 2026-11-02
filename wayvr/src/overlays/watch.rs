use crate::{
    gui::{
        panel::{
            GuiPanel, NewGuiPanelParams, apply_custom_command, device_list::DeviceList,
            overlay_list::OverlayList, plugins,
        },
        timer::GuiTimer,
    },
    state::AppState,
    windowing::{Z_ORDER_WATCH, backend::OverlayEventData, window::OverlayWindowConfig},
};
use glam::{Affine3A, Quat, Vec3, vec3};
use std::time::Duration;
use wgui::{
    assets::AssetPathRef,
    components::button::ComponentButton,
    parser::{Fetchable, ParseDocumentParams, TemplateParams},
};
use wlx_common::{
    common::LeftRight,
    watch_buttons::{WatchButton, shown_buttons},
    windowing::{OverlayWindowState, Positioning},
};

pub const WATCH_NAME: &str = "watch";
pub const WATCH_POS: Vec3 = vec3(-0.03, -0.01, 0.125);
pub const WATCH_ROT: Quat = Quat::from_xyzw(-0.707_106_6, 0.000_796_361_8, 0.707_106_6, 0.0);

#[derive(Default)]
struct WatchState {
    device_list: DeviceList,
    overlay_list: OverlayList,
    clock_12h: bool,
    edit_mode: bool,
    /// the buttons in `buttons_root` right now
    buttons: Vec<WatchButton>,
}

/// Built-in watch buttons: element id, action, icon, short name and tooltip
fn builtin_button(id: &str) -> Option<(&'static str, &'static str, &'static str, &'static str, &'static str)> {
    Some(match id {
        "center" => (
            "btn_reset_layout",
            "::ResetLayout",
            "@/watch/center.svg",
            "Центрувати",
            "Поставити монітори й клавіатуру перед собою",
        ),
        "keyboard" => (
            "btn_keyboard",
            "::OverlayToggle kbd",
            "@/watch/keyboard.svg",
            "Клавіатура",
            "Показати або сховати клавіатуру",
        ),
        "trackers" => (
            "btn_ovl_trackers",
            "::OverlayToggle trackers",
            "@/watch/track.svg",
            "Трекери",
            "Калібрування трекерів",
        ),
        "dashboard" => (
            "btn_dashboard",
            "::DashToggle",
            "@/watch/wayvr_dashboard_mono.svg",
            "Налаштування",
            "Кольори, кнопки годинника, вигляд",
        ),
        "edit" => (
            "btn_edit_mode",
            "::EditToggle",
            "@/watch/edit.svg",
            "Редагувати",
            "Режим редагування вікон",
        ),
        _ => return None,
    })
}

/// Fills `buttons_root` with the buttons the user chose, in their order
fn build_buttons(
    panel: &mut GuiPanel<WatchState>,
    app: &mut AppState,
    doc_params: &ParseDocumentParams,
    buttons: Vec<WatchButton>,
) -> anyhow::Result<()> {
    let root = panel.parser_state.get_widget_id("buttons_root")?;
    panel.layout.remove_children(root);

    let plugin_list: Vec<_> = buttons
        .iter()
        .filter_map(|b| Some((b.id.clone(), b.plugin.clone()?)))
        .collect();
    plugins::set_plugins(&plugin_list);

    for (i, button) in buttons.iter().enumerate() {
        let mut params = TemplateParams::new();
        let template = if let Some((elem, press, icon, text, tooltip)) = builtin_button(&button.id) {
            params.insert("id", elem);
            params.insert("press", press);
            params.insert("icon", icon);
            params.insert("text", text);
            params.insert("tooltip", tooltip);
            "Tile"
        } else if let Some(plugin) = &button.plugin {
            params.insert_rc("id", format!("btn_plugin_{i}").into());
            params.insert_rc("press", format!("::PluginPress {}", button.id).into());
            params.insert("text", &plugin.name);
            params.insert(
                "tooltip",
                plugin.tooltip.as_deref().unwrap_or(&plugin.name),
            );
            match plugin.icon_path().filter(|p| p.is_file()) {
                Some(icon) => {
                    params.insert_rc("icon", icon.to_string_lossy().into());
                    "TileFile"
                }
                None => "TileText",
            }
        } else {
            continue;
        };
        panel
            .parser_state
            .instantiate_template(doc_params, template, &mut panel.layout, root, params)?;
    }
    panel.state.buttons = buttons;

    // the new buttons must show what is on right now
    panel.state.overlay_list.refresh(
        &mut panel.layout,
        &mut panel.parser_state,
        doc_params,
    )?;
    if let Ok(btn) = panel
        .parser_state
        .fetch_component_as::<ComponentButton>("btn_edit_mode")
    {
        btn.set_sticky_state(&mut panel.layout.common(), panel.state.edit_mode);
    }
    panel.process_custom_elems(app);
    Ok(())
}

pub fn create_watch(app: &mut AppState) -> anyhow::Result<OverlayWindowConfig> {
    let state = WatchState {
        clock_12h: app.session.config.clock_12h,
        ..Default::default()
    };
    let watch_xml = "gui/watch.xml";

    let mut panel =
        GuiPanel::new_from_template(app, watch_xml, state, NewGuiPanelParams::default())?;

    let doc_params = ParseDocumentParams {
        globals: panel.layout.state.globals.clone(),
        path: AssetPathRef::FileOrBuiltIn(watch_xml),
        extra: panel.doc_extra.take().unwrap_or_default(),
    };

    let buttons = shown_buttons(&app.session.config);
    build_buttons(&mut panel, app, &doc_params, buttons)?;

    panel.on_notify = Some(Box::new({
        let name = WATCH_NAME;
        move |panel, app, event_data| {
            let mut elems_changed = panel.state.overlay_list.on_notify(
                &mut panel.layout,
                &mut panel.parser_state,
                &event_data,
                &doc_params,
            )?;

            elems_changed |= panel.state.device_list.on_notify(
                app,
                &mut panel.layout,
                &mut panel.parser_state,
                &event_data,
                &doc_params,
            )?;

            match event_data {
                OverlayEventData::EditModeChanged(edit_mode) => {
                    panel.state.edit_mode = edit_mode;
                    if let Ok(btn_edit_mode) = panel
                        .parser_state
                        .fetch_component_as::<ComponentButton>("btn_edit_mode")
                    {
                        btn_edit_mode.set_sticky_state(&mut panel.layout.common(), edit_mode);
                    }
                }
                OverlayEventData::SettingsChanged => {
                    panel.layout.mark_redraw();

                    let buttons = shown_buttons(&app.session.config);
                    if buttons != panel.state.buttons {
                        build_buttons(panel, app, &doc_params, buttons)?;
                        elems_changed = true;
                    }

                    if app.session.config.clock_12h != panel.state.clock_12h {
                        panel.state.clock_12h = app.session.config.clock_12h;

                        let clock_root = panel.parser_state.get_widget_id("clock_root")?;
                        panel.layout.remove_children(clock_root);

                        panel.parser_state.instantiate_template(
                            &doc_params,
                            "Clock",
                            &mut panel.layout,
                            clock_root,
                            Default::default(),
                        )?;

                        elems_changed = true;
                    }
                }
                OverlayEventData::CustomCommand { element, command } => {
                    if let Err(e) = apply_custom_command(panel, app, &element, &command) {
                        log::warn!("Could not apply {command:?} on {name}/{element}: {e:?}");
                    } else {
                        elems_changed = true;
                    }
                }
                _ => {}
            }

            if elems_changed {
                panel.process_custom_elems(app);
            }

            Ok(())
        }
    }));

    panel
        .timers
        .push(GuiTimer::new(Duration::from_millis(100), 0));

    let positioning = Positioning::FollowHand {
        hand: LeftRight::Left,
        lerp: 1.0,
    };

    panel.update_layout(app)?;

    Ok(OverlayWindowConfig {
        name: WATCH_NAME.into(),
        z_order: Z_ORDER_WATCH,
        default_state: OverlayWindowState {
            grabbable: false,
            interactable: true,
            positioning,
            transform: Affine3A::from_scale_rotation_translation(
                Vec3::ONE * 0.115,
                WATCH_ROT,
                WATCH_POS,
            ),
            angle_fade: true,
            ..OverlayWindowState::default()
        },
        show_on_spawn: app.session.config.enable_watch,
        global: true,
        ..OverlayWindowConfig::from_backend(Box::new(panel))
    })
}
