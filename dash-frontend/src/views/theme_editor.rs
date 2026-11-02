//! "Свої кольори": mix your own color theme. The user picks five colors (with ready swatches
//! or hue/saturation/brightness sliders); the rest of the palette is mixed from them
//! (`wlx_common::palette::ThemeColors`) and saved as a custom palette file.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use wgui::{
	assets::AssetPathRef,
	color::WguiColor,
	components::{button::ComponentButton, slider::ComponentSlider},
	drawing::{Color, HsvColor},
	globals::WguiGlobals,
	i18n::Translation,
	layout::{Layout, LayoutTask, WidgetID},
	parser::{Fetchable, ParseDocumentParams, ParserState, TemplateParams},
	task::Tasks,
	widget::{label::WidgetLabel, rectangle::WidgetRectangle},
};
use wlx_common::{
	dash_interface::ConfigChangeKind,
	palette::{ThemeColors, load_palette, new_custom_palette_name, save_custom_palette},
};

use crate::{
	frontend::{FrontendTask, FrontendTasks},
	util::popup_manager::{MountPopupOnceParams, PopupHolder},
	views::{ViewTrait, ViewUpdateParams},
};

const SLOTS: [&str; 5] = ["Основний", "Другий", "Третій", "Фон", "Текст"];

/// Ready colors: bright ones for buttons, then dark and light ones for the background and text
const SWATCHES: &[&str] = &[
	"#FF5C8A", "#FF79C6", "#E056FD", "#BD93F9", "#8B7BFF", "#5B8CFF", "#4FC3F7", "#2EE6D6", "#50FA7B", "#A6E22E",
	"#F1FA8C", "#FFB86C", "#FF6E40", "#FF5555", "#11111B", "#1E1E2E", "#282A36", "#2D1B3D", "#1B2B3D", "#3B3B3B",
	"#D9D9E3", "#F8F8F2", "#FFFFFF",
];

const SLIDERS: [(&str, &str, f32); 3] = [
	("sl_h", "Відтінок", 359.0),
	("sl_s", "Насиченість", 100.0),
	("sl_v", "Яскравість", 100.0),
];

#[derive(Clone)]
enum Task {
	SelectSlot(usize),
	SetColor(Color),
	Slider(usize, f32),
	Save { as_new: bool },
}

pub struct Params<'a> {
	pub globals: WguiGlobals,
	pub layout: &'a mut Layout,
	pub parent_id: WidgetID,
	pub frontend_tasks: &'a FrontendTasks,
	pub on_close_request: Box<dyn FnOnce()>,
	pub current_palette: Arc<str>,
}

pub struct View {
	tasks: Tasks<Task>,
	frontend_tasks: FrontendTasks,
	on_close_request: Option<Box<dyn FnOnce()>>,
	parser_state: ParserState,
	colors: ThemeColors,
	slot: usize,
	/// slider values of the color being edited: hue 0..359, saturation and brightness 0..100
	hsv: Rc<RefCell<[f32; 3]>>,
	/// the custom palette being edited, if the current one is custom
	editing: Option<String>,
	slot_buttons: Vec<Rc<ComponentButton>>,
	sliders: Vec<Rc<ComponentSlider>>,
	first_refresh: bool,
}

fn slot_color(colors: &mut ThemeColors, slot: usize) -> &mut Color {
	match slot {
		0 => &mut colors.primary,
		1 => &mut colors.secondary,
		2 => &mut colors.tertiary,
		3 => &mut colors.background,
		_ => &mut colors.text,
	}
}

fn to_sliders(c: Color) -> [f32; 3] {
	let hsv = HsvColor::from(c);
	[(hsv.h * 360.0).round() % 360.0, (hsv.s * 100.0).round(), (hsv.v * 100.0).round()]
}

fn from_sliders(v: [f32; 3]) -> Color {
	HsvColor::new(v[0] / 360.0, v[1] / 100.0, v[2] / 100.0, 1.0).to_rgb()
}

impl ViewTrait for View {
	fn update(&mut self, par: &mut ViewUpdateParams) -> anyhow::Result<()> {
		let mut changed = std::mem::take(&mut self.first_refresh);
		let mut sync_sliders = changed;

		for task in self.tasks.drain() {
			match task {
				Task::SelectSlot(slot) => {
					self.slot = slot;
					sync_sliders = true;
				}
				Task::SetColor(c) => {
					*slot_color(&mut self.colors, self.slot) = c;
					sync_sliders = true;
				}
				Task::Slider(i, value) => {
					let mut hsv = self.hsv.borrow_mut();
					hsv[i] = value;
					*slot_color(&mut self.colors, self.slot) = from_sliders(*hsv);
				}
				Task::Save { as_new } => {
					let name = match (&self.editing, as_new) {
						(Some(name), false) => name.clone(),
						_ => new_custom_palette_name(),
					};
					let palette = self.colors.to_palette();
					if let Err(e) = save_custom_palette(&name, &palette) {
						log::warn!("Could not save theme {name}: {e:?}");
						self.set_status(par.layout, &format!("Не вдалося зберегти: {e}"))?;
						continue;
					}
					// switch to the new theme, like picking it in the palette list
					let mut globals = par.layout.state.globals.get();
					globals.palette = load_palette(&name);
					drop(globals);
					par.general_config.color_palette = name.into();
					par.layout.tasks.push(LayoutTask::RefreshPalette);
					self.frontend_tasks.push(FrontendTask::MountSettings);
					par.config_change_kind.replace(ConfigChangeKind::WguiColorPaletteChange);
					if let Some(c) = self.on_close_request.take() {
						c();
					}
					return Ok(());
				}
			}
			changed = true;
		}

		if sync_sliders {
			let values = to_sliders(*slot_color(&mut self.colors, self.slot));
			*self.hsv.borrow_mut() = values;
			let mut common = par.layout.common();
			for (slider, value) in self.sliders.iter().zip(values) {
				slider.set_value_primary(&mut common, value);
			}
			for (i, btn) in self.slot_buttons.iter().enumerate() {
				btn.set_sticky_state(&mut common, i == self.slot);
			}
			// setting the sliders queued slider tasks with the same values
			drop(common);
			let _ = self.tasks.drain();
		}

		if changed {
			self.refresh_preview(par.layout)?;
		}
		Ok(())
	}
}

impl View {
	pub fn new(params: Params) -> anyhow::Result<Self> {
		let doc_params = &ParseDocumentParams {
			globals: params.globals.clone(),
			path: AssetPathRef::BuiltIn("gui/view/theme_editor.xml"),
			extra: Default::default(),
		};
		let mut parser_state = wgui::parser::parse_from_assets(doc_params, params.layout, params.parent_id)?;
		let tasks = Tasks::new();

		let editing = params
			.current_palette
			.to_ascii_lowercase()
			.ends_with(".json")
			.then(|| params.current_palette.to_string());
		let colors = ThemeColors::from_palette(&load_palette(&params.current_palette));

		let slots_id = parser_state.get_widget_id("slots")?;
		let mut slot_buttons = Vec::new();
		for (idx, text) in SLOTS.iter().enumerate() {
			let mut p = TemplateParams::new();
			p.insert_rc("idx", idx.to_string().into());
			p.insert("text", text);
			parser_state.instantiate_template(doc_params, "SlotButton", params.layout, slots_id, p)?;
			let btn = parser_state.fetch_component_as::<ComponentButton>(&format!("slot_{idx}"))?;
			btn.on_click(Rc::new({
				let tasks = tasks.clone();
				move |_common, _e| {
					tasks.push(Task::SelectSlot(idx));
					Ok(())
				}
			}));
			slot_buttons.push(btn);
		}

		let swatches_id = parser_state.get_widget_id("swatches")?;
		for (idx, hex) in SWATCHES.iter().enumerate() {
			let mut p = TemplateParams::new();
			p.insert_rc("idx", idx.to_string().into());
			p.insert("color", hex);
			parser_state.instantiate_template(doc_params, "Swatch", params.layout, swatches_id, p)?;
			let color = Color::from_hex(hex).unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
			let btn = parser_state.fetch_component_as::<ComponentButton>(&format!("swatch_{idx}"))?;
			btn.on_click(Rc::new({
				let tasks = tasks.clone();
				move |_common, _e| {
					tasks.push(Task::SetColor(color));
					Ok(())
				}
			}));
		}

		let sliders_id = parser_state.get_widget_id("sliders")?;
		let mut sliders = Vec::new();
		for (i, (id, text, max)) in SLIDERS.iter().enumerate() {
			let mut p = TemplateParams::new();
			p.insert("id", id);
			p.insert("text", text);
			p.insert_rc("max", max.to_string().into());
			parser_state.instantiate_template(doc_params, "ColorSlider", params.layout, sliders_id, p)?;
			let slider = parser_state.fetch_component_as::<ComponentSlider>(id)?;
			slider.on_value_changed(Box::new({
				let tasks = tasks.clone();
				move |_common, e| {
					tasks.push(Task::Slider(i, e.value));
				}
			}));
			sliders.push(slider);
		}

		for (id, as_new) in [("btn_save", false), ("btn_save_new", true)] {
			let btn = parser_state.fetch_component_as::<ComponentButton>(id)?;
			btn.on_click(Rc::new({
				let tasks = tasks.clone();
				move |_common, _e| {
					tasks.push(Task::Save { as_new });
					Ok(())
				}
			}));
		}

		let me = Self {
			tasks,
			frontend_tasks: params.frontend_tasks.clone(),
			on_close_request: Some(params.on_close_request),
			parser_state,
			colors,
			slot: 0,
			hsv: Rc::new(RefCell::new([0.0; 3])),
			editing,
			slot_buttons,
			sliders,
			first_refresh: true,
		};

		if let Some(name) = &me.editing {
			let mut common = params.layout.common();
			let mut label = me.parser_state.fetch_widget_as::<WidgetLabel>(common.state, "btn_save_t")?;
			label.set_text(
				&mut common,
				Translation::from_raw_text(&format!("Зберегти в «{}»", &name[..name.len() - 5])),
			);
		}
		Ok(me)
	}

	fn set_status(&self, layout: &mut Layout, text: &str) -> anyhow::Result<()> {
		let mut common = layout.common();
		let mut label = self.parser_state.fetch_widget_as::<WidgetLabel>(common.state, "status")?;
		label.set_text(&mut common, Translation::from_raw_text(text));
		Ok(())
	}

	/// Paints the preview card and the slot circles with the colors being mixed
	fn refresh_preview(&self, layout: &mut Layout) -> anyhow::Result<()> {
		use wgui::color::WguiColorName as N;
		let p = self.colors.to_palette();

		let rects: [(&str, Color, Color); 4] = [
			("pv_bg", p[N::Background], p[N::Outline]),
			("pv_primary", p[N::Primary], p[N::Primary]),
			("pv_secondary", p[N::Secondary], p[N::Secondary]),
			("pv_tertiary", p[N::Tertiary], p[N::Tertiary]),
		];
		for (id, color, border) in rects {
			let mut rect = self.parser_state.fetch_widget_as::<WidgetRectangle>(&layout.state, id)?;
			rect.params.color = WguiColor::Raw(color);
			rect.params.color2 = WguiColor::Raw(color);
			rect.params.border_color = WguiColor::Raw(border);
		}
		let slot_colors = [
			self.colors.primary,
			self.colors.secondary,
			self.colors.tertiary,
			self.colors.background,
			self.colors.text,
		];
		for (i, color) in slot_colors.into_iter().enumerate() {
			let mut rect = self
				.parser_state
				.fetch_widget_as::<WidgetRectangle>(&layout.state, &format!("slot_swatch_{i}"))?;
			rect.params.color = WguiColor::Raw(color);
			rect.params.color2 = WguiColor::Raw(color);
		}

		let labels: [(&str, Color); 5] = [
			("pv_title", p[N::OnBackground]),
			("pv_sub", p[N::OnBackgroundVariant]),
			("pv_primary_t", p[N::OnPrimary]),
			("pv_secondary_t", p[N::OnSecondary]),
			("pv_tertiary_t", p[N::OnTertiary]),
		];
		let mut common = layout.common();
		for (id, color) in labels {
			let mut label = self.parser_state.fetch_widget_as::<WidgetLabel>(common.state, id)?;
			label.set_color(&mut common, WguiColor::Raw(color), true);
		}
		drop(common);
		layout.mark_redraw();
		Ok(())
	}
}

pub fn mount_popup(
	frontend_tasks: FrontendTasks,
	globals: WguiGlobals,
	popup: PopupHolder<View>,
	current_palette: Arc<str>,
) {
	frontend_tasks
		.clone()
		.push(FrontendTask::MountPopupOnce(MountPopupOnceParams::new(
			Translation::from_raw_text("Свої кольори"),
			Box::new(move |data| {
				let on_close_request = popup.get_close_callback(data.layout);
				let view = View::new(Params {
					globals: globals.clone(),
					layout: data.layout,
					parent_id: data.id_content,
					frontend_tasks: &frontend_tasks,
					current_palette,
					on_close_request,
				})?;

				popup.set_view(data.handle, view, None);
				Ok(popup.get_close_callback(data.layout))
			}),
			Default::default(), /* extra */
		)));
}
