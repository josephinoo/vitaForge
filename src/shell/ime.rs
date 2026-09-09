#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImePurpose {
    Search,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeResult {
    Confirmed(String),
    Canceled,
}

#[cfg(target_os = "vita")]
mod dialog;
#[cfg(target_os = "vita")]
pub use dialog::{close, feed_event, open, poll};

#[cfg(not(target_os = "vita"))]
pub fn open(
    _video: &sdl2::VideoSubsystem,
    _window: &sdl2::video::Window,
    _purpose: ImePurpose,
    _initial: &str,
) -> bool {
    false
}
#[cfg(not(target_os = "vita"))]
pub fn poll() -> Option<ImeResult> {
    None
}
#[cfg(not(target_os = "vita"))]
pub fn close(_video: &sdl2::VideoSubsystem) {}
#[cfg(not(target_os = "vita"))]
pub fn feed_event(_event: &sdl2::event::Event) {}
