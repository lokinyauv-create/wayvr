use wlx_common::{config::GeneralConfig, dash_interface::ConfigChangeKind};

pub mod audio_settings;
pub mod color_palettes;
pub mod theme_editor;

pub struct ViewUpdateParams<'a> {
	pub layout: &'a mut wgui::layout::Layout,
	pub general_config: &'a mut GeneralConfig,
	pub config_change_kind: &'a mut Option<ConfigChangeKind>,
}

pub trait ViewTrait {
	fn update(&mut self, par: &mut ViewUpdateParams) -> anyhow::Result<()>;
}
