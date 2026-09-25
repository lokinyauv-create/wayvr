pub mod input;

#[cfg(feature = "openvr")]
pub mod openvr;

pub mod wayvr;

pub mod set;

pub mod task;

use thiserror::Error;
use wgui::globals::WguiGlobals;
use wlx_common::config::GeneralConfig;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("backend not supported")]
    NotSupported,
    #[error("Shutdown")]
    #[allow(dead_code)]
    Shutdown,
    #[allow(dead_code)]
    #[error("Restart")]
    Restart,
    #[error("Fatal: {0:?}")]
    Fatal(#[from] anyhow::Error),
}

pub struct RunParams {
    pub wgui_globals: WguiGlobals,
    pub config: GeneralConfig,
}
