use crate::windowing::{
    OverlayID,
    backend::{OverlayEventData, OverlayMeta},
    window::OverlayCategory,
};
use slotmap::{Key, SecondaryMap};
use std::rc::Rc;
use wgui::{
    components::button::ComponentButton,
    layout::Layout,
    parser::{Fetchable, ParseDocumentParams, ParserState, TemplateParams},
};

#[derive(Default)]
/// Helper for managing a list of overlays
/// Populates `id="panels_root"` with `<Screen>` templates; mirrors, panels and apps are left out
/// Clears `id="apps_root"` (optional)
/// Uses the following parameters: `name`, `display`
/// Lights up `btn_dashboard`, `btn_keyboard` and `btn_ovl_<name>` while that overlay is shown
pub struct OverlayList {
    overlay_buttons: SecondaryMap<OverlayID, Rc<ComponentButton>>,
    last_overlays: Option<Rc<[OverlayMeta]>>,
    last_visible: Option<Rc<[OverlayID]>>,
}

impl OverlayList {
    /// Fills everything in again from the last known state, after the buttons were rebuilt
    pub fn refresh(
        &mut self,
        layout: &mut Layout,
        parser_state: &mut ParserState,
        doc_params: &ParseDocumentParams,
    ) -> anyhow::Result<()> {
        if let Some(overlays) = self.last_overlays.clone() {
            self.on_notify(
                layout,
                parser_state,
                &OverlayEventData::OverlaysChanged(overlays),
                doc_params,
            )?;
        }
        if let Some(visible) = self.last_visible.clone() {
            self.on_notify(
                layout,
                parser_state,
                &OverlayEventData::VisibleOverlaysChanged(visible),
                doc_params,
            )?;
        }
        Ok(())
    }

    pub fn on_notify(
        &mut self,
        layout: &mut Layout,
        parser_state: &mut ParserState,
        event_data: &OverlayEventData,
        doc_params: &ParseDocumentParams,
    ) -> anyhow::Result<bool> {
        let mut elements_changed = false;
        match event_data {
            OverlayEventData::OverlaysChanged(metas) => {
                self.last_overlays = Some(metas.clone());
                let panels_root = parser_state
                    .get_widget_id("panels_root")
                    .unwrap_or_default();
                let apps_root = parser_state.get_widget_id("apps_root").unwrap_or_default();

                layout.remove_children(panels_root);
                layout.remove_children(apps_root);
                self.overlay_buttons.clear();

                for (i, meta) in metas.iter().enumerate() {
                    let mut params = TemplateParams::new();

                    let (template, root) = match meta.category {
                        OverlayCategory::Screen => {
                            params.insert_rc(
                                "display",
                                format!(
                                    "{}{}",
                                    (*meta.name).chars().next().unwrap_or_default(),
                                    (*meta.name).chars().last().unwrap_or_default()
                                )
                                .into(),
                            );
                            ("Screen", panels_root)
                        }
                        _ => {
                            let key = match meta.category {
                                OverlayCategory::Dashboard => "btn_dashboard".to_string(),
                                OverlayCategory::Keyboard => "btn_keyboard".to_string(),
                                _ => format!("btn_ovl_{}", meta.name),
                            };

                            let Ok(overlay_button) =
                                parser_state.fetch_component_as::<ComponentButton>(&key)
                            else {
                                continue;
                            };

                            if meta.visible {
                                overlay_button.set_sticky_state(&mut layout.common(), true);
                            }
                            self.overlay_buttons.insert(meta.id, overlay_button);
                            continue;
                        }
                    };

                    if root.is_null() {
                        continue;
                    }

                    params.insert_rc("idx", i.to_string().into());
                    params.insert("name", meta.name.as_ref());
                    parser_state
                        .instantiate_template(doc_params, template, layout, root, params)?;
                    let overlay_button = parser_state
                        .fetch_component_as::<ComponentButton>(&format!("overlay_{i}"))?;
                    if meta.visible {
                        overlay_button.set_sticky_state(&mut layout.common(), true);
                    }
                    self.overlay_buttons.insert(meta.id, overlay_button);
                }
                elements_changed = true;
            }
            OverlayEventData::VisibleOverlaysChanged(overlays) => {
                self.last_visible = Some(overlays.clone());
                let mut overlay_buttons = self.overlay_buttons.clone();

                for visible in overlays.as_ref() {
                    if let Some(btn) = overlay_buttons.remove(*visible) {
                        btn.set_sticky_state(&mut layout.common(), true);
                    }
                }

                for btn in overlay_buttons.values() {
                    btn.set_sticky_state(&mut layout.common(), false);
                }
            }
            _ => {}
        }

        Ok(elements_changed)
    }
}
