use wayvr_ipc::{packet_client::WvrProcessLaunchParams, packet_server::WvrProcessHandle};

use crate::{
	config::GeneralConfig,
	dash_interface::{self, ConfigChangeKind, DashInterface},
};

/// Stand-in for the live WayVR interface, used by the `uidev` dashboard testbed
pub struct DashInterfaceEmulated {
	general_config: GeneralConfig,
	launched: u64,
}

impl DashInterfaceEmulated {
	pub fn new() -> Self {
		Self {
			// Use serde defaults
			general_config: serde_json::from_str("{}").unwrap(),
			launched: 0,
		}
	}
}

impl Default for DashInterfaceEmulated {
	fn default() -> Self {
		Self::new()
	}
}

impl DashInterface<()> for DashInterfaceEmulated {
	fn process_launch(
		&mut self,
		_: &mut (),
		_: bool,
		_params: WvrProcessLaunchParams,
	) -> anyhow::Result<WvrProcessHandle> {
		self.launched += 1;
		Ok(WvrProcessHandle { user: self.launched })
	}

	fn general_config<'a>(&'a mut self, _: &'a mut ()) -> &'a mut GeneralConfig {
		&mut self.general_config
	}

	fn config_changed(&mut self, _: &mut (), _: ConfigChangeKind) {}

	fn toggle_dashboard(&mut self, _data: &mut ()) {}

	fn get_feats(&mut self, _data: &mut ()) -> dash_interface::InterfaceFeats {
		dash_interface::InterfaceFeats {
			xr_backend: crate::XrBackend::OpenVR,
			desktop_backend: crate::DesktopBackend::Wayland,
			passthru: false,
		}
	}
}
