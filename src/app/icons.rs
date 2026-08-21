use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
const CACHE_DIR: &str = "ux0:data/vitaforge/cache";
const MAX_CONCURRENT_FETCHES: usize = 6;
const MAX_CONCURRENT_DECODES: usize = 2;
const LIVE_RESERVED_SLOTS: usize = 2;
const MAX_BACKOFF: Duration = Duration::from_secs(15 * 60);
pub const MAX_ICON_SIDE: u32 = 128;
pub const MAX_SCREENSHOT_SIDE: u32 = 480;
pub const HERO_SIDE: u32 = 480;
const MAX_RESIDENT_BYTES: usize = 24 * 1024 * 1024;
const MAX_RESIDENT_READY: usize = 160;
const MAX_META_ENTRIES: usize = 2048;
const MAX_GPU_BACKLOG_FOR_SPAWN: usize = 24;
const RETRY_AFTER: Duration = Duration::from_secs(5);
const EXHAUSTED_RETRY_AFTER: Duration = Duration::from_secs(300);
const MAX_ATTEMPTS: u32 = 3;
const MAX_THROTTLE_ATTEMPTS: u32 = 80;
const REQUEST_SPACING: Duration = Duration::from_millis(15);
const MAX_QUEUE_AHEAD: Duration = Duration::from_millis(3000);
const DEFAULT_THROTTLE: Duration = Duration::from_secs(2);
const RETRY_DEFERRED: Duration = Duration::from_millis(80);
const BACKGROUND_PACING: Duration = Duration::from_millis(15);
const LIVE_PRIORITY_WINDOW: Duration = Duration::from_millis(100);
enum IconState {
    Loading,
    Ready { texture: egui::TextureHandle, last_used: u64, byte_size: usize },
    Failed { at: Instant, attempts: u32 },
    Throttled { until: Instant, attempts: u32 },
}
impl IconState {
    fn retriable(&self) -> bool {
        match self {
            IconState::Failed { at, attempts } => {
                let cooldown =
                    if *attempts >= MAX_ATTEMPTS { EXHAUSTED_RETRY_AFTER } else { RETRY_AFTER };
                at.elapsed() >= cooldown
            }
            IconState::Throttled { attempts, .. } => *attempts < MAX_THROTTLE_ATTEMPTS,
            _ => false,
        }
    }
}
type Bucket = HashMap<String, IconState>;
fn texture_name(url: &str, max_side: u32) -> String {
    format!("{url}@{max_side}")
}
#[derive(Clone)]
pub struct IconCache {
    entries: Arc<Mutex<HashMap<u32, Bucket>>>,
    disk_index: Arc<Mutex<HashSet<String>>>,
    fetch_limit: Arc<Semaphore>,
    clock: Arc<Mutex<u64>>,
    gate: Arc<Mutex<Instant>>,
    precache_gate: Arc<Mutex<Instant>>,
    rate_limited_until: Arc<Mutex<Option<Instant>>>,
    last_live_request: Arc<Mutex<Instant>>,
    ready_count: Arc<AtomicUsize>,
    resident_bytes: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    gpu_backlog: Arc<AtomicUsize>,
}
impl IconCache {
    pub fn new() -> Self {
        let far_past = Instant::now() - LIVE_PRIORITY_WINDOW - Duration::from_secs(1);
        let disk_index = Arc::new(Mutex::new(HashSet::new()));
        let scan_into = disk_index.clone();
        std::thread::spawn(move || {
            let scanned = scan_disk_index();
            if let Ok(mut guard) = scan_into.lock() {
                guard.extend(scanned);
            }
        });
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            disk_index,
            fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES)),
            clock: Arc::new(Mutex::new(0)),
            gate: Arc::new(Mutex::new(Instant::now())),
            precache_gate: Arc::new(Mutex::new(Instant::now())),
            rate_limited_until: Arc::new(Mutex::new(None)),
            last_live_request: Arc::new(Mutex::new(far_past)),
            ready_count: Arc::new(AtomicUsize::new(0)),
            resident_bytes: Arc::new(AtomicUsize::new(0)),
            in_flight: Arc::new(AtomicUsize::new(0)),
            gpu_backlog: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn disk_has(&self, digest: &str) -> bool {
        self.disk_index
            .lock()
            .map(|g| g.contains(digest))
            .unwrap_or(false)
    }
    fn disk_remember(&self, digest: String) {
        if let Ok(mut g) = self.disk_index.lock() {
            g.insert(digest);
        }
    }
    pub fn clear_disk_index(&self) {
        if let Ok(mut g) = self.disk_index.lock() {
            g.clear();
        }
    }
    pub fn set_gpu_backlog(&self, n: usize) {
        self.gpu_backlog.store(n, Ordering::Relaxed);
    }
    fn tick(&self) -> u64 {
        let mut clock = self.clock.lock().unwrap();
        *clock += 1;
        *clock
    }
    pub fn rate_limited_for(&self) -> Option<Duration> {
        let mut guard = self.rate_limited_until.lock().unwrap();
        let left = (*guard)?.saturating_duration_since(Instant::now());
        if left.is_zero() {
            *guard = None;
            return None;
        }
        Some(left)
    }
    fn note_rate_limit(&self, after: Duration) {
        let until = Instant::now() + after.min(MAX_BACKOFF);
        let mut guard = self.rate_limited_until.lock().unwrap();
        if guard.is_none_or(|previous| previous < until) {
            *guard = Some(until);
        }
    }
    fn try_reserve_slot(&self) -> Result<Duration, Duration> {
        if let Some(left) = self.rate_limited_for() {
            return Err(left);
        }
        *self.last_live_request.lock().unwrap() = Instant::now();
        let mut gate = self.gate.lock().unwrap();
        let now = Instant::now();
        let start = (*gate).max(now);
        let wait = start - now;
        if wait > MAX_QUEUE_AHEAD {
            return Err(wait);
        }
        *gate = start + REQUEST_SPACING;
        Ok(wait)
    }
    fn back_off(&self, after: Duration) {
        self.note_rate_limit(after);
        let mut gate = self.gate.lock().unwrap();
        *gate = (*gate).max(Instant::now() + after.min(MAX_BACKOFF));
    }
    fn try_reserve_precache_slot(&self) -> Result<Duration, Duration> {
        if let Some(left) = self.rate_limited_for() {
            return Err(left);
        }
        if self.last_live_request.lock().unwrap().elapsed() < LIVE_PRIORITY_WINDOW {
            return Err(LIVE_PRIORITY_WINDOW);
        }
        let mut gate = self.precache_gate.lock().unwrap();
        let now = Instant::now();
        let start = (*gate).max(now);
        let wait = start - now;
        if wait > MAX_QUEUE_AHEAD {
            return Err(wait);
        }
        *gate = start + BACKGROUND_PACING;
        Ok(wait)
    }
    pub fn is_loading(&self, url: &str, max_side: u32) -> bool {
        let entries = self.entries.lock().unwrap();
        match entries.get(&max_side).and_then(|bucket| bucket.get(url)) {
            Some(IconState::Loading) => true,
            Some(IconState::Throttled { until, attempts }) => {
                *attempts < MAX_THROTTLE_ATTEMPTS && Instant::now() >= *until
            }
            Some(state) => state.retriable(),
            None => false,
        }
    }
    pub fn repaint_delay(&self, url: &str, max_side: u32) -> Option<Duration> {
        let entries = self.entries.lock().unwrap();
        match entries.get(&max_side).and_then(|bucket| bucket.get(url)) {
            Some(IconState::Loading) => Some(Duration::ZERO),
            Some(IconState::Throttled { until, .. }) => {
                Some(until.saturating_duration_since(Instant::now()))
            }
            Some(state) if state.retriable() => Some(Duration::ZERO),
            _ => None,
        }
    }

    pub fn get(&self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        self.get_sized(ctx, url, MAX_ICON_SIDE)
    }
    pub fn get_hero(&self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        self.get_sized(ctx, url, HERO_SIDE)
    }
    pub fn peek(&self, url: &str) -> Option<egui::TextureHandle> {
        let mut entries = self.entries.lock().unwrap();
        let bucket = entries.get_mut(&MAX_ICON_SIDE)?;
        match bucket.get_mut(url)? {
            IconState::Ready { texture, last_used, .. } => {
                *last_used = self.tick();
                Some(texture.clone())
            }
            _ => None,
        }
    }
    pub fn get_sized(&self, ctx: &egui::Context, url: &str, max_side: u32) -> Option<egui::TextureHandle> {
        let mut entries = self.entries.lock().unwrap();
        let now = {
            let mut clock = self.clock.lock().unwrap();
            *clock += 1;
            *clock
        };
        let bucket = entries.entry(max_side).or_default();
        let mut attempts = 0;
        let mut skip_disk = false;
        if let Some(state) = bucket.get_mut(url) {
            match state {
                IconState::Ready { texture, last_used, .. } => {
                    *last_used = now;
                    return Some(texture.clone());
                }
                IconState::Loading => return None,
                IconState::Failed { at, attempts: tries } => {
                    let exhausted = *tries >= MAX_ATTEMPTS;
                    let cooldown = if exhausted { EXHAUSTED_RETRY_AFTER } else { RETRY_AFTER };
                    if at.elapsed() < cooldown {
                        return None;
                    }
                    attempts = if exhausted { 0 } else { *tries };
                }
                IconState::Throttled { until, attempts: tries } => {
                    if *tries >= MAX_THROTTLE_ATTEMPTS || Instant::now() < *until {
                        return None;
                    }
                    attempts = *tries;
                    skip_disk = true;
                }
            }
        }
        let from_disk = !skip_disk && cache_digest(url).is_some_and(|d| self.disk_has(&d));
        let spawn_budget =
            if from_disk { MAX_CONCURRENT_DECODES } else { MAX_CONCURRENT_FETCHES };
        if self.gpu_backlog.load(Ordering::Relaxed) >= MAX_GPU_BACKLOG_FOR_SPAWN
            || self.in_flight.load(Ordering::Relaxed) >= spawn_budget
        {
            return None;
        }
        bucket.insert(url.to_owned(), IconState::Loading);
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        drop(entries);
        let cache = self.clone();
        let ctx = ctx.clone();
        let url = url.to_owned();
        tokio::spawn(async move {
            let outcome = load_icon(&url, max_side, &cache, skip_disk).await;
            let stamp = cache.tick();
            let mut entries = cache.entries.lock().unwrap();
            let bucket = entries.entry(max_side).or_default();
            match outcome {
                LoadOutcome::Ready(color_image) => {
                    let [w, h] = color_image.size;
                    let byte_size = w * h * 4;
                    let name = texture_name(&url, max_side);
                    let texture = ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR);
                    bucket.insert(url, IconState::Ready { texture, last_used: stamp, byte_size });
                    cache.ready_count.fetch_add(1, Ordering::Relaxed);
                    cache.resident_bytes.fetch_add(byte_size, Ordering::Relaxed);
                    let protect = stamp.saturating_sub(1);
                    prune_meta(&mut entries);
                    evict_stale(
                        &mut entries,
                        &ctx,
                        protect,
                        &cache.ready_count,
                        &cache.resident_bytes,
                    );
                }
                LoadOutcome::Deferred { retry_in } => {
                    let until = Instant::now() + retry_in.max(RETRY_DEFERRED);
                    bucket.insert(url, IconState::Throttled { until, attempts: attempts + 1 });
                    prune_meta(&mut entries);
                }
                LoadOutcome::RateLimited { after } => {
                    bucket.insert(
                        url,
                        IconState::Throttled { until: Instant::now() + after, attempts: attempts + 1 },
                    );
                    prune_meta(&mut entries);
                }
                LoadOutcome::Failed => {
                    let attempts = attempts + 1;
                    if attempts >= MAX_ATTEMPTS {
                        eprintln!("giving up on {url} after {attempts} attempts");
                    }
                    bucket.insert(url, IconState::Failed { at: Instant::now(), attempts });
                    prune_meta(&mut entries);
                }
            }
            cache.in_flight.fetch_sub(1, Ordering::Relaxed);
            drop(entries);
            ctx.request_repaint_after(Duration::from_millis(16));
        });
        None
    }

    pub fn prefetch_urls(&self, ctx: &egui::Context, urls: Vec<String>) {
        if self.gpu_backlog.load(Ordering::Relaxed) >= MAX_GPU_BACKLOG_FOR_SPAWN {
            return;
        }
        let mut budget = MAX_CONCURRENT_FETCHES
            .saturating_sub(LIVE_RESERVED_SLOTS)
            .saturating_sub(self.in_flight.load(Ordering::Relaxed))
            .min(4);
        if budget == 0 {
            return;
        }
        for url in urls {
            if budget == 0 {
                break;
            }
            {
                let entries = self.entries.lock().unwrap();
                if entries
                    .get(&MAX_ICON_SIDE)
                    .is_some_and(|bucket| bucket.contains_key(&url))
                {
                    continue;
                }
            }
            let before = self.in_flight.load(Ordering::Relaxed);
            let _ = self.get_sized(ctx, &url, MAX_ICON_SIDE);
            if self.in_flight.load(Ordering::Relaxed) > before {
                budget -= 1;
            } else if self.gpu_backlog.load(Ordering::Relaxed) >= MAX_GPU_BACKLOG_FOR_SPAWN
                || self.in_flight.load(Ordering::Relaxed) >= MAX_CONCURRENT_FETCHES
            {
                break;
            }
        }
    }

    pub fn forget_textures(&self, ctx: &egui::Context, ids: &[egui::TextureId]) {
        if ids.is_empty() {
            return;
        }
        let mut entries = self.entries.lock().unwrap();
        let stale: Vec<(u32, String, usize)> = entries
            .iter()
            .flat_map(|(&max_side, bucket)| {
                bucket.iter().filter_map(move |(url, state)| match state {
                    IconState::Ready { texture, byte_size, .. } if ids.contains(&texture.id()) => {
                        Some((max_side, url.clone(), *byte_size))
                    }
                    _ => None,
                })
            })
            .collect();
        for (max_side, url, byte_size) in stale {
            if let Some(bucket) = entries.get_mut(&max_side) {
                bucket.remove(&url);
            }
            ctx.forget_image(&texture_name(&url, max_side));
            let _ = self
                .ready_count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| Some(n.saturating_sub(1)));
            let _ = self.resident_bytes.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_sub(byte_size))
            });
        }
    }

    pub fn clear_resident(&self, ctx: &egui::Context) {
        let mut entries = self.entries.lock().unwrap();
        for (&max_side, bucket) in entries.iter() {
            for (url, state) in bucket.iter() {
                if matches!(state, IconState::Ready { .. }) {
                    ctx.forget_image(&texture_name(url, max_side));
                }
            }
        }
        entries.clear();
        self.ready_count.store(0, Ordering::Relaxed);
        self.resident_bytes.store(0, Ordering::Relaxed);
    }

    pub fn maintain(&self, ctx: &egui::Context) {
        let ready = self.ready_count.load(Ordering::Relaxed);
        let bytes = self.resident_bytes.load(Ordering::Relaxed);
        if ready <= MAX_RESIDENT_READY && bytes <= MAX_RESIDENT_BYTES {
            return;
        }
        let protect = *self.clock.lock().unwrap();
        let mut entries = self.entries.lock().unwrap();
        prune_meta(&mut entries);
        evict_stale(&mut entries, ctx, protect, &self.ready_count, &self.resident_bytes);
    }

    pub fn precache_background(&self, urls: Vec<String>) {
        let cache = self.clone();
        tokio::spawn(async move {
            let to_fetch: Vec<String> = urls
                .into_iter()
                .filter(|url| {
                    cache_digest(url)
                        .map(|d| !cache.disk_has(&d))
                        .unwrap_or(true)
                })
                .collect();
            for url in to_fetch {
                if cache.rate_limited_for().is_some() {
                    return;
                }
                if cache.is_loading(&url, MAX_ICON_SIDE) {
                    continue;
                }
                {
                    let entries = cache.entries.lock().unwrap();
                    if matches!(
                        entries.get(&MAX_ICON_SIDE).and_then(|bucket| bucket.get(&url)),
                        Some(IconState::Ready { .. })
                    ) {
                        continue;
                    }
                }
                loop {
                    match cache.try_reserve_precache_slot() {
                        Ok(wait) => {
                            if !wait.is_zero() {
                                tokio::time::sleep(wait).await;
                            }
                            break;
                        }
                        Err(retry_in) => tokio::time::sleep(retry_in.max(LIVE_PRIORITY_WINDOW)).await,
                    }
                }
                if let WarmOutcome::Backoff(after) = warm_disk_cache(&url, &cache).await {
                    eprintln!(
                        "precache stopping: server rate limit, {}s to go",
                        after.as_secs()
                    );
                    return;
                }
            }
        });
    }

    pub fn warm_disk_urls(&self, urls: Vec<String>) {
        if urls.is_empty() {
            return;
        }
        let cache = self.clone();
        tokio::spawn(async move {
            for url in urls {
                if cache_digest(&url).is_some_and(|d| cache.disk_has(&d)) {
                    continue;
                }
                loop {
                    match cache.try_reserve_precache_slot() {
                        Ok(wait) => {
                            if !wait.is_zero() {
                                tokio::time::sleep(wait).await;
                            }
                            break;
                        }
                        Err(retry_in) => tokio::time::sleep(retry_in.max(LIVE_PRIORITY_WINDOW)).await,
                    }
                }
                if let WarmOutcome::Backoff(after) = warm_disk_cache(&url, &cache).await {
                    eprintln!(
                        "precache stopping: server rate limit, {}s to go",
                        after.as_secs()
                    );
                    return;
                }
            }
        });
    }
}
enum WarmOutcome {
    Continue,
    Backoff(Duration),
}

async fn warm_disk_cache(url: &str, cache: &IconCache) -> WarmOutcome {
    let Some(digest) = cache_digest(url) else { return WarmOutcome::Continue };
    if cache.disk_has(&digest) {
        return WarmOutcome::Continue;
    }
    let Some(path) = cache_path_for_digest(&digest) else { return WarmOutcome::Continue };
    let bytes = match fetch_bytes(url).await {
        FetchOutcome::Ok(bytes) => bytes,
        FetchOutcome::RateLimited { after } => {
            cache.back_off(after);
            return WarmOutcome::Backoff(after.min(MAX_BACKOFF));
        }
        FetchOutcome::Failed => return WarmOutcome::Continue,
    };
    let wrote = tokio::task::spawn_blocking(move || {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, &bytes).is_ok()
    })
    .await
    .unwrap_or(false);
    if wrote {
        cache.disk_remember(digest);
    }
    WarmOutcome::Continue
}
fn prune_meta(entries: &mut HashMap<u32, Bucket>) {
    let mut meta: Vec<(u32, String)> = Vec::new();
    for (&max_side, bucket) in entries.iter() {
        for (url, state) in bucket.iter() {
            if let IconState::Throttled { until, attempts } = state
                && (*attempts >= MAX_THROTTLE_ATTEMPTS
                    || Instant::now() > *until + Duration::from_secs(30))
            {
                meta.push((max_side, url.clone()));
            }
        }
    }
    for (max_side, url) in meta {
        if let Some(bucket) = entries.get_mut(&max_side) {
            bucket.remove(&url);
        }
    }
    let mut leftover: Vec<(u32, String, u64)> = Vec::new();
    for (&max_side, bucket) in entries.iter() {
        for (url, state) in bucket.iter() {
            let rank = match state {
                IconState::Ready { .. } | IconState::Loading => continue,
                IconState::Failed { at, .. } => at.elapsed().as_millis() as u64,
                IconState::Throttled { until, .. } => {
                    Instant::now().saturating_duration_since(*until).as_millis() as u64
                }
            };
            leftover.push((max_side, url.clone(), rank));
        }
    }
    if leftover.len() <= MAX_META_ENTRIES {
        return;
    }
    leftover.sort_unstable_by_key(|(_, _, age)| *age);
    for (max_side, url, _) in leftover.into_iter().skip(MAX_META_ENTRIES) {
        if let Some(bucket) = entries.get_mut(&max_side) {
            bucket.remove(&url);
        }
    }
}
fn evict_stale(
    entries: &mut HashMap<u32, Bucket>,
    ctx: &egui::Context,
    protect_after: u64,
    ready_count: &AtomicUsize,
    resident_bytes_atom: &AtomicUsize,
) {
    let mut by_age: Vec<(u32, String, u64, usize)> = entries
        .iter()
        .flat_map(|(size, bucket)| {
            bucket.iter().filter_map(move |(url, state)| match state {
                IconState::Ready { last_used, byte_size, .. } => {
                    Some((*size, url.clone(), *last_used, *byte_size))
                }
                _ => None,
            })
        })
        .collect();
    let mut resident_bytes: usize = by_age.iter().map(|(_, _, _, size)| *size).sum();
    let mut ready = by_age.len();
    ready_count.store(ready, Ordering::Relaxed);
    resident_bytes_atom.store(resident_bytes, Ordering::Relaxed);
    if resident_bytes <= MAX_RESIDENT_BYTES && ready <= MAX_RESIDENT_READY {
        return;
    }

    by_age.sort_unstable_by_key(|(_, _, last_used, _)| *last_used);

    let mut to_remove = Vec::new();
    for (max_side, url, last_used, byte_size) in by_age {
        if resident_bytes <= MAX_RESIDENT_BYTES && ready <= MAX_RESIDENT_READY {
            break;
        }
        if last_used >= protect_after {
            continue;
        }
        ctx.forget_image(&texture_name(&url, max_side));
        resident_bytes = resident_bytes.saturating_sub(byte_size);
        ready = ready.saturating_sub(1);
        to_remove.push((max_side, url));
    }

    for (max_side, url) in to_remove {
        if let Some(bucket) = entries.get_mut(&max_side) {
            bucket.remove(&url);
        }
    }
    ready_count.store(ready, Ordering::Relaxed);
    resident_bytes_atom.store(resident_bytes, Ordering::Relaxed);
}
fn scan_disk_index() -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(root) = std::fs::read_dir(CACHE_DIR) else {
        return out;
    };
    for entry in root.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let prefix = entry.file_name();
        let prefix = prefix.to_string_lossy();
        if prefix.len() != 2 {
            continue;
        }
        let Ok(files) = std::fs::read_dir(entry.path()) else { continue };
        for file in files.flatten() {
            let Ok(ft) = file.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let name = file.file_name();
            let name = name.to_string_lossy();
            if name.len() == 30 && name.chars().all(|c| c.is_ascii_hexdigit()) {
                out.insert(format!("{prefix}{name}"));
            }
        }
    }
    out
}
fn cache_digest(url: &str) -> Option<String> {
    use md5::Digest;
    let clean_url = url.split('?').next().unwrap_or(url);
    if clean_url.rsplit('/').next().is_none_or(str::is_empty) {
        return None;
    }
    let digest = md5::Md5::digest(clean_url.as_bytes());
    Some(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn cache_path_for_digest(digest: &str) -> Option<PathBuf> {
    if digest.len() != 32 {
        return None;
    }
    Some(PathBuf::from(CACHE_DIR).join(&digest[..2]).join(&digest[2..]))
}
fn cache_path(url: &str) -> Option<PathBuf> {
    cache_digest(url).and_then(|d| cache_path_for_digest(&d))
}
enum LoadOutcome {
    Ready(egui::ColorImage),
    RateLimited { after: Duration },
    Deferred { retry_in: Duration },
    Failed,
}
async fn load_icon(url: &str, max_side: u32, cache: &IconCache, skip_disk: bool) -> LoadOutcome {
    let digest = cache_digest(url);
    let path = digest.as_ref().and_then(|d| cache_path_for_digest(d));
    let try_disk = !skip_disk
        && digest
            .as_ref()
            .is_some_and(|d| cache.disk_has(d));
    if try_disk {
        if let Some(path) = path.clone() {
            let cached = tokio::task::spawn_blocking(move || {
                let bytes = std::fs::read(&path).ok()?;
                if bytes.is_empty() {
                    let _ = std::fs::remove_file(&path);
                    return None;
                }
                let decoded = decode_image(&bytes, max_side);
                if decoded.is_none() {
                    eprintln!("cached image {} didn't decode, dropping it", path.display());
                    let _ = std::fs::remove_file(&path);
                }
                decoded
            })
            .await;
            match cached {
                Ok(Some(image)) => return LoadOutcome::Ready(image),
                Ok(None) => {
                    if let Some(d) = digest.as_ref() {
                        if let Ok(mut g) = cache.disk_index.lock() {
                            g.remove(d);
                        }
                    }
                }
                Err(err) => eprintln!("image cache worker crashed: {err}"),
            }
        }
    }
    let wait = match cache.try_reserve_slot() {
        Ok(wait) => wait,
        Err(retry_in) => return LoadOutcome::Deferred { retry_in },
    };
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
    let _permit = cache.fetch_limit.acquire().await;
    let bytes = match fetch_bytes(url).await {
        FetchOutcome::Ok(bytes) => bytes,
        FetchOutcome::RateLimited { after } => {
            cache.back_off(after);
            return LoadOutcome::RateLimited { after };
        }
        FetchOutcome::Failed if url.contains("drdecki.github.io/VitaDBtoo-db/") => {
            let fallback = url.replace(
                "drdecki.github.io/VitaDBtoo-db/",
                "raw.githubusercontent.com/DrDecki/VitaDBtoo-db/main/",
            );
            match fetch_bytes(&fallback).await {
                FetchOutcome::Ok(bytes) => bytes,
                FetchOutcome::RateLimited { after } => {
                    cache.back_off(after);
                    return LoadOutcome::RateLimited { after };
                }
                FetchOutcome::Failed => return LoadOutcome::Failed,
            }
        }
        FetchOutcome::Failed => {
            if let Some(fallback) = hexflow_cover_fallback(url) {
                match fetch_bytes(&fallback).await {
                    FetchOutcome::Ok(bytes) => bytes,
                    FetchOutcome::RateLimited { after } => {
                        cache.back_off(after);
                        return LoadOutcome::RateLimited { after };
                    }
                    FetchOutcome::Failed => return LoadOutcome::Failed,
                }
            } else {
                return LoadOutcome::Failed;
            }
        }
    };
    let digest_for_write = digest.clone();
    let decoded = tokio::task::spawn_blocking(move || {
        let Some(decoded) = decode_image(&bytes, max_side) else {
            eprintln!("couldn't decode the image ({} bytes)", bytes.len());
            return None;
        };
        Some((decoded, bytes, path))
    })
    .await;
    match decoded {
        Ok(Some((image, bytes, path))) => {
            if let Some(path) = path {
                let disk_index = cache.disk_index.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(err) = std::fs::write(&path, &bytes) {
                        eprintln!("couldn't cache {}: {err}", path.display());
                        return;
                    }
                    if let Some(d) = digest_for_write {
                        if let Ok(mut g) = disk_index.lock() {
                            g.insert(d);
                        }
                    }
                });
            }
            LoadOutcome::Ready(image)
        }
        Ok(None) => LoadOutcome::Failed,
        Err(err) => {
            eprintln!("image decode worker crashed for {url}: {err}");
            LoadOutcome::Failed
        }
    }
}

fn hexflow_cover_fallback(url: &str) -> Option<String> {
    if url.contains("/api/v1/images/cover/") {
        return None;
    }
    let title_id = url
        .rsplit('/')
        .next()
        .and_then(|name| name.split('.').next())
        .filter(|id| {
            let id = id.trim();
            (9..=12).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric())
        })?;
    if !(url.contains("/assets/covers/") || url.contains("/covers/")) {
        return None;
    }
    Some(format!(
        "https://vitaforge.josephinoo.dev/api/v1/images/cover/{title_id}?size=medium&format=jpeg"
    ))
}
enum FetchOutcome {
    Ok(bytes::Bytes),
    RateLimited { after: Duration },
    Failed,
}
fn throttle_delay(headers: &reqwest::header::HeaderMap) -> Duration {
    headers
        .get("x-ratelimit-after")
        .or_else(|| headers.get(reqwest::header::RETRY_AFTER))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_THROTTLE)
}
async fn fetch_bytes(url: &str) -> FetchOutcome {
    let res = match crate::net::client()
        .get(url)
        .header("X-Client-ID", crate::data::client_id::get())
        .send()
        .await
    {
        Ok(res) => res,
        Err(err) => {
            eprintln!("couldn't fetch {url}: {err}");
            return FetchOutcome::Failed;
        }
    };
    if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return FetchOutcome::RateLimited { after: throttle_delay(res.headers()) };
    }
    if !res.status().is_success() {
        eprintln!("fetching {url} returned {}", res.status());
        return FetchOutcome::Failed;
    }
    match res.bytes().await {
        Ok(bytes) => FetchOutcome::Ok(bytes),
        Err(err) => {
            eprintln!("the body of {url} never arrived: {err}");
            FetchOutcome::Failed
        }
    }
}
#[cfg(target_os = "vita")]
unsafe extern "C" {
    fn vita_decode_jpeg_hw(
        jpeg_data: *const u8,
        size: usize,
        out_rgba: *mut u8,
        out_max_bytes: usize,
        out_w: *mut i32,
        out_h: *mut i32,
    ) -> i32;

    fn vita_decode_png_fast(
        png_data: *const u8,
        size: usize,
        out_rgba: *mut u8,
        out_max_bytes: usize,
        out_w: *mut i32,
        out_h: *mut i32,
    ) -> i32;
}

#[cfg(target_os = "vita")]
fn try_decode_fast_native(bytes: &[u8]) -> Option<egui::ColorImage> {
    if bytes.len() < 4 {
        return None;
    }
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    let max_bytes = 960 * 544 * 4;
    let mut rgba_buf = vec![0u8; max_bytes];

    if bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let res = unsafe {
            vita_decode_jpeg_hw(
                bytes.as_ptr(),
                bytes.len(),
                rgba_buf.as_mut_ptr(),
                max_bytes,
                &mut w as *mut i32,
                &mut h as *mut i32,
            )
        };
        if res >= 0 && w > 0 && h > 0 {
            let size = (w as usize) * (h as usize) * 4;
            if size <= rgba_buf.len() {
                rgba_buf.truncate(size);
                return Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba_buf));
            }
        }
    } else if bytes[0] == 0x89 && bytes[1] == b'P' && bytes[2] == b'N' && bytes[3] == b'G' {
        let res = unsafe {
            vita_decode_png_fast(
                bytes.as_ptr(),
                bytes.len(),
                rgba_buf.as_mut_ptr(),
                max_bytes,
                &mut w as *mut i32,
                &mut h as *mut i32,
            )
        };
        if res >= 0 && w > 0 && h > 0 {
            let size = (w as usize) * (h as usize) * 4;
            if size <= rgba_buf.len() {
                rgba_buf.truncate(size);
                return Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba_buf));
            }
        }
    }
    None
}

fn decode_image(bytes: &[u8], max_side: u32) -> Option<egui::ColorImage> {
    #[cfg(target_os = "vita")]
    if let Some(color_img) = try_decode_fast_native(bytes) {
        return Some(color_img);
    }

    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut decoded = match reader.decode() {
        Ok(decoded) => decoded,
        Err(err) => {
            eprintln!("image decoder rejected {} bytes: {err}", bytes.len());
            return None;
        }
    };
    if decoded.width() > max_side || decoded.height() > max_side {
        let filter = if max_side > MAX_ICON_SIDE {
            image::imageops::FilterType::Triangle
        } else {
            image::imageops::FilterType::Nearest
        };
        decoded = decoded.resize(max_side, max_side, filter);
    }
    let rgba = decoded.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};
    #[test]
    fn reads_the_servers_rate_limit_hint() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-after", HeaderValue::from_static("2"));
        assert_eq!(throttle_delay(&headers), Duration::from_secs(2));
    }
    #[test]
    fn falls_back_to_retry_after_then_to_a_default() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, HeaderValue::from_static("7"));
        assert_eq!(throttle_delay(&headers), Duration::from_secs(7));
        assert_eq!(throttle_delay(&HeaderMap::new()), DEFAULT_THROTTLE);
    }
    #[test]
    fn a_throttled_entry_stays_retriable() {
        let throttled = IconState::Throttled { until: Instant::now(), attempts: 0 };
        assert!(throttled.retriable());
        let exhausted = IconState::Failed { at: Instant::now() - RETRY_AFTER, attempts: MAX_ATTEMPTS };
        assert!(!exhausted.retriable());
    }
    #[test]
    fn requests_are_spaced_out() {
        let cache = IconCache::new();
        assert!(cache.try_reserve_slot().expect("first slot is free").is_zero());
        let second = cache.try_reserve_slot().expect("second slot is still within the queue");
        assert!(second >= REQUEST_SPACING - Duration::from_millis(5), "got {second:?}");
    }
    #[test]
    fn the_launch_queue_stops_growing_instead_of_parking_everyone() {
        let cache = IconCache::new();
        let mut granted = 0;
        for _ in 0..1000 {
            match cache.try_reserve_slot() {
                Ok(wait) => {
                    assert!(wait <= MAX_QUEUE_AHEAD, "queued {wait:?} into the future");
                    granted += 1;
                }
                Err(_) => break,
            }
        }
        assert!(granted < 1000, "the queue accepted every request");
        assert!(granted >= 2, "the queue should accept a burst first, got {granted}");
        assert!(cache.try_reserve_slot().is_err());
    }
    #[test]
    fn one_size_shares_the_cached_bytes_of_another() {
        let icon = cache_path("https://x/api/v1/images/screenshot/abc.png?size=medium&format=jpeg");
        let hero = cache_path("https://x/api/v1/images/screenshot/abc.png?size=thumb&format=png");
        assert_eq!(icon, hero);
    }
    #[test]
    fn two_games_first_screenshots_do_not_collide() {
        let gow = cache_path("https://x/scraped_assets/psvita/god_of_war/screenshots/01.jpeg");
        let inc = cache_path("https://x/scraped_assets/psvita/stealth_inc_2/screenshots/01.jpeg");
        assert!(gow.is_some());
        assert_ne!(gow, inc);
    }
    #[test]
    fn hero_and_icon_use_distinct_texture_name_sizes() {
        let a = texture_name("https://x/icon.png", MAX_ICON_SIDE);
        let b = texture_name("https://x/icon.png", HERO_SIDE);
        assert_ne!(a, b);
    }

    #[test]
    fn vitadeck_asset_cover_falls_back_to_hexflow() {
        let url = "https://vitadeck.josephinoo.dev/assets/covers/PCSB00170.png";
        let fb = hexflow_cover_fallback(url).expect("fallback");
        assert!(fb.contains("/images/cover/PCSB00170"), "{fb}");
        assert!(hexflow_cover_fallback(&fb).is_none());
    }

    #[test]
    fn a_huge_server_backoff_cannot_park_every_tile_for_minutes() {
        let cache = IconCache::new();
        cache.back_off(Duration::from_secs(600));
        let gate = *cache.gate.lock().unwrap();
        assert!(
            gate <= Instant::now() + MAX_BACKOFF + Duration::from_secs(1),
            "a single 429 pushed the gate past the clamp"
        );
    }

    #[test]
    fn a_dropped_gpu_texture_is_forgotten_so_the_art_can_be_rebuilt() {
        let ctx = egui::Context::default();
        let cache = IconCache::new();
        let texture = ctx.load_texture(
            texture_name("https://x/a.png", MAX_ICON_SIDE),
            egui::ColorImage::new([2, 2], egui::Color32::RED),
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        cache
            .entries
            .lock()
            .unwrap()
            .entry(MAX_ICON_SIDE)
            .or_default()
            .insert("https://x/a.png".to_owned(), IconState::Ready { texture, last_used: 1, byte_size: 16 });
        assert!(cache.peek("https://x/a.png").is_some());

        cache.forget_textures(&ctx, &[id]);
        assert!(cache.peek("https://x/a.png").is_none(), "stale Ready entry survived");

        cache.forget_textures(&ctx, &[id]);
    }

    #[test]
    fn exhausted_failures_are_remembered_instead_of_being_refetched_forever() {
        let mut entries: HashMap<u32, Bucket> = HashMap::new();
        entries.entry(MAX_ICON_SIDE).or_default().insert(
            "https://x/missing.jpg".to_owned(),
            IconState::Failed { at: Instant::now(), attempts: MAX_ATTEMPTS },
        );
        prune_meta(&mut entries);
        assert!(
            entries[&MAX_ICON_SIDE].contains_key("https://x/missing.jpg"),
            "the record that suppresses the re-request was pruned"
        );

        let fresh = IconState::Failed { at: Instant::now(), attempts: MAX_ATTEMPTS };
        assert!(!fresh.retriable(), "an exhausted URL must cool down before being tried again");
        let cold = IconState::Failed {
            at: Instant::now() - EXHAUSTED_RETRY_AFTER,
            attempts: MAX_ATTEMPTS,
        };
        assert!(cold.retriable(), "the cooldown must eventually expire, not be permanent");
    }

    #[test]
    fn meta_eviction_forgets_the_oldest_records_not_the_freshest() {
        let mut entries: HashMap<u32, Bucket> = HashMap::new();
        let bucket = entries.entry(MAX_ICON_SIDE).or_default();
        for i in 0..(MAX_META_ENTRIES + 1) {
            bucket.insert(
                format!("https://x/{i}.jpg"),
                IconState::Failed {
                    at: Instant::now() - Duration::from_secs((MAX_META_ENTRIES - i) as u64),
                    attempts: MAX_ATTEMPTS,
                },
            );
        }
        prune_meta(&mut entries);
        let bucket = &entries[&MAX_ICON_SIDE];
        assert_eq!(bucket.len(), MAX_META_ENTRIES);
        assert!(!bucket.contains_key("https://x/0.jpg"), "the oldest record should be dropped");
        assert!(
            bucket.contains_key(&format!("https://x/{}.jpg", MAX_META_ENTRIES)),
            "the freshest record — the one still suppressing a re-request — was dropped"
        );
    }


    #[test]
    fn a_429_parks_both_lanes_and_reports_the_wait() {
        let cache = IconCache::new();
        assert!(cache.rate_limited_for().is_none());
        assert!(cache.try_reserve_slot().is_ok());

        cache.back_off(Duration::from_secs(300));

        let left = cache.rate_limited_for().expect("the window should be open");
        assert!(left > Duration::from_secs(290), "got {left:?}");

        let live = cache.try_reserve_slot().expect_err("the live lane must be closed");
        assert!(live > Duration::from_secs(290), "got {live:?}");
        let precache =
            cache.try_reserve_precache_slot().expect_err("the precache lane must be closed too");
        assert!(precache > Duration::from_secs(290), "got {precache:?}");
    }

    #[test]
    fn the_server_backoff_is_honoured_not_shortened() {
        let cache = IconCache::new();
        cache.back_off(Duration::from_secs(358));
        let left = cache.rate_limited_for().expect("window open");
        assert!(left > Duration::from_secs(350), "backoff was shortened to {left:?}");
    }

    #[test]
    fn a_throttled_tile_does_not_claim_to_be_loading() {
        let parked = IconState::Throttled {
            until: Instant::now() + Duration::from_secs(300),
            attempts: 1,
        };
        assert!(parked.retriable(), "it should still be retried eventually");

        let cache = IconCache::new();
        cache
            .entries
            .lock()
            .unwrap()
            .entry(MAX_ICON_SIDE)
            .or_default()
            .insert("https://x/a.png".to_owned(), parked);
        assert!(!cache.is_loading("https://x/a.png", MAX_ICON_SIDE));
    }
}
