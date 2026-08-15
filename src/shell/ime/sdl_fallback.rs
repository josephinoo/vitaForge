use std::sync::Mutex;

use sdl2::keyboard::{Keycode, Mod};
use super::{ImePurpose, ImeResult};

struct State {
    open: bool,
    text: String,
    confirmed: bool,
    canceled: bool,
}
static STATE: Mutex<State> =
    Mutex::new(State { open: false, text: String::new(), confirmed: false, canceled: false });

pub fn open(
    _video: &sdl2::VideoSubsystem,
    _window: &sdl2::video::Window,
    _purpose: ImePurpose,
    initial: &str,
) -> bool {
    if let Ok(mut state) = STATE.lock() {
        if state.open {
            return false;
        }
        state.open = true;
        state.text = initial.to_owned();
        state.confirmed = false;
        state.canceled = false;
    }
    true
}

pub fn close(_video: &sdl2::VideoSubsystem) {
    if let Ok(mut state) = STATE.lock() {
        state.open = false;
    }
}

fn key_char(key: Keycode, keymod: Mod) -> Option<char> {
    let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    match key {
        Keycode::A => Some(if shift { 'A' } else { 'a' }),
        Keycode::B => Some(if shift { 'B' } else { 'b' }),
        Keycode::C => Some(if shift { 'C' } else { 'c' }),
        Keycode::D => Some(if shift { 'D' } else { 'd' }),
        Keycode::E => Some(if shift { 'E' } else { 'e' }),
        Keycode::F => Some(if shift { 'F' } else { 'f' }),
        Keycode::G => Some(if shift { 'G' } else { 'g' }),
        Keycode::H => Some(if shift { 'H' } else { 'h' }),
        Keycode::I => Some(if shift { 'I' } else { 'i' }),
        Keycode::J => Some(if shift { 'J' } else { 'j' }),
        Keycode::K => Some(if shift { 'K' } else { 'k' }),
        Keycode::L => Some(if shift { 'L' } else { 'l' }),
        Keycode::M => Some(if shift { 'M' } else { 'm' }),
        Keycode::N => Some(if shift { 'N' } else { 'n' }),
        Keycode::O => Some(if shift { 'O' } else { 'o' }),
        Keycode::P => Some(if shift { 'P' } else { 'p' }),
        Keycode::Q => Some(if shift { 'Q' } else { 'q' }),
        Keycode::R => Some(if shift { 'R' } else { 'r' }),
        Keycode::S => Some(if shift { 'S' } else { 's' }),
        Keycode::T => Some(if shift { 'T' } else { 't' }),
        Keycode::U => Some(if shift { 'U' } else { 'u' }),
        Keycode::V => Some(if shift { 'V' } else { 'v' }),
        Keycode::W => Some(if shift { 'W' } else { 'w' }),
        Keycode::X => Some(if shift { 'X' } else { 'x' }),
        Keycode::Y => Some(if shift { 'Y' } else { 'y' }),
        Keycode::Z => Some(if shift { 'Z' } else { 'z' }),
        Keycode::Num0 | Keycode::Kp0 => Some('0'),
        Keycode::Num1 | Keycode::Kp1 => Some('1'),
        Keycode::Num2 | Keycode::Kp2 => Some('2'),
        Keycode::Num3 | Keycode::Kp3 => Some('3'),
        Keycode::Num4 | Keycode::Kp4 => Some('4'),
        Keycode::Num5 | Keycode::Kp5 => Some('5'),
        Keycode::Num6 | Keycode::Kp6 => Some('6'),
        Keycode::Num7 | Keycode::Kp7 => Some('7'),
        Keycode::Num8 | Keycode::Kp8 => Some('8'),
        Keycode::Num9 | Keycode::Kp9 => Some('9'),
        Keycode::Space => Some(' '),
        Keycode::Minus if !shift => Some('-'),
        Keycode::Minus if shift => Some('_'),
        Keycode::Period => Some('.'),
        _ => None,
    }
}

pub fn feed_event(event: &sdl2::event::Event) {
    match event {
        sdl2::event::Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            repeat: false,
            ..
        } => {
            if let Ok(mut state) = STATE.lock() {
                state.text.pop();
            }
        }
        sdl2::event::Event::KeyDown {
            keycode: Some(Keycode::Return) | Some(Keycode::KpEnter),
            repeat: false,
            ..
        } => {
            if let Ok(mut state) = STATE.lock() {
                state.confirmed = true;
            }
        }
        sdl2::event::Event::KeyDown {
            keycode: Some(Keycode::Escape),
            repeat: false,
            ..
        } => {
            if let Ok(mut state) = STATE.lock() {
                state.canceled = true;
            }
        }
        sdl2::event::Event::KeyDown {
            keycode: Some(key),
            keymod,
            repeat: false,
            ..
        } => {
            if let Some(ch) = key_char(*key, *keymod) {
                if let Ok(mut state) = STATE.lock() {
                    state.text.push(ch);
                }
            }
        }
        _ => {}
    }
}

pub fn poll() -> Option<ImeResult> {
    let mut state = STATE.lock().ok()?;
    if !state.open {
        return None;
    }
    if state.confirmed {
        state.open = false;
        Some(ImeResult::Confirmed(std::mem::take(&mut state.text)))
    } else if state.canceled {
        state.open = false;
        Some(ImeResult::Canceled)
    } else {
        None
    }
}
