use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STORAGE_CACHE_TTL: Duration = Duration::from_secs(15);
const BATTERY_CACHE_TTL: Duration = Duration::from_secs(5);

pub fn clock_hhmm() -> String {
    let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return String::from("--:--");
    };
    let mins = (elapsed.as_secs() / 60) % (24 * 60);
    format!("{:02}:{:02}", mins / 60, mins % 60)
}

struct BatteryCache {
    at: Instant,
    percent: Option<u8>,
}
static BATTERY_CACHE: Mutex<Option<BatteryCache>> = Mutex::new(None);

pub fn battery_percent() -> Option<u8> {
    if let Ok(guard) = BATTERY_CACHE.lock() {
        if let Some(cache) = guard.as_ref() {
            if cache.at.elapsed() < BATTERY_CACHE_TTL {
                return cache.percent;
            }
        }
    }
    let percent = battery_percent_uncached();
    if let Ok(mut guard) = BATTERY_CACHE.lock() {
        *guard = Some(BatteryCache {
            at: Instant::now(),
            percent,
        });
    }
    percent
}

#[cfg(target_os = "vita")]
fn battery_percent_uncached() -> Option<u8> {
    let pct = unsafe { vitasdk_sys::scePowerGetBatteryLifePercent() };
    if pct < 0 {
        return None;
    }
    Some((pct as u8).min(100))
}

#[cfg(not(target_os = "vita"))]
fn battery_percent_uncached() -> Option<u8> {
    None
}

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
        *guard = Some(StorageCache {
            at: Instant::now(),
            value,
        });
    }
    value
}

#[cfg(target_os = "vita")]
fn storage_uncached(dev: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;
    let dev = CString::new(dev).ok()?;
    let mut max_size: u64 = 0;
    let mut free_size: u64 = 0;
    let ret =
        unsafe { vitasdk_sys::sceAppMgrGetDevInfo(dev.as_ptr(), &mut max_size, &mut free_size) };
    if ret < 0 || max_size == 0 {
        return None;
    }
    Some((max_size.saturating_sub(free_size), max_size))
}
#[cfg(not(target_os = "vita"))]
fn storage_uncached(_dev: &str) -> Option<(u64, u64)> {
    None
}
