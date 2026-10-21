use crate::{
    gui::{
        panel::{GuiPanel, NewGuiPanelParams},
        timer::GuiTimer,
    },
    state::AppState,
    windowing::{Z_ORDER_WATCH, window::OverlayWindowConfig},
};
use glam::{Affine3A, Vec3};
use std::time::Duration;
use wlx_common::{
    common::LeftRight,
    windowing::{OverlayWindowState, Positioning},
};

use super::watch::{WATCH_POS, WATCH_ROT};

pub const TRACKERS_NAME: &str = "trackers";

/// The panel has the same size and sits in the watch's plane, this far along the forearm
/// (in watch widths) so it looks as if it slides out of the watch.
const SLIDE_OUT: f32 = 1.05;
const WATCH_SCALE: f32 = 0.115;

/// Tracker calibration panel next to the watch; toggled by a button on the watch
pub fn create_trackers(app: &mut AppState) -> anyhow::Result<OverlayWindowConfig> {
    let mut panel = GuiPanel::new_from_template(
        app,
        "gui/trackers.xml",
        (),
        NewGuiPanelParams::default(),
    )?;

    // refreshes the labels (they listen to the internal state change events)
    panel
        .timers
        .push(GuiTimer::new(Duration::from_millis(250), 0));

    panel.update_layout(app)?;

    let offset = WATCH_ROT * Vec3::new(-SLIDE_OUT * WATCH_SCALE, 0.0, 0.0);

    Ok(OverlayWindowConfig {
        name: TRACKERS_NAME.into(),
        z_order: Z_ORDER_WATCH,
        default_state: OverlayWindowState {
            grabbable: false,
            interactable: true,
            positioning: Positioning::FollowHand {
                hand: LeftRight::Left,
                lerp: 1.0,
            },
            transform: Affine3A::from_scale_rotation_translation(
                Vec3::ONE * WATCH_SCALE,
                WATCH_ROT,
                WATCH_POS + offset,
            ),
            angle_fade: true,
            ..OverlayWindowState::default()
        },
        show_on_spawn: false,
        global: true,
        ..OverlayWindowConfig::from_backend(Box::new(panel))
    })
}
