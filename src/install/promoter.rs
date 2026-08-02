
use anyhow::Result;

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

        pub fn promote(&self, dir: &str) -> Result<()> {
            let path = CString::new(dir)?;
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
pub fn promote_package(dir: &str) -> Result<()> {
    let session = sys::Session::open()?;
    session.promote(dir)
}

#[cfg(not(target_os = "vita"))]
pub fn promote_package(_dir: &str) -> Result<()> {
    anyhow::bail!("installing only works on the vita itself")
}
