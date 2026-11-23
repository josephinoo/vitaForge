//! Embedded USB Mass Storage mode (ux0:) without VitaShell.
//! Requires `usbdevice.skprx` bundled in `app0:module/usbdevice.skprx` (or fallback paths).
use anyhow::Result;

#[cfg(target_os = "vita")]
mod imp {
    use anyhow::{Result, bail};
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::path::Path;
    use std::sync::Mutex;

    const MODULE_PATHS: [&str; 3] = [
        "app0:module/usbdevice.skprx",
        "ux0:data/vitaforge/usbdevice.skprx",
        "ux0:VitaShell/module/usbdevice.skprx",
    ];

    #[repr(C)]
    struct TaiModuleArgs {
        size: usize,
        pid: i32,
        args: usize,
        argp: *mut c_void,
        flags: u32,
    }

    unsafe extern "C" {
        fn taiLoadStartKernelModuleForUser(path: *const c_char, args: *mut TaiModuleArgs) -> i32;
        fn taiStopUnloadKernelModuleForUser(
            modid: i32,
            args: *mut TaiModuleArgs,
            opt: *mut c_void,
            res: *mut c_int,
        ) -> c_int;
        fn sceUsbstorVStorSetDeviceInfo(name: *const c_char, version: *const c_char) -> c_int;
        fn sceUsbstorVStorSetImgFilePath(path: *const c_char) -> c_int;
        fn sceUsbstorVStorStart(vstor_type: c_int) -> c_int;
        fn sceUsbstorVStorStop() -> c_int;
    }

    struct ActiveUsb {
        modid: i32,
    }

    static STATE: Mutex<Option<ActiveUsb>> = Mutex::new(None);

    pub fn is_active() -> bool {
        STATE.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub fn start() -> Result<()> {
        let mut guard = STATE.lock().map_err(|_| anyhow::anyhow!("usb state poisoned"))?;
        if guard.is_some() {
            return Ok(());
        }
        let mod_path = MODULE_PATHS
            .iter()
            .copied()
            .find(|p| Path::new(p).exists())
            .ok_or_else(|| anyhow::anyhow!("usbdevice.skprx not found (expected app0:module/usbdevice.skprx)"))?;

        let c_mod = CString::new(mod_path)?;
        let mut args = TaiModuleArgs {
            size: std::mem::size_of::<TaiModuleArgs>(),
            pid: 0,
            args: 0,
            argp: std::ptr::null_mut(),
            flags: 0,
        };
        let modid = unsafe { taiLoadStartKernelModuleForUser(c_mod.as_ptr(), &mut args) };
        if modid < 0 {
            bail!("taiLoadStartKernelModule failed (0x{modid:08x})");
        }

        let name = CString::new("VitaForge")?;
        let ver = CString::new("1.00")?;
        // shortcut: assumes ux0: is backed by xmc/sd2vita hooked by usbdevice.skprx; add mount selector if uma0:/imc0: is needed
        let img = CString::new("sdstor0:xmc-lp-ign-userext")?;
        let ret = unsafe {
            let r1 = sceUsbstorVStorSetDeviceInfo(name.as_ptr(), ver.as_ptr());
            let r2 = if r1 >= 0 { sceUsbstorVStorSetImgFilePath(img.as_ptr()) } else { r1 };
            if r2 >= 0 { sceUsbstorVStorStart(0) } else { r2 }
        };
        if ret < 0 {
            let mut res: c_int = 0;
            unsafe {
                let _ = taiStopUnloadKernelModuleForUser(modid, &mut args, std::ptr::null_mut(), &mut res);
            }
            bail!("sceUsbstorVStorStart failed (0x{ret:08x})");
        }
        *guard = Some(ActiveUsb { modid });
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let mut guard = STATE.lock().map_err(|_| anyhow::anyhow!("usb state poisoned"))?;
        let Some(active) = guard.take() else {
            return Ok(());
        };
        let mut args = TaiModuleArgs {
            size: std::mem::size_of::<TaiModuleArgs>(),
            pid: 0,
            args: 0,
            argp: std::ptr::null_mut(),
            flags: 0,
        };
        let mut res: c_int = 0;
        unsafe {
            let _ = sceUsbstorVStorStop();
            let _ = taiStopUnloadKernelModuleForUser(active.modid, &mut args, std::ptr::null_mut(), &mut res);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "vita"))]
mod imp {
    use anyhow::{Result, bail};
    pub fn is_active() -> bool {
        false
    }
    pub fn start() -> Result<()> {
        bail!("USB mode is only available on PS Vita")
    }
    pub fn stop() -> Result<()> {
        Ok(())
    }
}

pub use imp::{is_active, start, stop};
