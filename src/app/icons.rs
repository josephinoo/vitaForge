use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

const CACHE_DIR: &str = "ux0:data/vitaforge/cache";
const MAX_CONCURRENT_FETCHES: usize = 3;

const MAX_CONCURRENT_LARGE_FETCHES: usize = 1;
const MAX_ICON_SIDE: u32 = 128;
pub const MAX_SCREENSHOT_SIDE: u32 = 256;
const MAX_RESIDENT_BYTES: usize = 16 * 1024 * 1024;

const RETRY_AFTER: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: u32 = 3;

enum IconState {
    Loading,
    Ready { texture: egui::TextureHandle, last_used: u64, byte_size: usize },
    Failed { at: Instant, attempts: u32 },
}

impl IconState {
    fn retriable(&self) -> bool {
        matches!(self, IconState::Failed { at, attempts }
            if *attempts < MAX_ATTEMPTS && at.elapsed() >= RETRY_AFTER)
    }
}

type Bucket = HashMap<String, IconState>;

#[derive(Clone)]
pub struct IconCache {
    entries: Arc<Mutex<HashMap<u32, Bucket>>>,
    fetch_limit: Arc<Semaphore>,
    large_fetch_limit: Arc<Semaphore>,
    clock: Arc<Mutex<u64>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES)),
            large_fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_LARGE_FETCHES)),
            clock: Arc::new(Mutex::new(0)),
        }
    }

    fn tick(&self) -> u64 {
        let mut clock = self.clock.lock().unwrap();
        *clock += 1;
        *clock
    }

    pub fn is_loading(&self, url: &str) -> bool {
        let entries = self.entries.lock().unwrap();
        entries.values().any(|bucket| match bucket.get(url) {
            Some(IconState::Loading) => true,
            Some(state) => state.retriable(),
            None => false,
        })
    }

    pub fn get(&self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        self.get_sized(ctx, url, MAX_ICON_SIDE)
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
            let image = load_icon(&url, max_side).await;
            let stamp = cache.tick();
            let mut entries = cache.entries.lock().unwrap();
            let bucket = entries.entry(max_side).or_default();
            match image {
                Some(color_image) => {
                    let [w, h] = color_image.size;
                    let byte_size = w * h * 4;
                    let texture = ctx.load_texture(&url, color_image, egui::TextureOptions::LINEAR);
                    bucket.insert(url, IconState::Ready { texture, last_used: stamp, byte_size });
                    evict_stale(&mut entries, &ctx);
                }
                None => {
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

fn cache_path(url: &str) -> Option<PathBuf> {
    let mut segments = url.rsplit('/');
    let name = segments.next()?;
    if name.is_empty() {
        return None;
    }
    let subdir = segments.next().filter(|s| !s.is_empty()).unwrap_or("misc");
    Some(PathBuf::from(CACHE_DIR).join(subdir).join(name))
}

async fn load_icon(url: &str, max_side: u32) -> Option<egui::ColorImage> {
    let path = cache_path(url);

    if let Some(path) = path.clone() {
        let cached = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&path).ok()?;
            if bytes.is_empty() {
                let _ = std::fs::remove_file(&path);
                return None;
            }
            let decoded = decode_icon(&bytes, max_side);
            if decoded.is_none() {
                eprintln!("cached image {} didn't decode, dropping it", path.display());
                let _ = std::fs::remove_file(&path);
            }
            decoded
        })
        .await;
        match cached {
            Ok(Some(image)) => return Some(image),
            Ok(None) => {}
            Err(err) => eprintln!("image cache worker crashed: {err}"),
        }
    }

    let bytes = match fetch_bytes(url).await {
        Some(b) => Some(b),
        None if url.contains("drdecki.github.io/VitaDBtoo-db/") => {
            let fallback = url.replace(
                "drdecki.github.io/VitaDBtoo-db/",
                "raw.githubusercontent.com/DrDecki/VitaDBtoo-db/main/",
            );
            fetch_bytes(&fallback).await
        }
        None => None,
    }?;

    let decoded = tokio::task::spawn_blocking(move || {
        let Some(decoded) = decode_icon(&bytes, max_side) else {
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
        Ok(image) => image,
        Err(err) => {
            eprintln!("image decode worker crashed for {url}: {err}");
            None
        }
    }
}

async fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    let res = match crate::net::client().get(url).send().await {
        Ok(res) => res,
        Err(err) => {
            eprintln!("couldn't fetch {url}: {err}");
            return None;
        }
    };
    if !res.status().is_success() {
        eprintln!("fetching {url} returned {}", res.status());
        return None;
    }
    match res.bytes().await {
        Ok(bytes) => Some(bytes.to_vec()),
        Err(err) => {
            eprintln!("the body of {url} never arrived: {err}");
            None
        }
    }
}

fn decode_icon(bytes: &[u8], max_side: u32) -> Option<egui::ColorImage> {
    let mut decoded = match image::load_from_memory(bytes) {
        Ok(decoded) => decoded,
        Err(err) => {
            eprintln!("image decoder rejected {} bytes: {err}", bytes.len());
            return None;
        }
    };
    if decoded.width() > max_side || decoded.height() > max_side {
        decoded = decoded.thumbnail(max_side, max_side);
    }
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
