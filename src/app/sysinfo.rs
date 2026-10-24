use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STORAGE_CACHE_TTL: Duration = Duration::from_secs(15);
const BATTERY_CACHE_TTL: Duration = Duration::from_secs(5);

pub fn log_stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let t = secs % 86_400;
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn log_stamp_is_a_full_date() {
        let stamp = super::log_stamp();
        assert_eq!(stamp.len(), 19, "{stamp}");
        assert!(stamp.starts_with("20"), "{stamp}");
    }
}
