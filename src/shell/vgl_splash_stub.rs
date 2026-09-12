
use std::os::raw::c_void;

#[unsafe(no_mangle)]
#[used]
pub static mut is_splashscreen_active: u8 = 0;

#[unsafe(no_mangle)]
#[used]
pub static mut splash_mutex: [i32; 2] = [0, 0];

#[unsafe(no_mangle)]
pub extern "C" fn invoke_splashscreen() {}

#[unsafe(no_mangle)]
pub extern "C" fn clear_splashscreen() {}

#[unsafe(no_mangle)]
pub extern "C" fn vglGetCaveBuffer(sz: *mut usize) -> *mut c_void {
    if !sz.is_null() {
        unsafe {
            *sz = 0;
        }
    }
    std::ptr::null_mut()
}

pub fn retain_splash_stubs() {
    invoke_splashscreen();
    clear_splashscreen();
    let mut n = 0usize;
    let _ = vglGetCaveBuffer(&mut n);
    unsafe {
        let _ = is_splashscreen_active;
        let _ = splash_mutex[0];
    }
}
