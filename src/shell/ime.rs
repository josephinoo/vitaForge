
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
mod sdl_fallback;
#[cfg(not(target_os = "vita"))]
pub use sdl_fallback::{close, feed_event, open, poll};
