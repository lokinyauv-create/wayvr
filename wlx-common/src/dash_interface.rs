use wayvr_ipc::{packet_client::WvrProcessLaunchParams, packet_server::WvrProcessHandle};

use crate::{DesktopBackend, XrBackend, config::GeneralConfig};

#[derive(Clone, Copy)]
pub struct InterfaceFeats {
	pub xr_backend: XrBackend,
	pub desktop_backend: DesktopBackend,
	pub passthru: bool,
}

impl InterfaceFeats {
	pub fn default_for_backend(xr_backend: XrBackend) -> Self {
		Self {
			xr_backend,
			desktop_backend: DesktopBackend::Headless,
			passthru: false,
		}
	}
}

pub trait DashInterface<T> {
	fn process_launch(
		&mut self,
		data: &mut T,
		auto_start: bool,
		params: WvrProcessLaunchParams,
	) -> anyhow::Result<WvrProcessHandle>;
	fn general_config<'a>(&'a mut self, data: &'a mut T) -> &'a mut GeneralConfig;
	fn config_changed(&mut self, data: &mut T, kind: ConfigChangeKind);
	fn toggle_dashboard(&mut self, data: &mut T);
	fn get_feats(&mut self, data: &mut T) -> InterfaceFeats;
}

#[derive(Default, Clone, Copy)]
pub enum ConfigChangeKind {
	OverlayConfig,
	WguiColorPaletteChange,
	/// Marks the config for saving but doesn't notify any components
	#[default]
	Other,
}

pub type BoxDashInterface<T> = Box<dyn DashInterface<T>>;
