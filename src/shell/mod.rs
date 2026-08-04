#[link(name = "SDL2", kind = "static")]
unsafe extern "C" {}
// SDL2's accelerated Canvas on Vita is backed by vitaGL; without these three linked in,
// hardware-accelerated rendering can end up in an undefined state once another app (e.g.
// Adrenaline) takes the GPU and this app resumes. Vita-only static libs, not present on host.
#[cfg(target_os = "vita")]
#[link(name = "vitaGL", kind = "static")]
#[link(name = "vita2d", kind = "static")]
#[link(name = "mathneon", kind = "static")]
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
use std::thread::sleep;
use std::time::{Duration, Instant};
use surface::{FramePaintStats, HEIGHT, VitaSurface, WIDTH};
const UI_SCALE: f32 = 1.3;
const ACTIVE_FRAME_TIME: Duration = Duration::from_millis(16);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(4);
// Ceiling on the idle sleep, i.e. how long a fresh SDL event can sit unnoticed before the loop
// wakes to drain it. Was 200ms — that landed directly on the input-latency critical path on
// real hardware, so tightened to 50ms.
const IDLE_REPAINT_FLOOR: Duration = Duration::from_millis(50);
const IDLE_FRAME_FLOOR: Duration = Duration::from_millis(33);
// Tightened from 350ms/110ms — L/R felt sluggish to kick in even at full render speed.
const STICK_REPEAT_DELAY: Duration = Duration::from_millis(200);
const STICK_REPEAT_INTERVAL: Duration = Duration::from_millis(70);
const IME_OPEN_GRACE: Duration = Duration::from_millis(500);
const IME_PAINT_INTERVAL: Duration = Duration::from_millis(250);
const FRAME_STATS_INTERVAL: Duration = Duration::from_secs(2);
const SLOW_FRAME_THRESHOLD: Duration = Duration::from_millis(33);
const FRAME_LOG_DIR: &str = "ux0:data/vitaforge";
// eprintln! goes nowhere on real hardware (no attached console), so mirror it to disk too.
const FRAME_LOG_FILE: &str = "ux0:data/vitaforge/frame_stats.log";
fn log_line(line: &str) {
    eprintln!("{line}");
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(FRAME_LOG_FILE) {
        let _ = writeln!(file, "{line}");
    }
}
const LAST_PHASE_FILE: &str = "ux0:data/vitaforge/last_phase.log";
const PHASE_WRITE_INTERVAL: Duration = Duration::from_secs(1);
thread_local! {
    static LAST_PHASE_WRITE: std::cell::Cell<Option<Instant>> =
        const { std::cell::Cell::new(None) };
}
fn mark_phase(phase: &str) {
    LAST_PHASE_WRITE.with(|last| {
        let now = Instant::now();
        if last.get().is_some_and(|at| now.duration_since(at) < PHASE_WRITE_INTERVAL) {
            return;
        }
        last.set(Some(now));
        let _ = std::fs::write(LAST_PHASE_FILE, phase);
    });
}
const LONG_GAP_THRESHOLD: Duration = Duration::from_millis(40);
#[derive(Default)]
struct FrameStats {
    window_started_at: Option<Instant>,
    frames: u32,
    tick: Duration,
    build_ui: Duration,
    tessellate: Duration,
    texture_apply: Duration,
    geometry: Duration,
    present: Duration,
    draw_calls: u64,
    textures_uploaded: u64,
    vertices_drawn: u64,
    iterations: u32,
    skipped: u32,
    idle: u32,
    commands: u32,
    keyboard_commands: u32,
    controller_button_commands: u32,
    last_painted_at: Option<Instant>,
    max_gap: Duration,
    long_gaps: u32,
    last_repeat_at: Option<Instant>,
    repeats: u32,
    max_repeat_gap: Duration,
}
impl FrameStats {
    fn note_iteration(&mut self) {
        self.iterations += 1;
        self.window_started_at.get_or_insert_with(Instant::now);
    }
    fn note_skip(&mut self) {
        self.skipped += 1;
    }
    fn note_commands(&mut self, count: usize) {
        self.commands += count as u32;
    }
    fn note_keyboard_command(&mut self) {
        self.keyboard_commands += 1;
    }
    fn note_controller_button_command(&mut self) {
        self.controller_button_commands += 1;
    }
    fn note_repeat(&mut self) {
        let now = Instant::now();
        if let Some(previous) = self.last_repeat_at {
            self.max_repeat_gap = self.max_repeat_gap.max(now.duration_since(previous));
        }
        self.last_repeat_at = Some(now);
        self.repeats += 1;
    }
    fn record(&mut self, tick: Duration, build_ui: Duration, tessellate: Duration, paint: FramePaintStats) {
        let now = Instant::now();
        let texture_apply = Duration::from_secs_f64(paint.texture_apply_secs);
        let geometry = Duration::from_secs_f64(paint.geometry_secs);
        let present = Duration::from_secs_f64(paint.present_secs);
        self.frames += 1;
        self.tick += tick;
        self.build_ui += build_ui;
        self.tessellate += tessellate;
        self.texture_apply += texture_apply;
        self.geometry += geometry;
        self.present += present;
        self.draw_calls += paint.draw_calls as u64;
        self.textures_uploaded += paint.textures_uploaded as u64;
        self.vertices_drawn += paint.vertices_drawn as u64;
        let paint_total = texture_apply + geometry + present;
        let total = tick + build_ui + tessellate + paint_total;
        if let Some(previous) = self.last_painted_at {
            let gap = now.duration_since(previous);
            self.max_gap = self.max_gap.max(gap);
            if gap > LONG_GAP_THRESHOLD {
                self.long_gaps += 1;
                log_line(&format!(
                    "long gap: {:.1}ms since last painted frame (work={:.1}ms elsewhere={:.1}ms)",
                    gap.as_secs_f64() * 1000.0,
                    total.as_secs_f64() * 1000.0,
                    gap.saturating_sub(total).as_secs_f64() * 1000.0,
                ));
            }
        }
        self.last_painted_at = Some(now);
        if total > SLOW_FRAME_THRESHOLD {
            log_line(&format!(
                "slow frame: tick={:.1}ms build_ui={:.1}ms tessellate={:.1}ms paint={:.1}ms \
                 (texture_apply={:.1}ms×{} geometry={:.1}ms×{}draws/{}verts present={:.1}ms) total={:.1}ms",
                tick.as_secs_f64() * 1000.0,
                build_ui.as_secs_f64() * 1000.0,
                tessellate.as_secs_f64() * 1000.0,
                paint_total.as_secs_f64() * 1000.0,
                texture_apply.as_secs_f64() * 1000.0,
                paint.textures_uploaded,
                geometry.as_secs_f64() * 1000.0,
                paint.draw_calls,
                paint.vertices_drawn,
                present.as_secs_f64() * 1000.0,
                total.as_secs_f64() * 1000.0,
            ));
        }
    }
    fn maybe_flush(&mut self) {
        let Some(window_started_at) = self.window_started_at else { return };
        let elapsed = window_started_at.elapsed();
        if elapsed < FRAME_STATS_INTERVAL {
            return;
        }
        let seconds = elapsed.as_secs_f64();
        let frames = self.frames.max(1) as f64;
        log_line(&format!(
            "frame stats ({:.1}s): {} painted ({:.1} fps) · {} iterations, {} skipped, {} idle · \
             {} commands [{} keyboard, {} controller-button (one-shot), {} held-direction repeat \
             (stick or D-pad)] (repeat worst gap {:.0}ms) · worst frame gap {:.0}ms, {} over {}ms",
            seconds,
            self.frames,
            self.frames as f64 / seconds,
            self.iterations,
            self.skipped,
            self.idle,
            self.commands,
            self.keyboard_commands,
            self.controller_button_commands,
            self.repeats,
            self.max_repeat_gap.as_secs_f64() * 1000.0,
            self.max_gap.as_secs_f64() * 1000.0,
            self.long_gaps,
            LONG_GAP_THRESHOLD.as_millis(),
        ));
        if self.frames > 0 {
            log_line(&format!(
                "  avg per painted frame: tick={:.2}ms build_ui={:.2}ms tessellate={:.2}ms \
                 texture_apply={:.2}ms ({:.1} uploads) geometry={:.2}ms ({:.1} draws, {:.0} verts) present={:.2}ms",
                self.tick.as_secs_f64() * 1000.0 / frames,
                self.build_ui.as_secs_f64() * 1000.0 / frames,
                self.tessellate.as_secs_f64() * 1000.0 / frames,
                self.texture_apply.as_secs_f64() * 1000.0 / frames,
                self.textures_uploaded as f64 / frames,
                self.geometry.as_secs_f64() * 1000.0 / frames,
                self.draw_calls as f64 / frames,
                self.vertices_drawn as f64 / frames,
                self.present.as_secs_f64() * 1000.0 / frames,
            ));
        }
        let last_painted_at = self.last_painted_at;
        let last_repeat_at = self.last_repeat_at;
        *self = FrameStats { last_painted_at, last_repeat_at, ..FrameStats::default() };
    }
}
struct ImeSession {
    opened_at: Instant,
    seen_open: bool,
    target: TextTarget,
    last_paint: Instant,
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
    let mut next_run_at = Instant::now();
    let mut frame_stats = FrameStats::default();
    let _ = std::fs::create_dir_all(FRAME_LOG_DIR);
    let _ = std::fs::write(FRAME_LOG_FILE, "");
    log_line(&format!("=== vitaforge b{} — new session ===", env!("BUILD_STAMP")));
    mark_phase("run: entering loop");
    loop {
        let loop_started_at = Instant::now();
        frame_stats.note_iteration();
        frame_stats.maybe_flush();
        let mut egui_events = Vec::new();
        let mut direct_commands = Vec::new();
        let mut ime_just_closed = false;
        let screen_points = (WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE);
        for event in event_pump.poll_iter() {
            if ime.is_some() {
                ime::feed_event(&event);
                continue;
            }
            if let Some(command) = map_keyboard_event(&event) {
                direct_commands.push(command);
                frame_stats.note_keyboard_command();
            }
            if let Some(egui_event) = map_pointer_event(&event, screen_points, UI_SCALE, &mut pointer_pos) {
                egui_events.push(egui_event);
            }
            if let Some(command) = map_controller_button_event(&event) {
                direct_commands.push(command);
                frame_stats.note_controller_button_command();
            }
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
        }
        if let Some(session) = &mut ime {
            ime::update();
            let shown = ime::is_shown(&video, surface.window());
            session.seen_open |= shown;
            let finished = if session.seen_open {
                !shown || ime::confirmed()
            } else {
                ime::confirmed() || session.opened_at.elapsed() >= IME_OPEN_GRACE
            };
            if !finished {
                if let Err(err) = app.tick(&egui_ctx) {
                    eprintln!("tick failed while the keyboard was open: {err:#}");
                }
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
            let confirmed = ime::confirmed();
            let text = ime::take_text();
            ime::close(&video);
            ime_just_closed = true;
            if confirmed && let Some(text) = text {
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
                    frame_stats.note_repeat();
                }
            }
            Some(direction) => {
                direct_commands.push(direction.into());
                held_direction = Some(direction);
                held_since = Instant::now();
                last_repeat_at = Instant::now();
                frame_stats.note_repeat();
            }
            None => held_direction = None,
        }
        let _had_direct_commands = !direct_commands.is_empty();
        let _had_egui_events = !egui_events.is_empty();
        frame_stats.note_commands(direct_commands.len());
        for command in direct_commands {
            app.handle_command(command)?;
        }
        let now = Instant::now();
        let should_skip_run = ime.is_none()
            && !ime_just_closed
            && !_had_egui_events
            && !_had_direct_commands
            && now < next_run_at;
        if should_skip_run {
            frame_stats.note_skip();
            let poll_interval = if controller.is_some() {
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
                if held_stick_direction(controller.as_ref()).is_some() {
                    break;
                }
            }
            continue;
        }
        mark_phase("tick");
        let tick_started_at = Instant::now();
        app.tick(&egui_ctx)?;
        let tick_elapsed = tick_started_at.elapsed();
        let text_target = match &app.state {
            crate::app::AppState::Catalog(catalog) if catalog.search_requested => Some(TextTarget::Search),
            crate::app::AppState::Detail { comment_entry_requested: true, .. } => Some(TextTarget::Comment),
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
                egui::ViewportInfo { native_pixels_per_point: Some(UI_SCALE), ..Default::default() },
            ))
            .collect(),
            time: Some(start_time.elapsed().as_secs_f64()),
            predicted_dt: ACTIVE_FRAME_TIME.as_secs_f32(),
            events: egui_events,
            ..Default::default()
        };
        mark_phase("build_ui");
        let build_ui_started_at = Instant::now();
        let mut ui_commands = Vec::new();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            ui_commands = build_ui(ctx, &app);
        });
        let build_ui_elapsed = build_ui_started_at.elapsed();
        app.clear_one_shot_ui_state();
        for command in ui_commands {
            app.handle_command(command)?;
        }
        let repaint_after = full_output.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::ZERO);
        mark_phase("tessellate");
        let tessellate_started_at = Instant::now();
        let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let tessellate_elapsed = tessellate_started_at.elapsed();
        mark_phase("paint: draw_scene");
        surface.draw_scene();
        mark_phase("paint: paint_egui (texture upload / render_geometry / present)");
        let paint_stats: FramePaintStats =
            surface.paint_egui(full_output.pixels_per_point, &clipped_primitives, &full_output.textures_delta)?;
        mark_phase("frame_stats.record");
        frame_stats.record(tick_elapsed, build_ui_elapsed, tessellate_elapsed, paint_stats);
        mark_phase("process_pending_bgdl");
        crate::install::process_pending_bgdl();
        mark_phase("frame complete, back to top of loop");
        if let Some(target) = text_target {
            ime::open(&video, surface.window());
            ime = Some(ImeSession { opened_at: Instant::now(), seen_open: false, target, last_paint: Instant::now() });
            continue;
        }
        let frame_delay = if held_direction.is_some() {
            ACTIVE_FRAME_TIME
        } else if repaint_after > Duration::ZERO {
            IDLE_FRAME_FLOOR.max(repaint_after.min(IDLE_REPAINT_FLOOR))
        } else {
            ACTIVE_FRAME_TIME
        };
        let frame_deadline = loop_started_at + frame_delay;
        next_run_at = frame_deadline;
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
