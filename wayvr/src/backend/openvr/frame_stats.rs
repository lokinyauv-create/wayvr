use std::time::{Duration, Instant};

use ovr_overlay::{
    TrackedDeviceIndex,
    compositor::CompositorManager,
    sys::ETrackedDeviceProperty,
    system::SystemManager,
};

use crate::state::FrameStats;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Turns the compositor's cumulative frame counters of the scene app into per-second rates.
///
/// `m_nNumFramePresents` counts every present including reprojected ones,
/// so the app's own frame rate is presents minus reprojected.
pub struct FrameStatsSampler {
    next_sample: Instant,
    last: Option<(Instant, Counters)>,
}

#[derive(Clone, Copy)]
struct Counters {
    pid: u32,
    presents: u32,
    reprojected: u32,
}

impl FrameStatsSampler {
    pub fn new() -> Self {
        Self {
            next_sample: Instant::now(),
            last: None,
        }
    }

    /// Returns the new value once per sample interval, `None` in between.
    pub fn update(
        &mut self,
        compositor: &mut CompositorManager,
        system: &mut SystemManager,
    ) -> Option<Option<FrameStats>> {
        let now = Instant::now();
        if now < self.next_sample {
            return None;
        }
        self.next_sample = now + SAMPLE_INTERVAL;

        if compositor.get_current_scene_focus_process() == 0 {
            self.last = None;
            return Some(None);
        }

        let stats = compositor.get_cumulative_stats();
        let current = Counters {
            pid: stats.m_nPid,
            presents: stats.m_nNumFramePresents,
            reprojected: stats.m_nNumReprojectedFrames,
        };
        let Some((last_time, last)) = self.last.replace((now, current)) else {
            return Some(None);
        };
        if last.pid != current.pid {
            return Some(None);
        }

        let secs = now.duration_since(last_time).as_secs_f32();
        let presents = current.presents.wrapping_sub(last.presents);
        let reprojected = current
            .reprojected
            .wrapping_sub(last.reprojected)
            .min(presents);
        if presents == 0 || secs <= 0.0 {
            return Some(None);
        }

        let display_hz = system
            .get_tracked_device_property::<f32>(
                TrackedDeviceIndex::HMD,
                ETrackedDeviceProperty::Prop_DisplayFrequency_Float,
            )
            .unwrap_or(0.0);

        Some(Some(FrameStats {
            fps: (presents - reprojected) as f32 / secs,
            display_hz,
            reprojected_pct: 100.0 * reprojected as f32 / presents as f32,
        }))
    }
}
