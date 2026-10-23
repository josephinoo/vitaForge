#[link(name = "SDL2", kind = "static")]
unsafe extern "C" {}
#[cfg(target_os = "vita")]
#[link(name = "vitaGL", kind = "static")]
#[link(name = "vitashark", kind = "static")]
#[link(name = "SceShaccCgExt", kind = "static")]
#[link(name = "SceShaccCg_stub")]
#[link(name = "taihen_stub")]
#[link(name = "SceKernelDmacMgr_stub")]
#[link(name = "vita2d", kind = "static")]
#[link(name = "mathneon", kind = "static")]
#[link(name = "stdc++")]
unsafe extern "C" {}
#[cfg(target_os = "vita")]
mod vgl;
#[cfg(target_os = "vita")]
pub(crate) mod vgl_splash_stub;
#[cfg(target_os = "vita")]
mod gl_egui_painter;
mod egui_painter;
mod ime;
mod surface;
use crate::app::App;
use crate::app::ui::build_ui;
use crate::input::{
    AppCommand, Pointer, TextTarget, held_stick_direction, map_controller_button_event,
    map_keyboard_event, open_first_controller, register_vita_controller_mapping,
};
use anyhow::Result;
use std::time::{Duration, Instant};
use surface::{FramePaintStats, HEIGHT, VitaSurface, WIDTH};
const UI_SCALE: f32 = 1.3;
const ACTIVE_FRAME_TIME: Duration = Duration::from_millis(16);
const STICK_REPEAT_DELAY: Duration = Duration::from_millis(200);
const STICK_REPEAT_INTERVAL: Duration = Duration::from_millis(70);
struct ImeSession {
    target: TextTarget,
    query_before: String,
}
pub fn run(mut app: App) -> Result<()> {
    crate::install::log_file("boot: SDL init");
    let sdl = sdl2::init().map_err(anyhow::Error::msg)?;
    crate::install::log_file("boot: SDL video init");
    let video = sdl.video().map_err(anyhow::Error::msg)?;
    register_vita_controller_mapping(&sdl).map_err(anyhow::Error::msg)?;
    let controllers = sdl.game_controller().map_err(anyhow::Error::msg)?;
    let mut controller = open_first_controller(&controllers);
    let mut event_pump = sdl.event_pump().map_err(anyhow::Error::msg)?;
    crate::install::log_file("boot: surface init");
    let mut surface = VitaSurface::new(&video)?;
    crate::install::log_file("boot: surface ready");
    let egui_ctx = egui::Context::default();
    crate::app::ui::apply_theme(&egui_ctx);
    let start_time = Instant::now();
    let mut pointer = Pointer::new();
    let mut ime: Option<ImeSession> = None;
    let mut held_direction = None;
    let mut held_since = Instant::now();
    let mut last_repeat_at = Instant::now();
    let mut last_state_kind = std::mem::discriminant(&app.state);
    let mut transition_started_at = Instant::now();
    const SCREEN_FADE_SECS: f32 = 0.15;
    let mut stalled_frames = 0u32;
    loop {
        let mut egui_events = Vec::new();
        pointer.deferred(&mut egui_events);
        let mut direct_commands = Vec::new();
        let screen_points = (WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE);
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. }
                | sdl2::event::Event::AppWillEnterBackground { .. }
                | sdl2::event::Event::AppDidEnterBackground { .. } => {
                    #[cfg(target_os = "vita")]
                    unsafe {
                        vitasdk_sys::sceKernelExitProcess(0);
                    }
                    #[cfg(not(target_os = "vita"))]
                    return Ok(());
                }
                sdl2::event::Event::ControllerDeviceAdded { .. } if controller.is_none() => {
                    controller = open_first_controller(&controllers);
                }
                sdl2::event::Event::ControllerDeviceRemoved { .. } => controller = None,
                _ => {}
            }
            if ime.is_some() {
                ime::feed_event(&event);
                continue;
            }
            if let Some(command) = map_keyboard_event(&event) {
                direct_commands.push(command);
            }
            pointer.map_event(&event, screen_points, UI_SCALE, &mut egui_events);
            if let Some(command) = map_controller_button_event(&event) {
                direct_commands.push(command);
            }
        }
        if ime.is_some() {
            if let Some(result) = ime::poll() {
                let session = ime.take().expect("IME session was open");
                ime::close(&video);
                match result {
                    ime::ImeResult::Confirmed(text) => match session.target {
                        TextTarget::Search => {
                            app.handle_command(AppCommand::SetSearchQuery(text))?
                        }
                        TextTarget::Comment => {
                            if !text.is_empty() {
                                app.handle_command(AppCommand::SubmitComment(text))?;
                            }
                        }
                    },
                    ime::ImeResult::Canceled if session.target == TextTarget::Search => {
                        app.handle_command(AppCommand::SetSearchQuery(session.query_before))?;
                    }
                    ime::ImeResult::Canceled => {}
                }
                match session.target {
                    TextTarget::Search => app.handle_command(AppCommand::CloseSearch)?,
                    TextTarget::Comment => app.handle_command(AppCommand::CloseCommentEntry)?,
                }
                held_direction = None;
                held_since = Instant::now();
                last_repeat_at = Instant::now();
            }
        }
        let current_state_kind = std::mem::discriminant(&app.state);
        if current_state_kind != last_state_kind {
            last_state_kind = current_state_kind;
            transition_started_at = Instant::now();
            held_direction = None;
            held_since = Instant::now();
            last_repeat_at = Instant::now();
        }
        let fade_t = (transition_started_at.elapsed().as_secs_f32() / SCREEN_FADE_SECS)
            .clamp(0.0, 1.0);
        let fade_alpha = fade_t * fade_t * (3.0 - 2.0 * fade_t);
        surface.set_frame_alpha(0.82 + 0.18 * fade_alpha);
        if ime.is_none() {
            match held_stick_direction(controller.as_ref()) {
                Some(direction) if held_direction == Some(direction) => {
                    if held_since.elapsed() >= STICK_REPEAT_DELAY
                        && last_repeat_at.elapsed() >= STICK_REPEAT_INTERVAL
                    {
                        direct_commands.push(direction.into());
                        last_repeat_at = Instant::now();
                    }
                }
                Some(direction) => {
                    direct_commands.push(direction.into());
                    held_direction = Some(direction);
                    held_since = Instant::now();
                    last_repeat_at = Instant::now();
                }
                None => held_direction = None,
            }
        }
        for command in direct_commands {
            app.handle_command(command)?;
        }
        let pending_gpu = surface.pending_texture_uploads();
        app.icons.set_gpu_backlog(pending_gpu);
        app.tick(&egui_ctx)?;
        let text_target = match &app.state {
            crate::app::AppState::Catalog(catalog) if catalog.search_requested => {
                Some(TextTarget::Search)
            }
            crate::app::AppState::Detail {
                comment_entry_requested: true,
                ..
            } => Some(TextTarget::Comment),
            _ => None,
        };
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE),
            )),
            viewport_id: egui::ViewportId::ROOT,
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(UI_SCALE),
                    ..Default::default()
                },
            ))
            .collect(),
            time: Some(start_time.elapsed().as_secs_f64()),
            predicted_dt: ACTIVE_FRAME_TIME.as_secs_f32(),
            events: egui_events,
            ..Default::default()
        };
        let mut ui_commands = Vec::new();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            ui_commands = build_ui(ctx, &app);
        });
        app.clear_one_shot_ui_state();
        for command in ui_commands {
            app.handle_command(command)?;
        }
        let clipped_primitives =
            egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        surface.set_common_dialog(ime.is_some());
        surface.draw_scene();
        let paint_stats: FramePaintStats = surface.paint_egui(
            full_output.pixels_per_point,
            &clipped_primitives,
            &full_output.textures_delta,
        )?;
        app.icons.set_gpu_backlog(surface.pending_texture_uploads());
        let mut dropped = surface.take_dropped_textures();
        if paint_stats.draw_calls == 0 && paint_stats.missing_textures == 0 {
            stalled_frames += 1;
        } else {
            stalled_frames = 0;
        }
        if stalled_frames >= 30 {
            stalled_frames = 0;
            dropped.extend(surface.force_recover());
            egui_ctx.request_repaint();
        }
        if !dropped.is_empty() {
            app.icons.forget_textures(&egui_ctx, &dropped);
            egui_ctx.request_repaint();
        }
        crate::install::process_pending_bgdl();
        #[cfg(target_os = "vita")]
        unsafe {
            vitasdk_sys::sceKernelPowerTick(vitasdk_sys::SCE_KERNEL_POWER_TICK_DEFAULT);
        }
        if ime.is_none()
            && let Some(target) = text_target
        {
            let purpose = match target {
                TextTarget::Search => ime::ImePurpose::Search,
                TextTarget::Comment => ime::ImePurpose::Comment,
            };
            let query_before = match &app.state {
                crate::app::AppState::Catalog(catalog) => catalog.search_query.clone(),
                _ => String::new(),
            };
            let initial = match target {
                TextTarget::Search => query_before.as_str(),
                TextTarget::Comment => "",
            };
            if ime::open(&video, surface.window(), purpose, initial) {
                ime = Some(ImeSession {
                    target,
                    query_before,
                });
                pointer.forget_touch(); // the keyboard eats the events that would have ended this touch
            } else {
                match target {
                    TextTarget::Search => app.handle_command(AppCommand::CloseSearch)?,
                    TextTarget::Comment => app.handle_command(AppCommand::CloseCommentEntry)?,
                }
            }
        }
    }
}
