
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

mod dialog;
pub use dialog::{close, feed_event, open, poll};
