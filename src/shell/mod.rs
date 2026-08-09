#[link(name = "SDL2", kind = "static")]
#[link(name = "vitaGL", kind = "static")]
#[link(name = "vita2d", kind = "static")]
#[link(name = "mathneon", kind = "static")]
#[link(name = "SceShaccCg_stub", kind = "static")]
#[link(name = "SceGxm_stub", kind = "static")]
#[link(name = "SceDisplay_stub", kind = "static")]
#[link(name = "SceCtrl_stub", kind = "static")]
#[link(name = "SceAppMgr_stub", kind = "static")]
#[link(name = "SceAppUtil_stub", kind = "static")]
#[link(name = "SceSysmodule_stub", kind = "static")]
#[link(name = "SceCommonDialog_stub", kind = "static")]
#[link(name = "SceIme_stub", kind = "static")]
#[link(name = "taihen_stub", kind = "static")]
unsafe extern "C" {}

mod egui_painter;
mod surface;

use crate::app::ui::build_ui;
use crate::app::App;
use crate::input::{
    held_stick_direction, map_controller_button_event, map_keyboard_event, map_pointer_event,
    open_first_controller, register_vita_controller_mapping, AppCommand,
};
use anyhow::Result;
use sdl2::keyboard::Keycode;
use std::thread::sleep;
use std::time::{Duration, Instant};
use surface::{HEIGHT, VitaSurface, WIDTH};

const UI_SCALE: f32 = 1.3;
const TARGET_FRAME_TIME: Duration = Duration::from_millis(16);
#[allow(dead_code)]
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(4);
const IDLE_REPAINT_FLOOR: Duration = Duration::from_millis(200);
const STICK_REPEAT_DELAY: Duration = Duration::from_millis(150);
const STICK_REPEAT_INTERVAL: Duration = Duration::from_millis(30);


struct ImeSession {
    text: Option<String>,
    confirmed: bool,
    canceled: bool,
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

    let mut last_present = Instant::now();
    #[cfg(target_os = "vita")]
    let mut native_ime: Option<NativeImeSession> = None;

    loop {
        request_display_keepalive();

        let loop_started_at = Instant::now();
        let mut egui_events = Vec::new();
        let mut direct_commands = Vec::new();
        let screen_points = (WIDTH as f32 / UI_SCALE, HEIGHT as f32 / UI_SCALE);

        #[cfg(target_os = "vita")]
        let mut ime_just_closed = false;

        #[cfg(target_os = "vita")]
        if let Some(session) = &mut native_ime {
            if let Some(result) = session.update() {
                native_ime = None;
                ime_just_closed = true;
                if let Some(text) = result {
                    let _ = app.handle_command(AppCommand::SetSearchQuery(text));
                }
                let _ = app.handle_command(AppCommand::CloseSearch);
            }
        }

        for event in event_pump.poll_iter() {
            if let Some(session) = &mut ime {
                match event {
                    sdl2::event::Event::TextInput { text, .. } => {
                        session.text = Some(text);
                        session.confirmed = true;
                    }
                    sdl2::event::Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                        session.confirmed = true;
                    }
                    sdl2::event::Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                        session.canceled = true;
                    }
                    sdl2::event::Event::ControllerButtonDown { button: sdl2::controller::Button::B, .. } => {
                        session.canceled = true;
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

        #[cfg(target_os = "vita")]
        if native_ime.is_some() || ime_just_closed {
            egui_events.clear();
            direct_commands.clear();
            held_direction = None;
        }

        if let Some(session) = &mut ime {
            let finished = session.confirmed || session.canceled || session.text.is_some();

            if !finished {
                app.tick(&egui_ctx)?;
                sleep(TARGET_FRAME_TIME);
                continue;
            }

            let session = ime.take().expect("checked above that a session is open");
            video.text_input().stop();

            if session.confirmed && let Some(text) = session.text {
                app.handle_command(AppCommand::SetSearchQuery(text))?;
            }
            app.handle_command(AppCommand::CloseSearch)?;

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

        let search_requested = matches!(
            &app.state,
            crate::app::AppState::Catalog(catalog) if catalog.search_requested
        );

        let entering_ime = search_requested && ime.is_none();
        #[cfg(target_os = "vita")]
        let entering_ime = entering_ime && native_ime.is_none();

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
            && !has_texture_delta;

        #[cfg(target_os = "vita")]
        let ime_active = native_ime.is_some() || entering_ime;
        #[cfg(not(target_os = "vita"))]
        let ime_active = false;

        let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        surface.draw_scene();
        surface.paint_egui(full_output.pixels_per_point, &clipped_primitives, &full_output.textures_delta, ime_active)?;
        last_present = Instant::now();

        if entering_ime {
            #[cfg(target_os = "vita")]
            {
                native_ime = NativeImeSession::start("Search VitaForge");
            }
            #[cfg(not(target_os = "vita"))]
            {
                video.text_input().start();
                ime = Some(ImeSession {
                    text: None,
                    confirmed: false,
                    canceled: false,
                });
            }
            continue;
        }

        let elapsed = loop_started_at.elapsed();
        if elapsed < TARGET_FRAME_TIME {
            sleep(TARGET_FRAME_TIME - elapsed);
        }
    }
}

fn request_display_keepalive() {
    #[cfg(target_os = "vita")]
    unsafe {
        let _ = vitasdk_sys::scePowerRequestDisplayOn();
    }
}

#[cfg(target_os = "vita")]
struct NativeImeSession {
    _title_u16: Vec<u16>,
    _initial_u16: Vec<u16>,
    buffer_u16: Vec<u16>,
}

#[cfg(target_os = "vita")]
impl NativeImeSession {
    fn start(title: &str) -> Option<Self> {
        use vitasdk_sys::*;
        let _title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let _initial_u16: Vec<u16> = vec![0u16];
        let mut buffer_u16: Vec<u16> = vec![0u16; 256];

        let mut param: SceImeDialogParam = unsafe { std::mem::zeroed() };
        param.sdkVersion = 0x03150000;
        param.supportedLanguages = 0x0001;
        param.dialogMode = 1;
        param.textBoxMode = 0;
        param.title = _title_u16.as_ptr();
        param.maxTextLength = 200;
        param.initialText = _initial_u16.as_ptr() as *mut _;
        param.inputTextBuffer = buffer_u16.as_mut_ptr();

        unsafe {
            let common_ptr = &mut param.commonParam as *mut _;
            param.commonParam.magic = 0xC0D1A109u32.wrapping_add(common_ptr as usize as u32);
            if sceImeDialogInit(&param) < 0 {
                return None;
            }
        }

        Some(Self { _title_u16, _initial_u16, buffer_u16 })
    }

    fn update(&mut self) -> Option<Option<String>> {
        use vitasdk_sys::*;
        unsafe {
            let status = sceImeDialogGetStatus();
            if status == 2 {
                let mut result: SceImeDialogResult = std::mem::zeroed();
                sceImeDialogGetResult(&mut result);
                sceImeDialogTerm();
                if result.button == 2 {
                    let len = self.buffer_u16.iter().position(|&c| c == 0).unwrap_or(self.buffer_u16.len());
                    return Some(String::from_utf16(&self.buffer_u16[..len]).ok());
                } else {
                    return Some(None);
                }
            }
        }
        None
    }
}


