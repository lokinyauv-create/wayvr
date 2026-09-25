use std::{
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};

use dash_frontend::frontend::{self, FrontendTask, FrontendUpdateParams};
use glam::{Affine2, Affine3A, Quat, Vec2, vec2, vec3};
use wayvr_ipc::{
    packet_client::WvrProcessLaunchParams,
    packet_server::WvrProcessHandle,
};
use wgui::{
    event::{
        DeviceBitmask, Event as WguiEvent, MouseButtonEvent, MouseButtonIndex, MouseLeaveEvent,
        MouseMotionEvent, MouseWheelEvent,
    },
    gfx::cmd::WGfxClearMode,
    renderer_vk::context::Context as WguiContext,
    widget::EventResult,
};
use wlx_common::{
    dash_interface::{self, ConfigChangeKind, DashInterface},
    locale::WayVRLangProvider,
    overlays::{BackendAttrib, BackendAttribValue},
};
use wlx_common::{
    timestep::Timestep,
    windowing::{OverlayWindowState, Positioning},
};

use crate::{
    backend::{
        input::{Haptics, HoverResult, PointerHit, PointerMode},
        task::{GlobalChange, OverlayTask, TaskType},
        wayvr::process::ProcessHandle,
    },
    config::{none_if_0, save_settings},
    ipc::ipc_server::{gen_args_vec, gen_env_vec},
    state::AppState,
    subsystem::hid::WheelDelta,
    windowing::{
        OverlaySelector, Z_ORDER_DASHBOARD,
        backend::{
            FrameMeta, OverlayBackend, OverlayEventData, RenderResources, ShouldRender,
            ui_transform,
        },
        window::{OverlayCategory, OverlayWindowConfig, realign},
    },
};

pub const DASH_NAME: &str = "Dashboard";

const DASH_RES_U32A: [u32; 2] = [1920, 1080];
const DASH_RES_VEC2: Vec2 = vec2(DASH_RES_U32A[0] as _, DASH_RES_U32A[1] as _);

pub struct DashFrontend {
    inner: frontend::Frontend<AppState>,
    initialized: bool,
    interaction_transform: Option<Affine2>,
    timestep: Timestep,
    has_focus: [bool; 2],
    context: WguiContext,
    tutorial: bool,
}

const GUI_SCALE: f32 = 2.0;

impl DashFrontend {
    fn new(app: &mut AppState) -> anyhow::Result<Self> {
        let mut interface = DashInterfaceLive::new();

        if app.session.no_autostart {
            log::info!("Not starting apps due to --no-autostart");
        } else {
            for p in app.session.config.autostart_apps.clone() {
                let _ = interface.process_launch(app, false, p);
            }
        }

        let tutorial = !app.session.config.tutorial_graduated;

        let frontend = frontend::Frontend::new(frontend::InitParams {
            interface: Box::new(interface),
            lang_provider: &WayVRLangProvider::from_config(&app.session.config),
            theme: app.wgui_theme.clone(),
            color_palette: &app.session.config.color_palette,
            executor: app.executor.clone(),
        })?;

        frontend
            .tasks
            .push(FrontendTask::PlaySound(frontend::SoundType::Startup));

        let context = WguiContext::new(&mut app.wgui_shared, 1.0)?;
        Ok(Self {
            inner: frontend,
            initialized: false,
            interaction_transform: None,
            timestep: Timestep::new(60.0),
            has_focus: [false, false],
            context,
            tutorial,
        })
    }

    fn update(&mut self, app: &mut AppState, timestep_alpha: f32) -> anyhow::Result<()> {
        let res = self.inner.update(FrontendUpdateParams {
            data: app,
            width: DASH_RES_VEC2.x / GUI_SCALE,
            height: DASH_RES_VEC2.y / GUI_SCALE,
            timestep_alpha,
        })?;
        self.inner
            .process_update(res, &mut app.audio_system, &mut app.audio_sample_player)?;
        Ok(())
    }

    fn push_event(&mut self, event: &WguiEvent) -> EventResult {
        match self.inner.layout.push_event(event, &mut (), &mut ()) {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to push event: {e:?}");
                EventResult::NoHit
            }
        }
    }
}

impl OverlayBackend for DashFrontend {
    fn init(&mut self, app: &mut AppState) -> anyhow::Result<()> {
        self.context
            .update_viewport(&mut app.wgui_shared, DASH_RES_U32A, GUI_SCALE)?;
        self.interaction_transform = Some(ui_transform(DASH_RES_U32A));

        if self.inner.layout.content_size.x * self.inner.layout.content_size.y != 0.0 {
            self.update(app, 0.0)?;
            self.initialized = true;
        }
        Ok(())
    }

    fn pause(&mut self, app: &mut AppState) -> anyhow::Result<()> {
        if app.session.config_dirty {
            save_settings(&app.session.config)?;
            app.session.config_dirty = false;
        }

        Ok(())
    }

    fn resume(&mut self, _app: &mut AppState) -> anyhow::Result<()> {
        self.inner.layout.needs_redraw = true;
        self.timestep.reset();
        Ok(())
    }

    fn should_render(&mut self, app: &mut AppState) -> anyhow::Result<ShouldRender> {
        while self.timestep.on_tick() {
            self.inner.layout.tick()?;
        }

        if let Err(e) = self.update(app, self.timestep.alpha) {
            log::error!("uncaught exception: {e:?}");
        }

        Ok(if self.inner.layout.check_toggle_needs_redraw() {
            ShouldRender::Should
        } else {
            ShouldRender::Can
        })
    }

    fn render(&mut self, app: &mut AppState, rdr: &mut RenderResources) -> anyhow::Result<()> {
        let globals = self.inner.layout.state.globals.clone(); // sorry
        let mut globals = globals.get();

        let primitives = wgui::drawing::draw(&mut wgui::drawing::DrawParams {
            globals: &mut globals,
            layout: &mut self.inner.layout,
            debug_draw: false,
            timestep_alpha: self.timestep.alpha,
        })?;
        self.context.draw(
            &globals.font_system,
            &mut app.wgui_shared,
            rdr.cmd_buf_single(),
            &primitives,
        )?;
        Ok(())
    }

    fn frame_meta(&mut self) -> Option<FrameMeta> {
        Some(FrameMeta {
            clear: WGfxClearMode::Clear([0., 0., 0., 0.]),
            extent: [DASH_RES_U32A[0], DASH_RES_U32A[1]],
            ..Default::default()
        })
    }

    fn notify(&mut self, app: &mut AppState, data: OverlayEventData) -> anyhow::Result<()> {
        if !self.tutorial {
            return Ok(());
        }

        // if we're grabbed, stop following the hmd
        if let OverlayEventData::OverlayGrabbed { name, .. } = data
            && &*name == DASH_NAME
        {
            self.tutorial = false;
            app.tasks.enqueue(TaskType::Overlay(OverlayTask::Modify(
                OverlaySelector::Name(name),
                Box::new(|_app, owc| {
                    if let Some(active_state) = owc.active_state.as_mut() {
                        active_state.positioning = Positioning::Floating;
                    }
                }),
            )));
        }

        Ok(())
    }

    fn on_scroll(&mut self, _app: &mut AppState, hit: &PointerHit, delta: WheelDelta) {
        let e = WguiEvent::MouseWheel(MouseWheelEvent {
            delta: vec2(delta.x, delta.y) / 8.0,
            pos: hit.uv * self.inner.layout.content_size,
            device: DeviceBitmask::from_index(hit.pointer),
        });
        self.push_event(&e);
    }

    fn on_hover(&mut self, _app: &mut AppState, hit: &PointerHit) -> HoverResult {
        let e = &WguiEvent::MouseMotion(MouseMotionEvent {
            pos: hit.uv * self.inner.layout.content_size,
            device: DeviceBitmask::from_index(hit.pointer),
        });

        self.has_focus[hit.pointer] = true;

        let result = self.push_event(e);

        HoverResult {
            consume: result != EventResult::NoHit,
            haptics: self
                .inner
                .layout
                .check_toggle_haptics_triggered()
                .then_some(Haptics {
                    intensity: 0.1,
                    duration: 0.01,
                    frequency: 5.0,
                }),
        }
    }

    fn on_left(&mut self, _app: &mut AppState, pointer: usize) {
        let e = WguiEvent::MouseLeave(MouseLeaveEvent {
            device: DeviceBitmask::from_index(pointer),
        });
        self.has_focus[pointer] = false;
        self.push_event(&e);
    }

    fn on_pointer(&mut self, _app: &mut AppState, hit: &PointerHit, pressed: bool) {
        let index = match hit.mode {
            PointerMode::Left => MouseButtonIndex::Left,
            PointerMode::Right => MouseButtonIndex::Right,
            PointerMode::Middle => MouseButtonIndex::Middle,
            _ => return,
        };

        let e = if pressed {
            WguiEvent::MouseDown(MouseButtonEvent {
                pos: hit.uv * self.inner.layout.content_size,
                index,
                device: DeviceBitmask::from_index(hit.pointer),
            })
        } else {
            WguiEvent::MouseUp(MouseButtonEvent {
                pos: hit.uv * self.inner.layout.content_size,
                index,
                device: DeviceBitmask::from_index(hit.pointer),
            })
        };
        self.push_event(&e);

        // released while off-panel → send mouse leave as well
        if !pressed && !self.has_focus[hit.pointer] {
            let e = WguiEvent::MouseMotion(MouseMotionEvent {
                pos: vec2(-1., -1.),
                device: DeviceBitmask::from_index(hit.pointer),
            });
            self.push_event(&e);
            let e = WguiEvent::MouseLeave(MouseLeaveEvent {
                device: DeviceBitmask::from_index(hit.pointer),
            });
            self.push_event(&e);
        }
    }

    fn get_interaction_transform(&mut self) -> Option<Affine2> {
        self.interaction_transform
    }
    fn get_attrib(&self, _attrib: BackendAttrib) -> Option<BackendAttribValue> {
        None
    }
    fn set_attrib(&mut self, _app: &mut AppState, _value: BackendAttribValue) -> bool {
        false
    }
}

fn tutorial_spawn_effect(app: &mut AppState) {
    let dash_name: Arc<str> = DASH_NAME.into();

    app.tasks.enqueue_at(
        TaskType::Overlay(OverlayTask::Modify(
            OverlaySelector::Name(dash_name.clone()),
            Box::new(|app, owc| {
                let hmd = &app.input_state.hmd;
                let pos = hmd.translation + hmd.z_axis * -2.0;
                let mut transform = Affine3A::from_rotation_translation(Quat::IDENTITY, pos.into());
                realign(&mut transform, hmd, 1.0, app.session.config.snap_angle_deg);

                owc.active_state = Some(OverlayWindowState {
                    saved_transform: Some(Affine3A::from_translation(vec3(0., -0.1, -0.9))),
                    transform,
                    grabbable: true,
                    interactable: true,
                    positioning: Positioning::FollowHead { lerp: 0.01 },
                    curvature: none_if_0(app.session.config.default_curvature),
                    alpha: 0.1,
                    ..Default::default()
                });
            }),
        )),
        Instant::now().add(Duration::from_millis(500)),
    );

    // hacky fade in over time
    for i in 0..46 {
        app.tasks.enqueue_at(
            TaskType::Overlay(OverlayTask::Modify(
                OverlaySelector::Name(dash_name.clone()),
                Box::new(|_app, owc| {
                    if let Some(active_state) = owc.active_state.as_mut() {
                        active_state.alpha = (active_state.alpha + 0.025).min(1.0);
                    }
                }),
            )),
            Instant::now().add(Duration::from_millis(500 + 40 * i)),
        );
    }
}

pub fn create_dash_frontend(app: &mut AppState) -> anyhow::Result<OverlayWindowConfig> {
    if !app.session.config.tutorial_graduated {
        tutorial_spawn_effect(app);
    }

    Ok(OverlayWindowConfig {
        name: DASH_NAME.into(),
        default_state: OverlayWindowState {
            transform: Affine3A::from_translation(vec3(0., 0., -0.9)),
            grabbable: true,
            interactable: true,
            positioning: Positioning::Floating,
            curvature: none_if_0(app.session.config.default_curvature),
            ..OverlayWindowState::default()
        },
        z_order: Z_ORDER_DASHBOARD,
        category: OverlayCategory::Dashboard,
        global: true,
        ..OverlayWindowConfig::from_backend(Box::new(DashFrontend::new(app)?))
    })
}
pub struct DashInterfaceLive {}

impl DashInterfaceLive {
    pub const fn new() -> Self {
        Self {}
    }
}

impl DashInterface<AppState> for DashInterfaceLive {
    fn process_launch(
        &mut self,
        app: &mut AppState,
        auto_start: bool,
        params: WvrProcessLaunchParams,
    ) -> anyhow::Result<WvrProcessHandle> {
        let wvr_server = app.wvr_server.as_mut().unwrap();

        let args_vec = gen_args_vec(&params.args);
        let env_vec = gen_env_vec(&params.env);

        if auto_start {
            app.session.config.autostart_apps.push(params.clone());
            save_settings(&app.session.config)?;
        }

        wvr_server
            .spawn_process(
                &params.name,
                &params.exec,
                &args_vec,
                &env_vec,
                params.resolution,
                params.pos_mode,
                None,
                params.icon.as_deref(),
                params.userdata,
            )
            .map(ProcessHandle::as_packet)
    }

    fn general_config<'a>(
        &'a mut self,
        data: &'a mut AppState,
    ) -> &'a mut wlx_common::config::GeneralConfig {
        &mut data.session.config
    }

    fn config_changed(&mut self, data: &mut AppState, kind: ConfigChangeKind) {
        data.session.config_dirty = true;

        match kind {
            ConfigChangeKind::WguiColorPaletteChange => {
                data.tasks
                    .enqueue(TaskType::Overlay(OverlayTask::GlobalChange(
                        GlobalChange::ColorPalette,
                    )));
            }
            ConfigChangeKind::OverlayConfig => {
                data.tasks
                    .enqueue(TaskType::Overlay(OverlayTask::GlobalChange(
                        GlobalChange::Settings,
                    )));
            }
            _ => {}
        }
    }

    fn toggle_dashboard(&mut self, data: &mut AppState) {
        data.tasks
            .enqueue(TaskType::Overlay(OverlayTask::ToggleDashboard));
    }

    fn get_feats(&mut self, data: &mut AppState) -> dash_interface::InterfaceFeats {
        data.feats
    }
}
