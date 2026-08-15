use std::sync::Mutex;
use std::time::{Duration, Instant};

const STORAGE_CACHE_TTL: Duration = Duration::from_secs(15);

struct StorageCache {
    at: Instant,
    value: Option<(u64, u64)>,
}
static STORAGE_CACHE: Mutex<Option<StorageCache>> = Mutex::new(None);

pub fn storage(dev: &str) -> Option<(u64, u64)> {
    if let Ok(guard) = STORAGE_CACHE.lock() {
        if let Some(cache) = guard.as_ref() {
            if cache.at.elapsed() < STORAGE_CACHE_TTL {
                return cache.value;
            }
        }
    }
    let value = storage_uncached(dev);
    if let Ok(mut guard) = STORAGE_CACHE.lock() {
        *guard = Some(StorageCache { at: Instant::now(), value });
    }
    value
}

#[cfg(target_os = "vita")]
fn storage_uncached(dev: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;
    let dev = CString::new(dev).ok()?;
    let mut max_size: u64 = 0;
    let mut free_size: u64 = 0;
    let ret = unsafe { vitasdk_sys::sceAppMgrGetDevInfo(dev.as_ptr(), &mut max_size, &mut free_size) };
    if ret < 0 || max_size == 0 {
        return None;
    }
    Some((max_size.saturating_sub(free_size), max_size))
}
#[cfg(not(target_os = "vita"))]
fn storage_uncached(_dev: &str) -> Option<(u64, u64)> {
    None
}
