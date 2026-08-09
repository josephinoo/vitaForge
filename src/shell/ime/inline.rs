
use std::ffi::c_void;
use std::sync::Mutex;
use vitasdk_sys::*;

const PSP2_SDK_VERSION: SceUInt32 = 0x0357_0011;
const MAX_TEXT_LEN: u32 = 256;
const BUFFER_CAPACITY: usize = 264;
#[repr(align(64))]
struct TextBuf([SceWChar16; BUFFER_CAPACITY]);
static mut INITIAL_TEXT: TextBuf = TextBuf([0; BUFFER_CAPACITY]);
static mut INPUT_BUFFER: TextBuf = TextBuf([0; BUFFER_CAPACITY]);
#[repr(align(64))]
#[allow(dead_code)]
struct WorkBuf([u8; SCE_IME_WORK_BUFFER_SIZE as usize]);
static mut WORK_BUFFER: WorkBuf = WorkBuf([0; SCE_IME_WORK_BUFFER_SIZE as usize]);
struct State {
    open: bool,
    text: String,
    confirmed: bool,
}
static STATE: Mutex<State> = Mutex::new(State { open: false, text: String::new(), confirmed: false });
unsafe fn decode_input_buffer() -> String {
    unsafe {
        let cells = &(*(&raw const INPUT_BUFFER)).0;
        let len = cells.iter().position(|&c| c == 0).unwrap_or(BUFFER_CAPACITY);
        String::from_utf16_lossy(&cells[..len])
    }
}
unsafe extern "C" fn on_ime_event(_arg: *mut c_void, event: *const SceImeEventData) {
    let Some(event) = (unsafe { event.as_ref() }) else {
        return;
    };
    match event.id {
        SCE_IME_EVENT_UPDATE_TEXT => {
            let text = unsafe { decode_input_buffer() };
            if let Ok(mut state) = STATE.lock() {
                state.text = text;
            }
        }
        SCE_IME_EVENT_PRESS_ENTER => {
            if let Ok(mut state) = STATE.lock() {
                state.confirmed = true;
            }
        }
        SCE_IME_EVENT_PRESS_CLOSE => {
            unsafe { sceImeClose() };
            if let Ok(mut state) = STATE.lock() {
                state.open = false;
            }
        }
        _ => {}
    }
}
pub fn open(_video: &sdl2::VideoSubsystem, _window: &sdl2::video::Window) {
    if STATE.lock().map(|s| s.open).unwrap_or(false) {
        return;
    }
    let result = unsafe {
        sceSysmoduleLoadModule(SCE_SYSMODULE_IME);
        (&raw mut INITIAL_TEXT).write(TextBuf([0; BUFFER_CAPACITY]));
        (&raw mut INPUT_BUFFER).write(TextBuf([0; BUFFER_CAPACITY]));
        let mut param: SceImeParam = core::mem::zeroed();
        param.sdkVersion = PSP2_SDK_VERSION;
        param.supportedLanguages = u64::from(SCE_IME_LANGUAGE_ENGLISH | SCE_IME_LANGUAGE_SPANISH);
        param.languagesForced = SCE_FALSE as SceBool;
        param.type_ = SCE_IME_TYPE_DEFAULT;
        param.option = SCE_IME_OPTION_NO_ASSISTANCE;
        param.work = (&raw mut WORK_BUFFER).cast();
        param.arg = core::ptr::null_mut();
        param.handler = Some(on_ime_event);
        param.filter = None;
        param.initialText = (&raw mut (*(&raw mut INITIAL_TEXT)).0).cast();
        param.maxTextLength = MAX_TEXT_LEN;
        param.inputTextBuffer = (&raw mut (*(&raw mut INPUT_BUFFER)).0).cast();
        param.enterLabel = SCE_IME_ENTER_LABEL_DEFAULT as SceUChar8;
        sceImeOpen(&param)
    };
    if result < 0 {
        eprintln!("couldn't open the native keyboard: {result:#010x}");
        return;
    }
    if let Ok(mut state) = STATE.lock() {
        state.open = true;
        state.text.clear();
        state.confirmed = false;
    }
}
pub fn close(_video: &sdl2::VideoSubsystem) {
    let was_open = STATE.lock().map(|mut s| std::mem::replace(&mut s.open, false)).unwrap_or(false);
    if was_open {
        unsafe { sceImeClose() };
    }
}
pub fn update() {
    if !STATE.lock().map(|s| s.open).unwrap_or(false) {
        return;
    }
    let status = unsafe { sceImeUpdate() };
    if status < 0 {
        if let Ok(mut state) = STATE.lock() {
            state.open = false;
        }
    }
}
pub fn feed_event(_event: &sdl2::event::Event) {}
pub fn is_shown(_video: &sdl2::VideoSubsystem, _window: &sdl2::video::Window) -> bool {
    STATE.lock().map(|s| s.open).unwrap_or(false)
}
pub fn confirmed() -> bool {
    STATE.lock().map(|s| s.confirmed).unwrap_or(false)
}
pub fn take_text() -> Option<String> {
    let mut state = STATE.lock().ok()?;
    if state.text.is_empty() { None } else { Some(std::mem::take(&mut state.text)) }
}
