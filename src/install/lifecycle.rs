use anyhow::Result;

fn clean_title(titleid: &str) -> Result<String> {
    let id = titleid.trim().to_uppercase();
    if id.len() != 9 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        anyhow::bail!("'{titleid}' is not a valid title id");
    }
    Ok(id)
}

/// Hands the screen over to another app, the way PSVitaAlive's updater does.
/// The caller exits afterwards so the Vita switches cleanly.
#[cfg(target_os = "vita")]
pub fn launch(titleid: &str) -> Result<()> {
    let id = clean_title(titleid)?;
    let uri = std::ffi::CString::new(format!("psgm:play?titleid={id}"))?;
    let ret = unsafe { vitasdk_sys::sceAppMgrLaunchAppByUri(0xFFFFF, uri.as_ptr()) };
    if ret < 0 {
        anyhow::bail!("couldn't launch {id} (0x{ret:08x})");
    }
    Ok(())
}
#[cfg(not(target_os = "vita"))]
pub fn launch(titleid: &str) -> Result<()> {
    clean_title(titleid)?;
    anyhow::bail!("launching apps is only available on PS Vita")
}

/// Lets the app write into its own `ux0:app/<TITLEID>` folder, which the system
/// keeps mounted as `app0:` while it runs (the step VitaDB Downloader takes).
#[cfg(target_os = "vita")]
pub fn release_own_folder() {
    let mount = std::ffi::CString::new("app0:").expect("no interior nul");
    let ret = unsafe { vitasdk_sys::sceAppMgrUmount(mount.as_ptr()) };
    super::log_file(&format!("sceAppMgrUmount(app0:) = 0x{ret:08x}"));
}
#[cfg(not(target_os = "vita"))]
pub fn release_own_folder() {}

/// Restarts into the freshly written eboot.
#[cfg(target_os = "vita")]
pub fn restart_self() -> Result<()> {
    let path = std::ffi::CString::new("app0:eboot.bin")?;
    let ret = unsafe {
        vitasdk_sys::sceAppMgrLoadExec(path.as_ptr(), std::ptr::null(), std::ptr::null())
    };
    super::log_file(&format!("sceAppMgrLoadExec = 0x{ret:08x}"));
    // LoadExec only returns on failure; ask the shell to reopen us instead.
    launch("VITAFORGE")?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    launch("VITAFORGE")
}
#[cfg(not(target_os = "vita"))]
pub fn restart_self() -> Result<()> {
    anyhow::bail!("restarting is only available on PS Vita")
}

/// Removes an installed app and its LiveArea bubble through the system promoter.
pub async fn uninstall(titleid: &str) -> Result<()> {
    let id = clean_title(titleid)?;
    super::log_file(&format!("Uninstall requested for {id}"));
    tokio::task::spawn_blocking(move || super::promoter::delete_package(&id)).await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clean_title;

    #[test]
    fn titles_are_nine_alphanumerics() {
        assert_eq!(clean_title(" nanosaur2 ").unwrap(), "NANOSAUR2");
        assert!(clean_title("short").is_err());
        assert!(clean_title("../../etc/x").is_err());
    }
}
