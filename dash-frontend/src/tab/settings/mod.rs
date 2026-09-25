use std::{marker::PhantomData, str::FromStr};

use glam::Vec2;
use strum::{AsRefStr, EnumProperty, EnumString};
use wgui::{
	assets::AssetPathRef,
	color::WguiColorName,
	i18n::Translation,
	layout::{Layout, WidgetID},
	parser::{Fetchable, ParseDocumentParams, ParserState},
	renderer_vk::text::{FontWeight, TextStyle, WguiTextShadow},
	taffy::{self, prelude::length},
	task::Tasks,
	widget::{
		div::WidgetDiv,
		label::{WidgetLabel, WidgetLabelParams},
	},
	windowing::context_menu::{self, Blueprint, ContextMenu, TickResult},
};
use wlx_common::{config::GeneralConfig, dash_interface::ConfigChangeKind};

use crate::{
	frontend::{Frontend, FrontendTask, FrontendTasks},
	tab::settings::macros::MacroParams,
	views::ViewUpdateParams,
};

mod macros;
mod tab_look_and_feel;

#[derive(Clone)]
pub(crate) enum Task {
	OpenContextMenu(Vec2, Vec<context_menu::Cell>),
	UpdateBool(SettingType, bool),
	UpdateFloat(SettingType, f32),
}

struct SettingsMountParams<'a> {
	mp: &'a mut MacroParams<'a>,
	frontend_tasks: &'a FrontendTasks,
	id_parent: WidgetID,
}

/// The dashboard's only page: the "look and feel" settings
pub struct TabSettings<T> {
	pub state: ParserState,

	context_menu: ContextMenu,
	look_and_feel: Option<tab_look_and_feel::State>,

	tasks: Tasks<Task>,
	marker: PhantomData<T>,
}

impl<T> TabSettings<T> {
	pub fn new(frontend: &mut Frontend<T>, parent_id: WidgetID, data: &mut T) -> anyhow::Result<Self> {
		let globals = frontend.layout.state.globals.clone();
		let doc_params = doc_params(&globals);

		let mut state = wgui::parser::parse_from_assets(&doc_params, &mut frontend.layout, parent_id)?;
		let tasks = Tasks::default();

		let root = state.get_widget_id("settings_root")?;
		let feats = frontend.interface.get_feats(data);
		let mut mp = MacroParams {
			layout: &mut frontend.layout,
			parser_state: &mut state,
			doc_params: &doc_params,
			config: frontend.interface.general_config(data),
			tasks: tasks.clone(),
			idx: 9001,
			feats,
		};
		let look_and_feel = tab_look_and_feel::State::mount(SettingsMountParams {
			mp: &mut mp,
			id_parent: root,
			frontend_tasks: &frontend.tasks,
		})?;

		Ok(Self {
			tasks,
			state,
			marker: PhantomData,
			context_menu: ContextMenu::default(),
			look_and_feel: Some(look_and_feel),
		})
	}

	pub fn update(&mut self, frontend: &mut Frontend<T>, data: &mut T) -> anyhow::Result<()> {
		if let Some(tab) = &mut self.look_and_feel {
			let mut config_change_kind = None;

			tab.update(&mut ViewUpdateParams {
				layout: &mut frontend.layout,
				general_config: frontend.interface.general_config(data),
				config_change_kind: &mut config_change_kind,
			})?;

			if let Some(kind) = config_change_kind {
				frontend.interface.config_changed(data, kind);
			}
		}

		let mut changed = None;
		for task in self.tasks.drain() {
			match task {
				Task::UpdateBool(setting, n) => {
					if let Some(task) = setting.get_frontend_task() {
						frontend.tasks.push(task)
					}
					let config = frontend.interface.general_config(data);
					*setting.mut_bool(config) = n;
					changed = Some(ConfigChangeKind::OverlayConfig);
				}
				Task::UpdateFloat(setting, n) => {
					if let Some(task) = setting.get_frontend_task() {
						frontend.tasks.push(task)
					}
					let config = frontend.interface.general_config(data);
					*setting.mut_f32(config) = n;
					changed = Some(ConfigChangeKind::OverlayConfig);
				}
				Task::OpenContextMenu(position, cells) => {
					self.context_menu.open(context_menu::OpenParams {
						on_custom_attribs: None,
						position,
						blueprint: Blueprint::Cells(cells),
					});
				}
			}
		}

		// Dropdown handling
		if let TickResult::Action(name) = self.context_menu.tick(&mut frontend.layout, &mut self.state)?
			&& let (Some(setting), Some(id), Some(value), Some(text), Some(translated)) = {
				let mut s = name.splitn(5, ';');
				(s.next(), s.next(), s.next(), s.next(), s.next())
			} {
			let mut common = frontend.layout.common();
			let mut label = self
				.state
				.fetch_widget_as::<WidgetLabel>(common.state, &format!("{id}_value"))?;

			let translation = Translation {
				text: text.into(),
				translated: translated == "1",
			};

			label.set_text(&mut common, translation);

			let setting = SettingType::from_str(setting).expect("Invalid Enum string");
			let config = frontend.interface.general_config(data);
			setting.set_enum(config, value);
			changed = Some(ConfigChangeKind::OverlayConfig);
		}

		// Notify overlays of the change
		if let Some(changed) = changed {
			frontend.interface.config_changed(data, changed);
		}

		Ok(())
	}
}

// Sorted alphabetically
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Eq, PartialEq, AsRefStr, EnumString)]
pub(crate) enum SettingType {
	Clock12h,
	DefaultCurvature,
	DefaultOpacity,
	DefaultOverlayScale,
	DefaultPositioning,
	HideGrabHelp,
	HideUsername,
	Language,
	OpaqueBackground,
	UiAnimationSpeed,
	UiGradientIntensity,
	UiRoundMultiplier,
}

impl SettingType {
	pub fn mut_bool(self, config: &mut GeneralConfig) -> &mut bool {
		match self {
			Self::Clock12h => &mut config.clock_12h,
			Self::HideGrabHelp => &mut config.hide_grab_help,
			Self::HideUsername => &mut config.hide_username,
			Self::OpaqueBackground => &mut config.opaque_background,
			_ => panic!("Requested bool for non-bool SettingType"),
		}
	}

	pub fn mut_f32(self, config: &mut GeneralConfig) -> &mut f32 {
		match self {
			Self::DefaultCurvature => &mut config.default_curvature,
			Self::DefaultOpacity => &mut config.default_opacity,
			Self::DefaultOverlayScale => &mut config.default_overlay_scale,
			Self::UiAnimationSpeed => &mut config.ui_animation_speed,
			Self::UiGradientIntensity => &mut config.ui_gradient_intensity,
			Self::UiRoundMultiplier => &mut config.ui_round_multiplier,
			_ => panic!("Requested f32 for non-f32 SettingType"),
		}
	}

	pub fn set_enum(self, config: &mut GeneralConfig, value: &str) {
		match self {
			Self::DefaultPositioning => {
				config.default_positioning =
					wlx_common::config::DefaultPositioning::from_str(value).expect("Invalid enum value!")
			}
			Self::Language => {
				config.language = Some(wlx_common::locale::Language::from_str(value).expect("Invalid enum value!"))
			}
			_ => panic!("Requested enum for non-enum SettingType"),
		}
	}

	fn get_enum_title(self, config: &mut GeneralConfig) -> Translation {
		match self {
			Self::DefaultPositioning => Self::get_enum_title_inner(config.default_positioning),
			Self::Language => match &config.language {
				Some(lang) => Self::get_enum_title_inner(*lang),
				None => Translation::from_translation_key("APP_SETTINGS.OPTION.AUTO"),
			},
			_ => panic!("Requested enum for non-enum SettingType"),
		}
	}

	fn get_enum_title_inner<E>(value: E) -> Translation
	where
		E: EnumProperty + AsRef<str>,
	{
		value
			.get_str("Translation")
			.map(Translation::from_translation_key)
			.or_else(|| value.get_str("Text").map(Translation::from_raw_text))
			.unwrap_or_else(|| Translation::from_raw_text(value.as_ref()))
	}

	fn get_enum_tooltip_inner<E>(value: E) -> Option<Translation>
	where
		E: EnumProperty + AsRef<str>,
	{
		value.get_str("Tooltip").map(Translation::from_translation_key)
	}

	/// Ok is translation, Err is raw text
	/// `match` sorted alphabetically
	fn get_translation(self) -> Result<&'static str, &'static str> {
		match self {
			Self::Clock12h => Ok("APP_SETTINGS.CLOCK_12H"),
			Self::DefaultCurvature => Ok("APP_SETTINGS.DEFAULT_CURVATURE"),
			Self::DefaultOpacity => Ok("APP_SETTINGS.DEFAULT_OPACITY"),
			Self::DefaultOverlayScale => Ok("APP_SETTINGS.DEFAULT_OVERLAY_SCALE"),
			Self::DefaultPositioning => Ok("APP_SETTINGS.DEFAULT_POSITIONING"),
			Self::HideGrabHelp => Ok("APP_SETTINGS.HIDE_GRAB_HELP"),
			Self::HideUsername => Ok("APP_SETTINGS.HIDE_USERNAME"),
			Self::Language => Ok("APP_SETTINGS.LANGUAGE"),
			Self::OpaqueBackground => Ok("APP_SETTINGS.OPAQUE_BACKGROUND"),
			Self::UiAnimationSpeed => Ok("APP_SETTINGS.ANIMATION_SPEED"),
			Self::UiGradientIntensity => Ok("APP_SETTINGS.UI_GRADIENT_INTENSITY"),
			Self::UiRoundMultiplier => Ok("APP_SETTINGS.ROUND_MULTIPLIER"),
		}
	}

	/// `match` sorted alphabetically
	fn get_tooltip(self) -> Option<&'static str> {
		match self {
			Self::DefaultCurvature => Some("APP_SETTINGS.OVERLAY_DEFAULTS_HELP"),
			Self::DefaultOpacity => Some("APP_SETTINGS.OVERLAY_DEFAULTS_HELP"),
			Self::DefaultOverlayScale => Some("APP_SETTINGS.DEFAULT_OVERLAY_SCALE_HELP"),
			Self::DefaultPositioning => Some("APP_SETTINGS.OVERLAY_DEFAULTS_HELP"),
			_ => None,
		}
	}

	fn requires_restart(self) -> bool {
		matches!(
			self,
			Self::UiAnimationSpeed | Self::UiRoundMultiplier | Self::UiGradientIntensity | Self::Language
		)
	}

	fn get_frontend_task(self) -> Option<FrontendTask> {
		match self {
			Self::Clock12h => Some(FrontendTask::RefreshClock),
			Self::OpaqueBackground => Some(FrontendTask::RefreshBackground),
			_ => None,
		}
	}
}

// creates a simple div with horizontal, centered flow
pub fn horiz_cell(layout: &mut Layout, parent: WidgetID) -> anyhow::Result<WidgetID> {
	let (pair, _) = layout.add_child(
		parent,
		WidgetDiv::create(),
		taffy::Style {
			flex_direction: taffy::FlexDirection::Row,
			align_items: Some(taffy::AlignItems::CENTER),
			gap: length(8.0_f32),
			..Default::default()
		},
	)?;

	Ok(pair.id)
}

fn mount_requires_restart(layout: &mut Layout, parent: WidgetID) -> anyhow::Result<()> {
	let content = Translation::from_translation_key("APP_SETTINGS.REQUIRES_RESTART");
	let label = WidgetLabel::create(
		&mut layout.state,
		WidgetLabelParams {
			content,
			style: TextStyle {
				wrap: false,
				color: Some(WguiColorName::Danger.into()),
				weight: Some(FontWeight::Bold),
				size: Some(10.0),
				shadow: Some(WguiTextShadow {
					x: 2.0,
					y: 2.0,
					..Default::default()
				}),
				..Default::default()
			},
			..Default::default()
		},
	);

	layout.add_child(parent, label, Default::default())?;
	Ok(())
}

fn doc_params(globals: &'_ wgui::globals::WguiGlobals) -> ParseDocumentParams<'_> {
	ParseDocumentParams {
		globals: globals.clone(),
		path: AssetPathRef::BuiltIn("gui/tab/settings.xml"),
		extra: Default::default(),
	}
}
