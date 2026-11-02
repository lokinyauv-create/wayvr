//! "Кнопки годинника": which buttons the watch shows and in what order.
//! The list itself comes from `wlx_common::watch_buttons` (built-in buttons + the user's own).

use std::{rc::Rc, sync::Arc};

use wgui::{
	components::{button::ComponentButton, checkbox::ComponentCheckbox},
	layout::WidgetID,
	parser::{Fetchable, ParseDocumentParams, ParserState, TemplateParams},
	task::Tasks,
};
use wlx_common::{
	config::GeneralConfig,
	dash_interface::ConfigChangeKind,
	watch_buttons::{all_buttons, is_shown, plugins_dir},
};

use crate::{
	tab::settings::macros::{MacroParams, options_category_text},
	views::ViewUpdateParams,
};

#[derive(Clone)]
enum Task {
	Move(String, isize),
	SetShown(String, bool),
}

pub struct State {
	tasks: Tasks<Task>,
	list_id: WidgetID,
}

impl State {
	pub fn mount(mp: &mut MacroParams, id_parent: WidgetID) -> anyhow::Result<State> {
		let c = options_category_text(mp, id_parent, "Кнопки годинника", "@/dashboard/controller.svg")?;

		let mut params = TemplateParams::new();
		params.insert_str(
			"text",
			format!(
				"Галочка — показувати кнопку, стрілки — пересунути. Свої кнопки (інші програми теж) — файли .json у {}",
				plugins_dir().display()
			),
		);
		mp
			.parser_state
			.instantiate_template(mp.doc_params, "HintText", mp.layout, c, params)?;

		let tasks = Tasks::<Task>::new();
		let list_id = {
			let (pair, _) = mp.layout.add_child(
				c,
				wgui::widget::div::WidgetDiv::create(),
				wgui::taffy::Style {
					flex_direction: wgui::taffy::FlexDirection::Column,
					gap: wgui::taffy::prelude::length(4.0_f32),
					..Default::default()
				},
			)?;
			pair.id
		};

		fill_list(
			mp.layout,
			mp.parser_state,
			mp.doc_params,
			mp.config,
			list_id,
			&tasks,
		)?;

		Ok(State { tasks, list_id })
	}

	pub fn update(
		&mut self,
		par: &mut ViewUpdateParams,
		parser_state: &mut ParserState,
		doc_params: &ParseDocumentParams,
	) -> anyhow::Result<()> {
		let mut changed = false;
		for task in self.tasks.drain() {
			let config = &mut *par.general_config;
			match task {
				Task::Move(id, delta) => {
					// store the full order, so the move is not undone by buttons added later
					let mut order: Vec<Arc<str>> = all_buttons(config).into_iter().map(|b| b.id.into()).collect();
					let Some(pos) = order.iter().position(|b| **b == *id) else {
						continue;
					};
					// skip over hidden buttons: moving should change what the watch shows
					let mut target = pos as isize;
					loop {
						target += delta;
						if target < 0 || target >= order.len() as isize {
							break;
						}
						if is_shown(config, &order[target as usize]) {
							break;
						}
					}
					let target = target.clamp(0, order.len() as isize - 1) as usize;
					let item = order.remove(pos);
					order.insert(target, item);
					config.watch_buttons_order = order;
				}
				Task::SetShown(id, shown) => {
					config.watch_buttons_hidden.retain(|h| **h != *id);
					if !shown {
						config.watch_buttons_hidden.push(id.into());
					}
				}
			}
			changed = true;
		}

		if changed {
			par.layout.remove_children(self.list_id);
			fill_list(
				par.layout,
				parser_state,
				doc_params,
				par.general_config,
				self.list_id,
				&self.tasks,
			)?;
			par.config_change_kind.replace(ConfigChangeKind::OverlayConfig);
		}
		Ok(())
	}
}

fn fill_list(
	layout: &mut wgui::layout::Layout,
	parser_state: &mut ParserState,
	doc_params: &ParseDocumentParams,
	config: &GeneralConfig,
	list_id: WidgetID,
	tasks: &Tasks<Task>,
) -> anyhow::Result<()> {
	for (i, button) in all_buttons(config).into_iter().enumerate() {
		let id = format!("watch_btn_{i}");
		let shown = is_shown(config, &button.id);

		let mut params = TemplateParams::new();
		params.insert("id", &id);
		params.insert("text", &button.title);
		params.insert("checked", if shown { "1" } else { "0" });
		parser_state.instantiate_template(doc_params, "WatchButtonRow", layout, list_id, params)?;

		let checkbox = parser_state.fetch_component_as::<ComponentCheckbox>(&format!("{id}_shown"))?;
		checkbox.on_toggle(Box::new({
			let tasks = tasks.clone();
			let bid = button.id.clone();
			move |_common, e| {
				tasks.push(Task::SetShown(bid.clone(), e.checked));
				Ok(())
			}
		}));

		for (suffix, delta) in [("up", -1), ("down", 1)] {
			let btn = parser_state.fetch_component_as::<ComponentButton>(&format!("{id}_{suffix}"))?;
			btn.on_click(Rc::new({
				let tasks = tasks.clone();
				let bid = button.id.clone();
				move |_common, _e| {
					tasks.push(Task::Move(bid.clone(), delta));
					Ok(())
				}
			}));
		}
	}
	Ok(())
}
