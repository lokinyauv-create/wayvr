//! Keyboard that clings to a screen: it slides in the screen's plane and moves along with the screen.

use glam::{Affine3A, Quat, Vec3};
use std::sync::Arc;

use crate::windowing::window::scalar_scale;

/// How far from a screen (in meters) the keyboard still gets pulled in
const RANGE: f32 = 0.45;
/// How close to the "docked under the screen" spot or the screen center the keyboard sticks
const STICKY: f32 = 0.08;
/// Gap between the screen bottom edge and the docked keyboard
pub const DOCK_GAP: f32 = 0.05;
/// How far the keyboard may be dragged below the docked spot
const SLIDE_DOWN: f32 = 0.6;
/// Keyboard floats this far in front of the screen plane, so they do not z-fight
const Z_OFFSET: f32 = 0.02;
pub const TILT_DEG: f32 = -10.0;

pub struct ScreenMagnet {
    pub name: Arc<str>,
    /// Position and rotation of the screen without scale
    pub frame: Affine3A,
    pub width: f32,
    pub height: f32,
}

/// Where the keyboard sits relative to a screen (scale included)
pub struct KeyboardDock {
    pub screen: Arc<str>,
    pub local: Affine3A,
}

pub fn unscaled(t: &Affine3A) -> Affine3A {
    Affine3A::from_cols(
        t.x_axis.normalize(),
        t.y_axis.normalize(),
        t.z_axis.normalize(),
        t.translation,
    )
}

pub fn dock_y(screen_height: f32, kbd_height: f32) -> f32 {
    -(screen_height / 2.0 + kbd_height / 2.0 + DOCK_GAP)
}

/// Pulls a dragged keyboard onto the nearest screen. Returns the new world transform and the dock,
/// or `None` if no screen is close enough (the keyboard then moves freely).
pub fn snap(
    magnets: &[ScreenMagnet],
    kbd: &Affine3A,
    kbd_height: f32,
) -> Option<(Affine3A, KeyboardDock)> {
    let kbd_width = scalar_scale(kbd);

    let (magnet, p) = magnets
        .iter()
        .filter_map(|m| {
            let p = m.frame.inverse().transform_point3a(kbd.translation);
            let dx = (p.x.abs() - m.width / 2.0).max(0.0);
            let dy = (p.y.abs() - m.height / 2.0).max(0.0);
            let dist = (dx * dx + dy * dy + p.z * p.z).sqrt();
            (dist < RANGE).then_some((dist, m, p))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, m, p)| (m, p))?;

    let max_x = ((magnet.width - kbd_width) / 2.0).max(0.0);
    let mut x = p.x.clamp(-max_x, max_x);
    if x.abs() < STICKY {
        x = 0.0;
    }

    let docked = dock_y(magnet.height, kbd_height);
    let mut y = p.y.clamp(docked - SLIDE_DOWN, magnet.height / 2.0 - kbd_height / 2.0);
    if (y - docked).abs() < STICKY {
        y = docked;
    }

    let local = Affine3A::from_scale_rotation_translation(
        Vec3::splat(kbd_width),
        Quat::from_rotation_x(TILT_DEG.to_radians()),
        Vec3::new(x, y, Z_OFFSET),
    );
    let world = magnet.frame * local;
    Some((
        world,
        KeyboardDock {
            screen: magnet.name.clone(),
            local,
        },
    ))
}

/// Keyboard position in the world for a dock, if the screen is visible
pub fn follow(magnets: &[ScreenMagnet], dock: &KeyboardDock) -> Option<Affine3A> {
    magnets
        .iter()
        .find(|m| m.name == dock.screen)
        .map(|m| m.frame * dock.local)
}
