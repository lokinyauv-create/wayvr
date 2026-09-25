use ovr_overlay::{compositor::CompositorManager, sys::ETrackingUniverseOrigin};

/// Follows which tracking universe (standing/seated) SteamVR uses.
///
/// This build never writes the SteamVR zone (chaperone): a left trackpad press in games
/// used to start space drags that rewrote the zone every frame, so dragging, rotating,
/// recentering and floor fixing were removed.
pub(super) struct Playspace {
    universe: ETrackingUniverseOrigin,
}

impl Playspace {
    pub const fn new() -> Self {
        Self {
            universe: ETrackingUniverseOrigin::TrackingUniverseRawAndUncalibrated,
        }
    }

    pub fn playspace_changed(&mut self, compositor_mgr: &mut CompositorManager) {
        let new_universe = compositor_mgr.get_tracking_space();
        if new_universe != self.universe {
            log::info!(
                "Playspace changed: {} -> {}",
                universe_str(&self.universe),
                universe_str(&new_universe)
            );
            self.universe = new_universe;
        }
    }

    pub fn get_universe(&self) -> ETrackingUniverseOrigin {
        self.universe.clone()
    }
}

const fn universe_str(universe: &ETrackingUniverseOrigin) -> &'static str {
    match universe {
        ETrackingUniverseOrigin::TrackingUniverseSeated => "Seated",
        ETrackingUniverseOrigin::TrackingUniverseStanding => "Standing",
        ETrackingUniverseOrigin::TrackingUniverseRawAndUncalibrated => "Raw",
    }
}
