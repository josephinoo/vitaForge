use std::sync::Mutex;
use vitasdk_sys::*;

use super::{ImePurpose, ImeResult};

const PSP2_SDK_VERSION: SceUInt32 = 0x0357_0011;

struct DialogState {
    active: bool,
    buffer: Vec<u16>,
    title: Vec<u16>,
}

static STATE: Mutex<DialogState> = Mutex::new(DialogState {
    active: false,
    buffer: Vec::new(),
    title: Vec::new(),
});

fn to_utf16_null_terminated(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_utf16_null_terminated(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn purpose_title(purpose: ImePurpose) -> &'static str {
    match purpose {
        ImePurpose::Search => "Search",
        ImePurpose::Comment => "Comment",
    }
}

fn purpose_max_len(purpose: ImePurpose) -> usize {
    match purpose {
        ImePurpose::Search => 64,
        ImePurpose::Comment => 256,
    }
}

pub fn open(
    _video: &sdl2::VideoSubsystem,
    _window: &sdl2::video::Window,
    purpose: ImePurpose,
    initial: &str,
) -> bool {
    let Ok(mut state) = STATE.lock() else {
        return false;
    };
    if state.active {
        return false;
    }

    let max_len = purpose_max_len(purpose);
    state.title = to_utf16_null_terminated(purpose_title(purpose));
    state.buffer = vec![0u16; max_len + 1];
    for (slot, ch) in state.buffer.iter_mut().zip(initial.encode_utf16().take(max_len)) {
        *slot = ch;
    }
    if let Some(last) = state.buffer.last_mut() {
        *last = 0;
    }

    let enter_label = match purpose {
        ImePurpose::Search => SCE_IME_ENTER_LABEL_SEARCH,
        ImePurpose::Comment => SCE_IME_ENTER_LABEL_SEND,
    };

    let result = unsafe {
        let _ = sceSysmoduleLoadModule(SCE_SYSMODULE_IME);
        let mut param: SceImeDialogParam = core::mem::zeroed();
        let common_ptr = &raw mut param.commonParam;
        param.commonParam.magic =
            SCE_COMMON_DIALOG_MAGIC_NUMBER.wrapping_add(common_ptr as usize as u32);
        param.sdkVersion = PSP2_SDK_VERSION;
        param.title = state.title.as_ptr();
        param.maxTextLength = max_len as u32;
        param.initialText = state.buffer.as_mut_ptr();
        param.inputTextBuffer = state.buffer.as_mut_ptr();
        param.type_ = SCE_IME_TYPE_BASIC_LATIN as u32;
        param.supportedLanguages =
            u64::from(SCE_IME_LANGUAGE_ENGLISH | SCE_IME_LANGUAGE_SPANISH);
        param.languagesForced = 0;
        param.dialogMode = SCE_IME_DIALOG_DIALOG_MODE_WITH_CANCEL;
        param.textBoxMode = SCE_IME_DIALOG_TEXTBOX_MODE_WITH_CLEAR;
        param.enterLabel = enter_label as SceUChar8;
        sceImeDialogInit(&param)
    };
    if result < 0 {
        eprintln!("couldn't open the native keyboard: {result:#010x}");
        state.active = false;
        return false;
    }
    state.active = true;
    true
}

pub fn poll() -> Option<ImeResult> {
    let Ok(mut state) = STATE.lock() else {
        return None;
    };
    if !state.active {
        return None;
    }
    unsafe {
        let status = sceImeDialogGetStatus();
        match status as u32 {
            SCE_COMMON_DIALOG_STATUS_RUNNING => None,
            SCE_COMMON_DIALOG_STATUS_FINISHED => {
                let mut result: SceImeDialogResult = core::mem::zeroed();
                let res = sceImeDialogGetResult(&mut result);
                let _ = sceImeDialogTerm();
                state.active = false;
                if res >= 0 && result.button as u32 == SCE_IME_DIALOG_BUTTON_ENTER as u32 {
                    Some(ImeResult::Confirmed(from_utf16_null_terminated(&state.buffer)))
                } else {
                    Some(ImeResult::Canceled)
                }
            }
            _ => {
                let _ = sceImeDialogTerm();
                state.active = false;
                Some(ImeResult::Canceled)
            }
        }
    }
}

pub fn close(_video: &sdl2::VideoSubsystem) {
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    if state.active {
        unsafe {
            let _ = sceImeDialogTerm();
        }
        state.active = false;
    }
}

pub fn feed_event(_event: &sdl2::event::Event) {}
