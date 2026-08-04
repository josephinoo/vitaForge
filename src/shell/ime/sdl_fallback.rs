//! Host fallback: no libime here, so this keeps using SDL2's own text-input handling,
//! same behavior this project had before the inline-IME split.
use std::sync::Mutex;
struct State {
    text: Option<String>,
    confirmed: bool,
}
static STATE: Mutex<State> = Mutex::new(State { text: None, confirmed: false });
pub fn open(video: &sdl2::VideoSubsystem, _window: &sdl2::video::Window) {
    if let Ok(mut state) = STATE.lock() {
        state.text = None;
        state.confirmed = false;
    }
    video.text_input().start();
}
pub fn close(video: &sdl2::VideoSubsystem) {
    video.text_input().stop();
}
pub fn update() {}
pub fn feed_event(event: &sdl2::event::Event) {
    match event {
        // SDL delivers SDL_TEXTINPUT in chunks; overwriting here would drop everything but
        // the last chunk of a multi-part IME result, so it accumulates instead.
        sdl2::event::Event::TextInput { text, .. } => {
            if let Ok(mut state) = STATE.lock() {
                state.text.get_or_insert_with(String::new).push_str(text);
            }
        }
        sdl2::event::Event::KeyDown { keycode: Some(sdl2::keyboard::Keycode::Return), .. } => {
            if let Ok(mut state) = STATE.lock() {
                state.confirmed = true;
            }
        }
        _ => {}
    }
}
pub fn is_shown(video: &sdl2::VideoSubsystem, window: &sdl2::video::Window) -> bool {
    video.text_input().is_screen_keyboard_shown(window)
}
pub fn confirmed() -> bool {
    STATE.lock().map(|s| s.confirmed).unwrap_or(false)
}
pub fn take_text() -> Option<String> {
    STATE.lock().ok()?.text.take()
}
