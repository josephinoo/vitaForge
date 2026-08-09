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
// Content ID is 36 bytes: 6-char group id + 9-char title id + "_00-" + 16 hex chars.
// PSN CDN urls embed it verbatim so we can just scan for the pattern.
fn extract_content_id_from_url(url: &str) -> Option<String> {
    let bytes = url.as_bytes();
    if bytes.len() < 36 {
        return None;
    }
    for i in 0..=bytes.len() - 36 {
        let sub = &url[i..i + 36];
        let sub_bytes = sub.as_bytes();
        if sub_bytes[6] == b'-'
            && sub_bytes[16] == b'_'
            && sub_bytes[17] == b'0'
            && sub_bytes[18] == b'0'
            && sub_bytes[19] == b'-'
            && sub_bytes[0].is_ascii_uppercase()
            && sub_bytes[1].is_ascii_uppercase()
        {
            return Some(sub.to_string());
        }
    }
    None
}
pub fn resolve_content_id(entry: &crate::data::AppEntry) -> Result<String> {
    if let Some(id) = entry.content_id.as_deref().filter(|s| !s.is_empty()) {
        return Ok(id.to_owned());
    }
    if let Some(id_from_url) = extract_content_id_from_url(&entry.download_url) {
        return Ok(id_from_url);
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
// SceNpDrmLicense layout: account_id at 0x08, content_id at 0x10 (0x30 bytes), signature at 0x70.
pub fn create_fake_license(content_id: &str) -> Vec<u8> {
    let mut rif = vec![0u8; 512];
    rif[0x08..0x10].copy_from_slice(&0x0123456789ABCDEFu64.to_le_bytes());
    // snprintf into a 0x30 field: truncated to fit, always NUL-terminated.
    let cid = content_id.as_bytes();
    let cid_len = cid.len().min(0x30 - 1);
    rif[0x10..0x10 + cid_len].copy_from_slice(&cid[..cid_len]);
    rif[0x70..0x70 + 0x28].fill(0xFF);
    rif
}
unsafe extern "C" {
    fn pkgi_zrif_decode(
        zrif: *const std::ffi::c_char,
        rif: *mut u8,
        err: *mut std::ffi::c_char,
        err_size: u32,
    ) -> i32;
}
pub fn decode_zrif(zrif_str: &str) -> Option<Vec<u8>> {
    let c_zrif = std::ffi::CString::new(zrif_str).ok()?;
    let mut rif = vec![0u8; 1024];
    let mut err_buf = [0i8; 256];
    let res = unsafe {
        pkgi_zrif_decode(
            c_zrif.as_ptr(),
            rif.as_mut_ptr(),
            err_buf.as_mut_ptr(),
            err_buf.len() as u32,
        )
    };
    if res > 0 {
        rif.truncate(res as usize);
        Some(rif)
    } else {
        None
    }
}
pub fn get_license_for_entry(entry: &crate::data::AppEntry) -> Option<Vec<u8>> {
    if let Some(zrif) = entry.zrif.as_deref().filter(|s| !s.is_empty()) {
        if let Some(rif_bytes) = decode_zrif(zrif) {
            return Some(rif_bytes);
        }
    }
    if let Ok(content_id) = resolve_content_id(entry) {
        return Some(create_fake_license(&content_id));
    }
    None
}
