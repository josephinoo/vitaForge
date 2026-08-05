//! Wraps `sceNotificationUtilSendNotification` so an install finishing (or
//! failing) shows up in the system notification tray, not just in-app —
//! this is what makes an in-app install feel like it actually happened
//! instead of vanishing the moment the user navigates away.
//!
//! Two things the header (`psp2/notificationutil.h`) makes explicit and that
//! are easy to get wrong: the text buffer must be exactly 0x410 bytes
//! regardless of the message length, and a truncated string must never split
//! a UTF-16 surrogate pair. `encode_notification` handles both and is plain
//! logic — testable on the host — while the actual FFI call is Vita-only.

/// Byte size the header requires for the notification text buffer.
const TEXT_BYTES: usize = 0x410;
/// `TEXT_BYTES` in `u16` units (the buffer is UTF-16).
const TEXT_UNITS: usize = TEXT_BYTES / 2;

/// Encodes `text` into a fixed `[u16; TEXT_UNITS]` buffer, NUL-terminated by
/// construction (it starts zeroed), truncated to fit without splitting a
/// surrogate pair.
fn encode_notification(text: &str) -> [u16; TEXT_UNITS] {
    let mut buf = [0u16; TEXT_UNITS];
    let max_len = TEXT_UNITS - 1; // always leave room for the NUL terminator

    let mut written = 0usize;
    for unit in text.encode_utf16() {
        if written >= max_len {
            break;
        }
        // A high surrogate needs its partner to be meaningful — if there's
        // only room for the high half, stop one unit early instead of
        // emitting a dangling surrogate.
        let is_high_surrogate = (0xD800..=0xDBFF).contains(&unit);
        if is_high_surrogate && written + 1 >= max_len {
            break;
        }
        buf[written] = unit;
        written += 1;
    }
    buf
}

/// Fire-and-forget: failures are logged, never propagated — a notification
/// failing to show must not fail the install it's reporting on.
pub fn send(text: &str) {
    #[cfg(target_os = "vita")]
    {
        vita::send(text);
    }
    #[cfg(not(target_os = "vita"))]
    {
        let _ = text;
    }
}

pub fn install_finished(title: &str) {
    send(&format!("{title} installed"));
}

pub fn install_failed(title: &str, reason: &str) {
    send(&format!("{title} failed to install: {reason}"));
}

#[cfg(target_os = "vita")]
mod vita {
    use std::sync::Once;

    static LOAD_MODULE: Once = Once::new();

    fn ensure_module_loaded() {
        LOAD_MODULE.call_once(|| unsafe {
            let rc = vitasdk_sys::sceSysmoduleLoadModule(vitasdk_sys::SCE_SYSMODULE_NOTIFICATION_UTIL);
            if rc < 0 {
                eprintln!("couldn't load SceNotificationUtil (0x{rc:08x})");
            }
        });
    }

    pub fn send(text: &str) {
        ensure_module_loaded();
        let buf = super::encode_notification(text);
        let rc = unsafe { vitasdk_sys::sceNotificationUtilSendNotification(buf.as_ptr()) };
        if rc < 0 {
            eprintln!("notification failed (0x{rc:08x}): {text}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_is_always_the_required_size() {
        let buf = encode_notification("hello");
        assert_eq!(buf.len(), TEXT_UNITS);
        assert_eq!(std::mem::size_of_val(&buf), TEXT_BYTES);
    }

    #[test]
    fn short_text_is_nul_terminated() {
        let buf = encode_notification("hi");
        assert_eq!(buf[0], 'h' as u16);
        assert_eq!(buf[1], 'i' as u16);
        assert_eq!(buf[2], 0);
    }

    #[test]
    fn long_text_is_truncated_to_fit_with_room_for_the_nul() {
        let long = "x".repeat(TEXT_UNITS + 100);
        let buf = encode_notification(&long);
        assert_eq!(buf[TEXT_UNITS - 1], 0, "last unit must stay NUL");
        assert_eq!(buf[TEXT_UNITS - 2], 'x' as u16);
    }

    #[test]
    fn truncation_never_splits_a_surrogate_pair() {
        // U+1F600 (😀) encodes as a high/low surrogate pair in UTF-16.
        let emoji = '\u{1F600}';
        let mut text = "x".repeat(TEXT_UNITS - 2);
        text.push(emoji);
        let buf = encode_notification(&text);
        // The emoji doesn't fit (only one unit of room left), so it must be
        // dropped entirely rather than emitting a lone high surrogate.
        let last_nonzero = buf.iter().rposition(|&u| u != 0).unwrap();
        assert!(!(0xD800..=0xDBFF).contains(&buf[last_nonzero]), "must not end on a bare high surrogate");
    }
}
