use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

const CACHE_DIR: &str = "ux0:data/vitaforge/cache";
const MAX_CONCURRENT_FETCHES: usize = 3;
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_ICON_SIDE: u32 = 64;
pub const MAX_SCREENSHOT_SIDE: u32 = 256;
const MAX_RESIDENT_BYTES: usize = 6 * 1024 * 1024;

enum IconState {
    Loading,
    Ready { texture: egui::TextureHandle, last_used: u64, byte_size: usize },
    Failed,
}

#[derive(Clone)]
pub struct IconCache {
    entries: Arc<Mutex<HashMap<String, IconState>>>,
    fetch_limit: Arc<Semaphore>,
    clock: Arc<Mutex<u64>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            fetch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES)),
            clock: Arc::new(Mutex::new(0)),
        }
    }

    fn tick(&self) -> u64 {
        let mut clock = self.clock.lock().unwrap();
        *clock += 1;
        *clock
    }

    pub fn is_loading(&self, url: &str) -> bool {
        matches!(self.entries.lock().unwrap().get(url), Some(IconState::Loading) | None)
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
        if let Some(state) = entries.get_mut(url) {
            return match state {
                IconState::Ready { texture, last_used, .. } => {
                    *last_used = now;
                    Some(texture.clone())
                }
                IconState::Loading | IconState::Failed => None,
            };
        }
        entries.insert(url.to_owned(), IconState::Loading);
        drop(entries);

        let cache = self.clone();
        let ctx = ctx.clone();
        let url = url.to_owned();
        tokio::spawn(async move {
            let _permit = cache.fetch_limit.acquire().await;
            let image = load_icon(&url, max_side).await;
            let stamp = cache.tick();
            let mut entries = cache.entries.lock().unwrap();
            match image {
                Some(color_image) => {
                    let [w, h] = color_image.size;
                    let byte_size = w * h * 4;
                    let texture = ctx.load_texture(&url, color_image, egui::TextureOptions::LINEAR);
                    entries.insert(url, IconState::Ready { texture, last_used: stamp, byte_size });
                    evict_stale(&mut entries, &ctx);
                }
                None => {
                    entries.insert(url, IconState::Failed);
                }
            }
            drop(entries);
            ctx.request_repaint();
        });
        None
    }
}

fn evict_stale(entries: &mut HashMap<String, IconState>, ctx: &egui::Context) {
    let mut by_age: Vec<(String, u64, usize)> = entries
        .iter()
        .filter_map(|(url, state)| match state {
            IconState::Ready { last_used, byte_size, .. } => Some((url.clone(), *last_used, *byte_size)),
            _ => None,
        })
        .collect();

    let mut resident_bytes: usize = by_age.iter().map(|(_, _, size)| size).sum();
    if resident_bytes <= MAX_RESIDENT_BYTES {
        return;
    }

    by_age.sort_by_key(|(_, last_used, _)| *last_used);

    for (url, _, byte_size) in by_age {
        if resident_bytes <= MAX_RESIDENT_BYTES {
            break;
        }
        if let Some(IconState::Ready { texture, .. }) = entries.remove(&url) {
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

    // Reading and decoding both stall the UI thread, so use the blocking pool.
    if let Some(path) = path.clone() {
        let cached = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&path).ok()?;
            decode_icon(&bytes, max_side)
        })
        .await
        .ok()
        .flatten();
        if let Some(image) = cached {
            return Some(image);
        }
    }

    // The shared client reuses the connection. A fresh one per icon meant a full
    // TLS handshake every time, which costs whole seconds here.
    let bytes = crate::net::client().get(url).send().await.ok()?.bytes().await.ok()?;

    tokio::task::spawn_blocking(move || {
        let decoded = decode_icon(&bytes, max_side)?;
        if let Some(path) = &path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, &bytes);
        }
        Some(decoded)
    })
    .await
    .ok()
    .flatten()
}

fn decode_icon(bytes: &[u8], max_side: u32) -> Option<egui::ColorImage> {
    let mut decoded = image::load_from_memory(bytes).ok()?;
    if decoded.width() > max_side || decoded.height() > max_side {
        decoded = decoded.thumbnail(max_side, max_side);
    }
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

