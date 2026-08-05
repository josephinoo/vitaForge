use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

const CACHE_DIR: &str = "ux0:data/vitaforge/cache";
const MAX_CONCURRENT_FETCHES: usize = 2;

const MAX_CONCURRENT_LARGE_FETCHES: usize = 1;
pub const MAX_ICON_SIDE: u32 = 128;
pub const MAX_SCREENSHOT_SIDE: u32 = 256;
/// Detail backdrops live in their own small bucket: upscaling them to the 960px
/// screen with linear filtering *is* the blur, with no extra draw passes. The
/// value sets how strong that blur is — 64px meant a 15x stretch, which came
/// out as unreadable mush, so this keeps enough of the image to recognise.
pub const HERO_SIDE: u32 = 160;
/// Budget for decoded images held in memory. Each one also becomes a separate
/// SDL texture in the console's video memory, which is far scarcer than main
/// RAM — 16 MiB of live textures was enough to make allocations start failing.
const MAX_RESIDENT_BYTES: usize = 8 * 1024 * 1024;

const RETRY_AFTER: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: u32 = 3;

/// The catalog server allows ~100 image requests per window and answers bursts
/// with 429. Leaving this much room between launches keeps a fast scroll (which
/// can queue dozens of tiles at once) under the limit instead of tripping it.
const REQUEST_SPACING: Duration = Duration::from_millis(60);
/// How far ahead the launch queue may run before new requests are turned away
/// rather than parked. Keeps the backlog to roughly a screenful.
const MAX_QUEUE_AHEAD: Duration = Duration::from_millis(750);
/// Used when a 429 arrives without a usable `x-ratelimit-after` header.
const DEFAULT_THROTTLE: Duration = Duration::from_secs(2);
/// How soon a request turned away by a full queue may try again.
const RETRY_DEFERRED: Duration = Duration::from_millis(150);

enum IconState {
    Loading,
    Ready { texture: egui::TextureHandle, last_used: u64, byte_size: usize },
    Failed { at: Instant, attempts: u32 },
    /// Rate-limited by the server, or turned away by a full launch queue.
    /// Deliberately separate from `Failed`: neither says anything about the
    /// image, so neither may burn one of the attempts.
    Throttled { until: Instant, attempts: u32 },
}

impl IconState {
    fn retriable(&self) -> bool {
        match self {
            IconState::Failed { at, attempts } => {
                *attempts < MAX_ATTEMPTS && at.elapsed() >= RETRY_AFTER
            }
            IconState::Throttled { .. } => true,
            _ => false,
        }
    }
}

type Bucket = HashMap<String, IconState>;

#[derive(Clone)]
pub struct IconCache {
    entries: Arc<Mutex<HashMap<u32, Bucket>>>,
    fetch_limit: Arc<Semaphore>,
    large_fetch_limit: Arc<Semaphore>,
    clock: Arc<Mutex<u64>>,
    /// Shared launch gate: the instant the next request may go out. A 429 pushes
    /// it forward so every in-flight task backs off together rather than each
    /// one spending its own 429 to discover the same thing.
    gate: Arc<Mutex<Instant>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES)),
            large_fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_LARGE_FETCHES)),
            clock: Arc::new(Mutex::new(0)),
            gate: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn tick(&self) -> u64 {
        let mut clock = self.clock.lock().unwrap();
        *clock += 1;
        *clock
    }

    /// Claims the next launch slot, or `None` when the queue is already backed
    /// up that far.
    ///
    /// Bailing out matters more than it looks: a caller that waits its turn
    /// holds a slot for a tile that may be long gone off screen. Scrolling a
    /// 8500-entry grid queued thousands of those, served first-come, so the
    /// artwork actually on screen sat behind minutes of stale work and the grid
    /// simply never finished loading. Giving up instead costs nothing — the
    /// tile asks again on the next frame if it is still visible.
    fn try_reserve_slot(&self) -> Option<Duration> {
        let mut gate = self.gate.lock().unwrap();
        let now = Instant::now();
        let start = (*gate).max(now);
        let wait = start - now;
        if wait > MAX_QUEUE_AHEAD {
            return None;
        }
        *gate = start + REQUEST_SPACING;
        Some(wait)
    }

    /// Pushes the shared gate out after a 429 so nobody retries into the wall.
    fn back_off(&self, after: Duration) {
        let mut gate = self.gate.lock().unwrap();
        *gate = (*gate).max(Instant::now() + after);
    }

    pub fn is_loading(&self, url: &str, max_side: u32) -> bool {
        let entries = self.entries.lock().unwrap();
        match entries.get(&max_side).and_then(|bucket| bucket.get(url)) {
            Some(IconState::Loading) => true,
            Some(state) => state.retriable(),
            None => false,
        }
    }

    /// How long the UI may sleep before this image is worth another frame.
    ///
    /// `Some(ZERO)` means something is genuinely in flight and the spinner
    /// should animate. A throttled image is only waiting for a clock to run
    /// out, so the frame loop can idle until then instead of redrawing the
    /// whole screen sixty times a second to show the exact same picture.
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

    /// Fetches the blurred backdrop variant of `url`. Because [`cache_path`]
    /// ignores the query string, this shares the on-disk bytes with the
    /// screenshot bucket — the second resolution costs a disk read, not a
    /// request, whichever one is asked for first.
    pub fn get_hero(&self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        self.get_sized(ctx, url, HERO_SIDE)
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
        // A throttled entry already went looking on disk and came up empty, so
        // its retry can skip straight to the network. Worth the bookkeeping:
        // the console's storage is slow and a screenful of retrying tiles would
        // otherwise re-probe it several times a second.
        let mut skip_disk = false;
        if let Some(state) = bucket.get_mut(url) {
            match state {
                IconState::Ready { texture, last_used, .. } => {
                    *last_used = now;
                    return Some(texture.clone());
                }
                IconState::Loading => return None,
                IconState::Failed { at, attempts: tries } => {
                    if *tries >= MAX_ATTEMPTS || at.elapsed() < RETRY_AFTER {
                        return None;
                    }
                    attempts = *tries;
                }
                IconState::Throttled { until, attempts: tries } => {
                    if Instant::now() < *until {
                        return None;
                    }
                    attempts = *tries;
                    skip_disk = true;
                }
            }
        }
        bucket.insert(url.to_owned(), IconState::Loading);
        drop(entries);

        let cache = self.clone();
        let ctx = ctx.clone();
        let url = url.to_owned();
        tokio::spawn(async move {
            let large = max_side > MAX_ICON_SIDE;
            let _big_permit = if large { cache.large_fetch_limit.acquire().await.ok() } else { None };
            let _permit = cache.fetch_limit.acquire().await;
            let outcome = load_icon(&url, max_side, &cache, skip_disk).await;
            let stamp = cache.tick();
            let mut entries = cache.entries.lock().unwrap();
            let bucket = entries.entry(max_side).or_default();
            match outcome {
                LoadOutcome::Ready(color_image) => {
                    let [w, h] = color_image.size;
                    let byte_size = w * h * 4;
                    let texture = ctx.load_texture(&url, color_image, egui::TextureOptions::LINEAR);
                    bucket.insert(url, IconState::Ready { texture, last_used: stamp, byte_size });
                    evict_stale(&mut entries, &ctx);
                }
                LoadOutcome::Deferred => {
                    // Re-arm almost immediately; whether it actually goes out
                    // next time depends on the tile still being on screen.
                    bucket.insert(
                        url,
                        IconState::Throttled { until: Instant::now() + RETRY_DEFERRED, attempts },
                    );
                }
                LoadOutcome::RateLimited { after } => {
                    // Not a failure: keep `attempts` where it was so a burst of
                    // 429s can't permanently retire an image that is perfectly
                    // fine, which is exactly what used to blank out the grid.
                    bucket.insert(url, IconState::Throttled { until: Instant::now() + after, attempts });
                }
                LoadOutcome::Failed => {
                    let attempts = attempts + 1;
                    if attempts >= MAX_ATTEMPTS {
                        eprintln!("giving up on {url} after {attempts} attempts");
                    }
                    bucket.insert(url, IconState::Failed { at: Instant::now(), attempts });
                }
            }
            drop(entries);
            ctx.request_repaint();
        });
        None
    }
}

fn evict_stale(entries: &mut HashMap<u32, Bucket>, ctx: &egui::Context) {
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

    let mut resident_bytes: usize = by_age.iter().map(|(_, _, _, size)| size).sum();
    if resident_bytes <= MAX_RESIDENT_BYTES {
        return;
    }

    by_age.sort_by_key(|(_, _, last_used, _)| *last_used);

    for (max_side, url, _, byte_size) in by_age {
        if resident_bytes <= MAX_RESIDENT_BYTES {
            break;
        }
        let Some(bucket) = entries.get_mut(&max_side) else { continue };
        if let Some(IconState::Ready { texture, .. }) = bucket.remove(&url) {
            ctx.forget_image(&url);
            drop(texture);
            resident_bytes = resident_bytes.saturating_sub(byte_size);
        }
    }
}

/// Where `url` is cached on disk.
///
/// Keyed by a digest of the whole path rather than its last two segments,
/// because the scraped catalog assets are laid out as
/// `.../<game>/screenshots/01.jpeg` — every game's first screenshot used to
/// land on the same `screenshots/01.jpeg` file and serve some other game's art.
///
/// The query string is deliberately excluded: the same asset requested at
/// different sizes shares one download, and the second resolution is decoded
/// straight from disk.
fn cache_path(url: &str) -> Option<PathBuf> {
    use md5::Digest;

    let clean_url = url.split('?').next().unwrap_or(url);
    if clean_url.rsplit('/').next().is_none_or(str::is_empty) {
        return None;
    }
    let digest = md5::Md5::digest(clean_url.as_bytes());
    let name = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    // Fan out over 256 directories; the console's filesystem gets slow when a
    // single directory holds thousands of entries.
    Some(PathBuf::from(CACHE_DIR).join(&name[..2]).join(&name[2..]))
}

enum LoadOutcome {
    Ready(egui::ColorImage),
    RateLimited { after: Duration },
    /// Turned away because the launch queue was full. Not a failure at all —
    /// the tile just tries again shortly.
    Deferred,
    Failed,
}

async fn load_icon(url: &str, max_side: u32, cache: &IconCache, skip_disk: bool) -> LoadOutcome {
    let path = cache_path(url);

    if let Some(path) = path.clone().filter(|_| !skip_disk) {
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
            Ok(None) => {}
            Err(err) => eprintln!("image cache worker crashed: {err}"),
        }
    }

    // Nothing on disk, so this one has to go out over the wire — claim a turn
    // at the shared launch gate, or step aside if the queue is already full.
    let Some(wait) = cache.try_reserve_slot() else {
        return LoadOutcome::Deferred;
    };
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }

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
        FetchOutcome::Failed => return LoadOutcome::Failed,
    };

    let decoded = tokio::task::spawn_blocking(move || {
        let Some(decoded) = decode_image(&bytes, max_side) else {
            eprintln!("couldn't decode the image ({} bytes)", bytes.len());
            return None;
        };
        if let Some(path) = &path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(err) = std::fs::write(path, &bytes) {
                eprintln!("couldn't cache {}: {err}", path.display());
            }
        }
        Some(decoded)
    })
    .await;

    match decoded {
        Ok(Some(image)) => LoadOutcome::Ready(image),
        Ok(None) => LoadOutcome::Failed,
        Err(err) => {
            eprintln!("image decode worker crashed for {url}: {err}");
            LoadOutcome::Failed
        }
    }
}

enum FetchOutcome {
    Ok(Vec<u8>),
    RateLimited { after: Duration },
    Failed,
}

/// The server signals its rate limit with `x-ratelimit-after` (whole seconds);
/// `Retry-After` is honoured too in case that ever changes.
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
    let res = match crate::net::client().get(url).send().await {
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
        Ok(bytes) => FetchOutcome::Ok(bytes.to_vec()),
        Err(err) => {
            eprintln!("the body of {url} never arrived: {err}");
            FetchOutcome::Failed
        }
    }
}

fn decode_image(bytes: &[u8], max_side: u32) -> Option<egui::ColorImage> {
    let mut decoded = match image::load_from_memory(bytes) {
        Ok(decoded) => decoded,
        Err(err) => {
            eprintln!("image decoder rejected {} bytes: {err}", bytes.len());
            return None;
        }
    };
    if decoded.width() > max_side || decoded.height() > max_side {
        // Backdrops average their pixels down (that averaging *is* the blur);
        // icons keep the cheap nearest-neighbour path.
        let filter = if max_side <= HERO_SIDE {
            image::imageops::FilterType::Triangle
        } else {
            image::imageops::FilterType::Nearest
        };
        decoded = decoded.resize(max_side, max_side, filter);
    }
    let rgba = decoded.to_rgba8();
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
        // ...whereas an exhausted failure does not.
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
        // Scrolling the grid used to queue one slot per tile with no ceiling,
        // so the artwork on screen waited behind minutes of stale requests and
        // the grid never finished loading.
        let cache = IconCache::new();
        let mut granted = 0;
        for _ in 0..1000 {
            match cache.try_reserve_slot() {
                Some(wait) => {
                    assert!(wait <= MAX_QUEUE_AHEAD, "queued {wait:?} into the future");
                    granted += 1;
                }
                None => break,
            }
        }
        assert!(granted < 1000, "the queue accepted every request");
        assert!(granted >= 2, "the queue should accept a burst first, got {granted}");
        // Once full it keeps refusing rather than parking the caller.
        assert!(cache.try_reserve_slot().is_none());
    }

    #[test]
    fn one_size_shares_the_cached_bytes_of_another() {
        let icon = cache_path("https://x/api/v1/images/screenshot/abc.png?size=medium&format=jpeg");
        let hero = cache_path("https://x/api/v1/images/screenshot/abc.png?size=thumb&format=png");
        assert_eq!(icon, hero);
    }

    #[test]
    fn two_games_first_screenshots_do_not_collide() {
        // Same trailing segments, different games — these used to share one file.
        let gow = cache_path("https://x/scraped_assets/psvita/god_of_war/screenshots/01.jpeg");
        let inc = cache_path("https://x/scraped_assets/psvita/stealth_inc_2/screenshots/01.jpeg");
        assert!(gow.is_some());
        assert_ne!(gow, inc);
    }
}
