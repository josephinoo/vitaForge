#[link(name = "SDL2", kind = "static")]
unsafe extern "C" {}

mod egui_painter;
mod surface;

use crate::app::ui::build_ui;
use crate::app::App;
use crate::input::{
    held_stick_direction, map_controller_button_event, map_keyboard_event, map_pointer_event,
    open_first_controller, register_vita_controller_mapping, AppCommand, TextTarget,
};
use anyhow::Result;
use sdl2::keyboard::Keycode;
use std::thread::sleep;
use std::time::{Duration, Instant};
use surface::{HEIGHT, VitaSurface, WIDTH};

const UI_SCALE: f32 = 1.3;
const TARGET_FRAME_TIME: Duration = Duration::from_millis(16);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(4);
const IDLE_REPAINT_FLOOR: Duration = Duration::from_millis(200);
const STICK_REPEAT_DELAY: Duration = Duration::from_millis(350);
const STICK_REPEAT_INTERVAL: Duration = Duration::from_millis(110);

const IME_OPEN_GRACE: Duration = Duration::from_millis(500);

struct ImeSession {
    opened_at: Instant,
    seen_open: bool,
    text: Option<String>,
    confirmed: bool,
    target: TextTarget,
}

pub fn run(mut app: App) -> Result<()> {
    let sdl = sdl2::init().map_err(anyhow::Error::msg)?;
    let video = sdl.video().map_err(anyhow::Error::msg)?;
    register_vita_controller_mapping(&sdl).map_err(anyhow::Error::msg)?;
    let controllers = sdl.game_controller().map_err(anyhow::Error::msg)?;
    let mut controller = open_first_controller(&controllers);
    let mut event_pump = sdl.event_pump().map_err(anyhow::Error::msg)?;
    let mut surface = VitaSurface::new(&video)?;
    let egui_ctx = egui::Context::default();
    crate::app::ui::apply_theme(&egui_ctx);
    let start_time = Instant::now();
    let mut pointer_pos = egui::Pos2::ZERO;
    let mut ime: Option<ImeSession> = None;
    let mut held_direction = None;
    let mut held_since = Instant::now();
    let mut last_repeat_at = Instant::now();

    loop {
        let loop_started_at = Instant::now();
        let mut egui_events = Vec::new();
        let mut direct_commands = Vec::new();
        let screen_points = (WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE);

        for event in event_pump.poll_iter() {

            if let Some(session) = &mut ime {
                match event {
                    sdl2::event::Event::TextInput { text, .. } => session.text = Some(text),

                    sdl2::event::Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                        session.confirmed = true;
                    }
                    _ => {}
                }
                continue;
            }

            if let Some(command) = map_keyboard_event(&event) {
                direct_commands.push(command);
            }
            if let Some(egui_event) = map_pointer_event(&event, screen_points, UI_SCALE, &mut pointer_pos) {
                egui_events.push(egui_event);
            }
            if let Some(command) = map_controller_button_event(&event) {
                direct_commands.push(command);
            }
            match event {
                sdl2::event::Event::ControllerDeviceAdded { .. } if controller.is_none() => {
                    controller = open_first_controller(&controllers);
                }
                sdl2::event::Event::ControllerDeviceRemoved { .. } => controller = None,
                _ => {}
            }
        }

        if let Some(session) = &mut ime {
            let shown = video.text_input().is_screen_keyboard_shown(surface.window());
            session.seen_open |= shown;

            let reported_result = session.text.is_some() || session.confirmed;
            let finished = if session.seen_open {
                !shown || reported_result
            } else {
                reported_result || session.opened_at.elapsed() >= IME_OPEN_GRACE
            };

            if !finished {
                app.tick(&egui_ctx)?;
                surface.present_snapshot()?;
                continue;
            }

            let session = ime.take().expect("checked above that a session is open");
            video.text_input().stop();

            if session.confirmed && let Some(text) = session.text {
                match session.target {
                    TextTarget::Search => app.handle_command(AppCommand::SetSearchQuery(text))?,
                    TextTarget::Comment => app.handle_command(AppCommand::SubmitComment(text))?,
                }
            }
            match session.target {
                TextTarget::Search => app.handle_command(AppCommand::CloseSearch)?,
                TextTarget::Comment => app.handle_command(AppCommand::CloseCommentEntry)?,
            }

            held_direction = None;
            held_since = Instant::now();
            last_repeat_at = Instant::now();
        }

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

        let _had_direct_commands = !direct_commands.is_empty();
        for command in direct_commands {
            app.handle_command(command)?;
        }
        app.tick(&egui_ctx)?;

        let text_target = match &app.state {
            crate::app::AppState::Catalog(catalog) if catalog.search_requested => Some(TextTarget::Search),
            crate::app::AppState::Detail { comment_entry_requested: true, .. } => Some(TextTarget::Comment),
            _ => None,
        };

        let entering_ime = text_target.is_some();

        let _had_egui_events = !egui_events.is_empty();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE),
            )),
            viewport_id: egui::ViewportId::ROOT,
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo { native_pixels_per_point: Some(UI_SCALE), ..Default::default() },
            ))
            .collect(),
            time: Some(start_time.elapsed().as_secs_f64()),
            predicted_dt: TARGET_FRAME_TIME.as_secs_f32(),
            events: egui_events,
            ..Default::default()
        };

        let mut ui_commands = Vec::new();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            ui_commands = build_ui(ctx, &app);
        });
        app.clear_scroll_to_selected();

        for command in ui_commands {
            app.handle_command(command)?;
        }

        let repaint_after = full_output.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::ZERO);
        let has_texture_delta = !full_output.textures_delta.free.is_empty() || !full_output.textures_delta.set.is_empty();
        let idle = repaint_after > Duration::ZERO
            && !_had_egui_events
            && !_had_direct_commands
            && !has_texture_delta
            // Retries only run while painting, so don't go idle on one.
            && !surface.has_pending_uploads();

        if !idle || entering_ime {
            let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
            if entering_ime {
                surface.snapshot_egui(full_output.pixels_per_point, &clipped_primitives, &full_output.textures_delta)?;
            } else {
                surface.draw_scene();
                surface.paint_egui(full_output.pixels_per_point, &clipped_primitives, &full_output.textures_delta)?;
            }
        }

        if let Some(target) = text_target {
            video.text_input().start();
            ime = Some(ImeSession {
                opened_at: Instant::now(),
                seen_open: false,
                text: None,
                confirmed: false,
                target,
            });
            continue;
        }

        let frame_deadline = if idle {
            loop_started_at + TARGET_FRAME_TIME.max(repaint_after.min(IDLE_REPAINT_FLOOR))
        } else {
            loop_started_at + TARGET_FRAME_TIME
        };

        while Instant::now() < frame_deadline {
            let remaining = frame_deadline.saturating_duration_since(Instant::now());
            sleep(remaining.min(INPUT_POLL_INTERVAL));
            if Instant::now() >= frame_deadline {
                break;
            }
            event_pump.pump_events();
        }
    }
}
