use anyhow::Result;
static PROMOTER: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(target_os = "vita")]
mod sys {
    use anyhow::{Result, bail};
    use std::ffi::CString;
    use vitasdk_sys::*;
    pub struct Session;
    impl Session {
        pub fn open() -> Result<Self> {
            unsafe {
                let mut opt_storage = [0u32; 0x100];
                opt_storage[0] = 0;
                opt_storage[1] = opt_storage.as_ptr() as u32;
                let mut paf_args: [u32; 5] = [0x400000, 0xEA60, 0x40000, 0, 0];
                let ret = sceSysmoduleLoadModuleInternalWithArg(
                    SCE_SYSMODULE_INTERNAL_PAF,
                    std::mem::size_of_val(&paf_args) as u32,
                    paf_args.as_mut_ptr().cast(),
                    opt_storage.as_mut_ptr().cast(),
                );
                if ret < 0 {
                    bail!("couldn't load the paf sysmodule (0x{ret:08x})");
                }
                let ret = sceSysmoduleLoadModuleInternal(SCE_SYSMODULE_INTERNAL_PROMOTER_UTIL);
                if ret < 0 {
                    bail!("couldn't load the promoter sysmodule (0x{ret:08x})");
                }
                let ret = scePromoterUtilityInit();
                if ret < 0 {
                    bail!("scePromoterUtilityInit failed (0x{ret:08x})");
                }
            }
            Ok(Self)
        }
        pub fn is_installed(&self, titleid: &str) -> Option<bool> {
            let path = CString::new(titleid).ok()?;
            let mut res: i32 = 0;
            let ret = unsafe { scePromoterUtilityCheckExist(path.as_ptr(), &mut res) };

            Some(ret >= 0 && res == 1)
        }
        pub fn promote(&self, dir: &str, mut on_tick: impl FnMut(u32)) -> Result<()> {
            let path = CString::new(dir)?;
            let started_at = std::time::Instant::now();
            unsafe {
                let ret = scePromoterUtilityPromotePkg(path.as_ptr(), 0);
                if ret < 0 {
                    bail!("scePromoterUtilityPromotePkg failed (0x{ret:08x})");
                }
                loop {
                    let mut state: i32 = 0;
                    let ret = scePromoterUtilityGetState(&mut state);
                    if ret < 0 {
                        bail!("scePromoterUtilityGetState failed (0x{ret:08x})");
                    }
                    if state == 0 {
                        break;
                    }
                    on_tick(started_at.elapsed().as_secs() as u32);
                    sceKernelDelayThread(200_000);
                }
                let mut result: i32 = 0;
                if scePromoterUtilityGetResult(&mut result) >= 0 && result < 0 {
                    bail!("the system rejected the package (0x{result:08x})");
                }
            }
            Ok(())
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                scePromoterUtilityExit();
                sceSysmoduleUnloadModuleInternal(SCE_SYSMODULE_INTERNAL_PROMOTER_UTIL);
                let mut opt = SceSysmoduleOpt { flags: 0, result: std::ptr::null_mut(), unused: [0; 2] };
                sceSysmoduleUnloadModuleInternalWithArg(SCE_SYSMODULE_INTERNAL_PAF, 0, std::ptr::null_mut(), &mut opt);
            }
        }
    }
}
#[cfg(target_os = "vita")]
pub fn promote_package(dir: &str, tx: &tokio::sync::watch::Sender<super::Progress>) -> Result<()> {
    let _guard = PROMOTER.lock().map_err(|_| anyhow::anyhow!("the promoter lock is poisoned"))?;
    let session = sys::Session::open()?;
    session.promote(dir, |elapsed_secs| {
        let _ = tx.send(super::Progress::Installing { elapsed_secs });
    })
}
#[cfg(target_os = "vita")]
pub fn installed_titles(titleids: &[String]) -> Option<Vec<String>> {
    let _guard = PROMOTER.lock().ok()?;
    let session = match sys::Session::open() {
        Ok(session) => session,
        Err(err) => {
            eprintln!("couldn't ask the system what is installed: {err:#}");
            return None;
        }
    };
    let mut found = Vec::new();
    for titleid in titleids {
        if session.is_installed(titleid) == Some(true) {
            found.push(titleid.clone());
        }
    }
    Some(found)
}
#[cfg(not(target_os = "vita"))]
pub fn promote_package(_dir: &str, _tx: &tokio::sync::watch::Sender<super::Progress>) -> Result<()> {
    anyhow::bail!("installing only works on the vita itself")
}
#[cfg(not(target_os = "vita"))]
pub fn installed_titles(_titleids: &[String]) -> Option<Vec<String>> {
    None
}
