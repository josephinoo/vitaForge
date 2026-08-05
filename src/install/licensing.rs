use anyhow::{Result, bail};

pub fn is_module_present(_module_name: &str) -> bool {
    #[cfg(target_os = "vita")]
    {
        if let Ok(c_name) = std::ffi::CString::new(_module_name) {
            unsafe {
                #[link(name = "SceVshBridge_stub")]
                unsafe extern "C" {
                    fn _vshKernelSearchModuleByName(
                        module_name: *const std::ffi::c_char,
                        opt: *mut std::ffi::c_void,
                    ) -> i32;
                }

                let mut opt = [0u8; 64];
                return _vshKernelSearchModuleByName(c_name.as_ptr(), opt.as_mut_ptr() as *mut _) >= 0;
            }
        }
    }
    false
}

/// Resolves the PSN content ID needed to build a fake NoNpDrm/NoPspEmuDrm
/// license. Prefers the catalog-provided `content_id`; when absent, derives
/// one from `titleid` + `region` (works for the common case, may miss
/// non-standard region prefixes).
pub fn resolve_content_id(entry: &crate::data::AppEntry) -> Result<String> {
    if let Some(id) = entry.content_id.as_deref().filter(|s| !s.is_empty()) {
        return Ok(id.to_owned());
    }

    if entry.titleid.is_empty() {
        bail!("no content ID or title ID available for '{}' — can't build a license", entry.name);
    }

    let prefix = match entry.region.as_deref() {
        Some(r) if r.eq_ignore_ascii_case("EU") => "EP",
        Some(r) if r.eq_ignore_ascii_case("JP") => "JP",
        Some(r) if r.eq_ignore_ascii_case("ASIA") => "UP",
        _ => "UP",
    };

    Ok(format!("{prefix}0001-{}_00-0000000000000000", entry.titleid))
}

pub fn create_fake_license(content_id: &str) -> Vec<u8> {
    #[cfg(target_os = "vita")]
    {
        let mut license: vitasdk_sys::SceNpDrmLicense = unsafe { std::mem::zeroed() };
        license.account_id = 0x0123456789ABCDEF;
        for b in &mut license.ecdsa_signature {
            *b = -1i8;
        }
        let bytes = content_id.as_bytes();
        let len = bytes.len().min(0x30 - 1);
        unsafe {
            let dest = license.content_id.as_mut_ptr() as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, len);
        }
        let slice = unsafe {
            std::slice::from_raw_parts(
                &license as *const _ as *const u8,
                std::mem::size_of::<vitasdk_sys::SceNpDrmLicense>(),
            )
        };
        slice.to_vec()
    }
    #[cfg(not(target_os = "vita"))]
    {
        // Host build never installs anything; the exact layout doesn't matter here.
        let _ = content_id;
        vec![0u8; 512]
    }
}
