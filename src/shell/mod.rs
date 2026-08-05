#[link(name = "SDL2", kind = "static")]
unsafe extern "C" {}

mod egui_painter;
mod ime;
mod surface;

use crate::app::ui::build_ui;
use crate::app::App;
use crate::input::{
    held_stick_direction, map_controller_button_event, map_keyboard_event, map_pointer_event,
    open_first_controller, register_vita_controller_mapping, AppCommand, TextTarget,
};
use anyhow::Result;
use ime::ImeGuard;
use sdl2::keyboard::Keycode;
use std::thread::sleep;
use std::time::{Duration, Instant};
use surface::{HEIGHT, VitaSurface, WIDTH};

const UI_SCALE: f32 = 1.3;
/// Pace of any frame that actually redraws — active scrolling, hover/press
/// animation, or a texture upload in flight. Must stay 16ms; the idle floor
/// below is a separate knob so raising it can never slow this one down.
const ACTIVE_FRAME_TIME: Duration = Duration::from_millis(16);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(4);
const IDLE_REPAINT_FLOOR: Duration = Duration::from_millis(200);
/// Floor under the idle-branch deadline: even if something asks for a
/// repaint sooner than this, idle frames never bother waking faster than
/// ~30fps for it. Defensive — every current `request_repaint_after` call
/// site already asks for 100ms+, so this floor isn't load-bearing today, but
/// it keeps any future short-interval repaint from silently pinning 60fps
/// while nothing is actually animating.
const IDLE_FRAME_FLOOR: Duration = Duration::from_millis(33);
const STICK_REPEAT_DELAY: Duration = Duration::from_millis(350);
const STICK_REPEAT_INTERVAL: Duration = Duration::from_millis(110);

const IME_OPEN_GRACE: Duration = Duration::from_millis(500);
/// While the on-screen keyboard is up, the backdrop UI is effectively static —
/// repainting it at full 60 fps would just burn battery for no visual gain, so
/// it's throttled to this rate instead.
const IME_PAINT_INTERVAL: Duration = Duration::from_millis(250);

struct ImeSession<'a> {
    opened_at: Instant,
    seen_open: bool,
    text: Option<String>,
    confirmed: bool,
    target: TextTarget,
    last_paint: Instant,
    /// Terminates the common dialog on drop, however this session ends —
    /// including an early return or propagated error from the loop below.
    _guard: ImeGuard<'a>,
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
    let mut ime: Option<ImeSession<'_>> = None;
    let mut held_direction = None;
    let mut held_since = Instant::now();
    let mut last_repeat_at = Instant::now();
    // When the next real tick()+run() is due — due immediately on the first
    // iteration. Carried across loop iterations so a "nothing happened"
    // iteration can skip straight past the layout/tick cost instead of only
    // skipping the paint (see the skip-check below).
    let mut next_run_at = Instant::now();

    loop {
        let loop_started_at = Instant::now();
        let mut egui_events = Vec::new();
        let mut direct_commands = Vec::new();
        let mut ime_just_closed = false;
        let screen_points = (WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE);

        for event in event_pump.poll_iter() {

            if let Some(session) = &mut ime {
                match event {
                    // SDL delivers `SDL_TEXTINPUT` in chunks; overwriting here
                    // would drop everything but the last chunk of a multi-part
                    // IME result, so it accumulates instead.
                    sdl2::event::Event::TextInput { text, .. } => {
                        session.text.get_or_insert_with(String::new).push_str(&text);
                    }
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

            // Text alone must never close the session — SDL can deliver a
            // `TextInput` chunk well before the user presses Return, and
            // treating that as "done" is what made the search box swallow
            // everything after the first character.
            let finished = if session.seen_open {
                !shown || session.confirmed
            } else {
                // The dialog never reported as shown — fall back to a timeout
                // rather than waiting forever.
                session.confirmed || session.opened_at.elapsed() >= IME_OPEN_GRACE
            };

            if !finished {
                // A tick error here must never tear down the frame loop or
                // skip the dialog-termination cleanup below — log and carry on.
                if let Err(err) = app.tick(&egui_ctx) {
                    eprintln!("tick failed while the keyboard was open: {err:#}");
                }

                // Throttled repaint: the backdrop behind the keyboard is
                // static, so there is nothing to gain from redrawing it at
                // full frame rate. No render-target texture is used here —
                // painting straight to the backbuffer avoids fighting the
                // common dialog for the GXM render target, which is the
                // suspected cause of the crash this replaces.
                if session.last_paint.elapsed() >= IME_PAINT_INTERVAL {
                    session.last_paint = Instant::now();
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
                        predicted_dt: ACTIVE_FRAME_TIME.as_secs_f32(),
                        ..Default::default()
                    };
                    let full_output = egui_ctx.run(raw_input, |ctx| {
                        let _ = build_ui(ctx, &app);
                    });
                    let clipped_primitives =
                        egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                    surface.draw_scene();
                    if let Err(err) = surface.paint_egui(
                        full_output.pixels_per_point,
                        &clipped_primitives,
                        &full_output.textures_delta,
                    ) {
                        eprintln!("couldn't paint behind the keyboard: {err:#}");
                    }
                } else {
                    sleep(INPUT_POLL_INTERVAL);
                }
                continue;
            }

            let session = ime.take().expect("checked above that a session is open");
            // `_guard` drops here, terminating the IME dialog. Any error from
            // the command handlers below no longer risks skipping that.
            // The app's state is about to change via the commands below, so
            // the upcoming skip-check must not mistake this for an idle
            // iteration and skip the real run() that needs to pick it up.
            ime_just_closed = true;

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
        let _had_egui_events = !egui_events.is_empty();
        for command in direct_commands {
            app.handle_command(command)?;
        }

        // Nothing arrived and nothing is due yet — skip the tick()/build_ui()
        // layout pass entirely rather than only skipping the paint at the end
        // of it. `ime_just_closed` forces a real run on the one iteration
        // where the IME session just handed commands to `app` directly,
        // outside `direct_commands`/`egui_events`, so this check wouldn't
        // otherwise see that anything happened.
        let now = Instant::now();
        let should_skip_run = ime.is_none()
            && !ime_just_closed
            && !_had_egui_events
            && !_had_direct_commands
            && now < next_run_at;

        if should_skip_run {
            let poll_interval = if held_direction.is_some() {
                INPUT_POLL_INTERVAL
            } else {
                next_run_at.saturating_duration_since(now)
            };
            while Instant::now() < next_run_at {
                let remaining = next_run_at.saturating_duration_since(Instant::now());
                sleep(remaining.min(poll_interval.max(Duration::from_millis(1))));
                if Instant::now() >= next_run_at {
                    break;
                }
                event_pump.pump_events();
            }
            continue;
        }

        app.tick(&egui_ctx)?;

        let text_target = match &app.state {
            crate::app::AppState::Catalog(catalog) if catalog.search_requested => Some(TextTarget::Search),
            crate::app::AppState::Detail { comment_entry_requested: true, .. } => Some(TextTarget::Comment),
            _ => None,
        };

        let entering_ime = text_target.is_some();

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
            predicted_dt: ACTIVE_FRAME_TIME.as_secs_f32(),
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
            surface.draw_scene();
            surface.paint_egui(full_output.pixels_per_point, &clipped_primitives, &full_output.textures_delta)?;
        }

        if let Some(target) = text_target {
            ime = Some(ImeSession {
                opened_at: Instant::now(),
                seen_open: false,
                text: None,
                confirmed: false,
                target,
                last_paint: Instant::now(),
                _guard: ImeGuard::open(&video),
            });
            continue;
        }

        let frame_deadline = if idle {
            loop_started_at + IDLE_FRAME_FLOOR.max(repaint_after.min(IDLE_REPAINT_FLOOR))
        } else {
            loop_started_at + ACTIVE_FRAME_TIME
        };
        next_run_at = frame_deadline;

        // A held stick direction needs to be re-sampled at repeat-interval
        // granularity (`held_stick_direction` polls controller state rather
        // than consuming events), but with nothing held there is nothing to
        // poll for — slicing the wait into 4 ms steps regardless burned
        // ~250 wakeups/second even sitting fully idle. Sleep for the whole
        // remaining budget in one go when idle, and only fall back to the
        // finer slice while a direction is actively held.
        let poll_interval = if held_direction.is_some() { INPUT_POLL_INTERVAL } else { frame_deadline.saturating_duration_since(Instant::now()) };
        while Instant::now() < frame_deadline {
            let remaining = frame_deadline.saturating_duration_since(Instant::now());
            sleep(remaining.min(poll_interval.max(Duration::from_millis(1))));
            if Instant::now() >= frame_deadline {
                break;
            }
            event_pump.pump_events();
        }
    }
}
