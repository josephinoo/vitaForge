//! On-screen keyboard, dual implementation behind one API so the frame loop in
//! `shell/mod.rs` never has to branch on target.
//!
//! On real hardware ([`inline`]) this is the Vita's *inline* IME (`sceImeOpen`), which
//! draws its own overlay while the app keeps rendering underneath — unlike the full-screen
#[cfg(target_os = "vita")]
mod inline;
#[cfg(target_os = "vita")]
pub use inline::{close, confirmed, feed_event, is_shown, open, take_text, update};
#[cfg(not(target_os = "vita"))]
mod sdl_fallback;
#[cfg(not(target_os = "vita"))]
pub use sdl_fallback::{close, confirmed, feed_event, is_shown, open, take_text, update};
