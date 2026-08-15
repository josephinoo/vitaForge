use super::client_id;
use super::{AppEntry, Category, Platform};
use serde::Deserialize;
const DEFAULT_API_URL: &str = "https://vitaforge.josephinoo.dev/api/v1";
pub fn api_url() -> &'static str {
    option_env!("SERVER_URL").unwrap_or(DEFAULT_API_URL)
}
fn endpoint(path: &str) -> String {
    format!("{}{path}", api_url().trim_end_matches('/'))
}

fn image_format_for(path: &str) -> &'static str {
    if path.contains("/images/icon/") {
        "png" // Icons preserve transparency
    } else {
        "jpeg" // Covers and screenshots use fast JPEG compression
    }
}
fn absolute(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_owned();
    }
    let url = api_url();
    let origin = url
        .find("://")
        .map(|scheme_end| scheme_end + 3)
        .and_then(|host_start| url[host_start..].find('/').map(|slash| host_start + slash))
        .map(|host_end| &url[..host_end])
        .unwrap_or(url);
    force_format(&format!("{origin}{path}"))
}

pub fn ensure_cover_fallback(entry: &mut AppEntry) {
    if entry.titleid.is_empty() {
        return;
    }
    let hexflow = absolute(&format!(
        "/api/v1/images/cover/{}?size=medium&format=jpeg",
        entry.titleid
    ));
    if entry.platform.is_commercial() {
        entry.cover_url = Some(hexflow);
        return;
    }
    if entry.cover_url.is_none() {
        entry.cover_url = Some(hexflow);
    }
}
fn force_format(url: &str) -> String {
    let wanted = image_format_for(url);
    let Some(start) = url.find("format=") else {
        let separator = if url.contains('?') { '&' } else { '?' };
        return format!("{url}{separator}format={wanted}");
    };
    let value_start = start + "format=".len();
    let value_end = url[value_start..].find('&').map_or(url.len(), |i| value_start + i);
    format!("{}{wanted}{}", &url[..value_start], &url[value_end..])
}
#[derive(Debug, Deserialize)]
struct RawApp {
    id: i64,
    #[serde(default)]
    title_id: Option<String>,
    #[serde(default)]
    content_id: Option<String>,
    name: String,
    #[serde(default)]
    original_name: Option<String>,
    #[serde(default)]
    overview: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
    #[serde(default)]
    requirements: Option<String>,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    release_page: Option<String>,
    #[serde(default)]
    category: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    icon_hash: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    cover_url: Option<String>,
    #[serde(default)]
    background_url: Option<String>,
    #[serde(default)]
    screenshot_urls: Vec<String>,
    #[serde(default)]
    download_url: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    zrif: Option<String>,
    #[serde(default)]
    source_catalog: Option<String>,
    #[serde(default)]
    source_labels: Vec<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    install_count: u64,
    #[serde(default)]
    average_rating: f32,
    #[serde(default)]
    ratings_count: u32,
    #[serde(default)]
    likes_count: u32,
    #[serde(default)]
    comments_count: u32,
    #[serde(default)]
    user_liked: bool,
    #[serde(default)]
    user_rating: Option<u8>,
    #[serde(default)]
    release_date: Option<String>,
}
const MAX_SCREENSHOTS_KEPT: usize = 8;
fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
fn slim_entry_strings(entry: &mut AppEntry) {
    if !entry.long_description.is_empty()
        && entry.long_description.trim() == entry.description.trim()
    {
        entry.long_description.clear();
        entry.long_description.shrink_to_fit();
    }
    if entry.screenshot_urls.len() > MAX_SCREENSHOTS_KEPT {
        entry.screenshot_urls.truncate(MAX_SCREENSHOTS_KEPT);
        entry.screenshot_urls.shrink_to_fit();
    }
    entry.description.shrink_to_fit();
    entry.long_description.shrink_to_fit();
    entry.requirements.shrink_to_fit();
    entry.changelog.shrink_to_fit();
    entry.download_url.shrink_to_fit();
}
impl RawApp {
    
    fn into_app_entry(self) -> Option<AppEntry> {
        if self.download_url.is_empty() || self.download_url.eq_ignore_ascii_case("missing") {
            return None;
        }
        let platform = Platform::from_api_type(&self.kind);
        if matches!(platform, Platform::Psp | Platform::NpsPsp | Platform::NpsPsx) {
            return None;
        }
        let category = Category::from_api(&self.category, &self.kind);
        if matches!(category, Category::PspGame | Category::Ps1Game) {
            return None;
        }
        let titleid = self.title_id.unwrap_or_default();
        let author = self.author
            .map(|a| if a.trim().is_empty() { "unknown".to_owned() } else { a })
            .unwrap_or_else(|| "unknown".to_owned());
        let description = self.description.unwrap_or_default();
        let long_description = self.long_description.unwrap_or_default();
        let requirements = self.requirements.unwrap_or_default();
        let changelog = self.changelog.unwrap_or_default();
        let version = self.version.unwrap_or_else(|| "1.0".to_owned());
        let source_catalog = self.source_catalog.unwrap_or_else(|| "vitadb".to_owned());
        let release_page = self.release_page.and_then(non_empty);
        let source = self.source.and_then(non_empty);
        let size_bytes = self.size
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let updated_at = self.release_date.unwrap_or_default();
        let has_valid_icon = self.icon_hash.as_ref().is_some_and(|h| h != "default");
        let icon_url = if has_valid_icon {
            self.icon_url.map(|u| absolute(&u))
        } else {
            None
        };
        let cover_url = self
            .cover_url
            .as_ref()
            .map(|u| absolute(u))
            .or_else(|| self.screenshot_urls.first().map(|u| absolute(u)))
            .or_else(|| {
                if titleid.is_empty() {
                    None
                } else {
                    Some(absolute(&format!(
                        "/api/v1/images/cover/{titleid}?size=medium&format=jpeg"
                    )))
                }
            });
        let mut entry = AppEntry {
            id: self.id.to_string(),
            titleid,
            titleid_lower: String::new(), 
            content_id: self.content_id.and_then(non_empty),
            name_lower: String::new(), 
            author_lower: String::new(), 
            name: self.name,
            original_name: self.original_name,
            overview: self.overview.map(|o| o.into_iter().collect()).unwrap_or_default(),
            author,
            description,
            long_description,
            requirements,
            changelog,
            release_page,
            category,
            platform,
            kind: self.kind.clone(),
            icon_url,
            cover_url,
            background_url: self.background_url.as_ref().map(|u| absolute(u)),
            screenshot_urls: self
                .screenshot_urls
                .iter()
                .take(MAX_SCREENSHOTS_KEPT)
                .map(|u| absolute(u))
                .collect(),
            download_url: self.download_url.clone(),
            source: source.filter(|s| s.contains("github.com")),
            version,
            region: self.region,
            zrif: self.zrif.and_then(non_empty),
            source_catalog,
            source_labels: self.source_labels,
            hash: String::new(),
            hash2: String::new(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes,
            downloads: self.install_count,
            rating: self.average_rating,
            updated_at,
            ratings_count: self.ratings_count,
            likes_count: self.likes_count,
            comments_count: self.comments_count,
            user_liked: self.user_liked,
            user_rating: self.user_rating,
        };
        entry.rebuild_derived();
        ensure_cover_fallback(&mut entry);
        slim_entry_strings(&mut entry);
        Some(entry)
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub author_name: String,
    pub content: String,
}
fn request(method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    crate::net::client().request(method, endpoint(path)).header("X-Client-ID", client_id::get())
}
#[derive(Debug, Deserialize)]
struct AppsPage {
    data: Vec<RawApp>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogVersionInfo {
    pub version: i64,
    pub total_apps: i64,
    #[serde(default)]
    pub etag: String,
}
#[allow(dead_code)]
const CATALOG_CACHE_PATH: &str = "ux0:data/vitaforge/catalog_cache.json";
#[allow(dead_code)]
const CATALOG_VERSION_PATH: &str = "ux0:data/vitaforge/catalog_version.json";
#[cfg(not(target_os = "vita"))]
const CATALOG_CACHE_PATH_HOST: &str = "data/vitaforge/catalog_cache.json";
#[cfg(not(target_os = "vita"))]
const CATALOG_VERSION_PATH_HOST: &str = "data/vitaforge/catalog_version.json";
fn get_catalog_cache_paths() -> (&'static str, &'static str) {
    #[cfg(target_os = "vita")]
    { (CATALOG_CACHE_PATH, CATALOG_VERSION_PATH) }
    #[cfg(not(target_os = "vita"))]
    { (CATALOG_CACHE_PATH_HOST, CATALOG_VERSION_PATH_HOST) }
}
pub fn load_cached_catalog_sync() -> Option<Vec<AppEntry>> {
    load_cached_catalog_blocking().map(|(entries, _)| drop_unavailable_platforms(entries))
}
fn load_cached_catalog_blocking() -> Option<(Vec<AppEntry>, CatalogVersionInfo)> {
    let (cache_path, version_path) = get_catalog_cache_paths();
    let version_bytes = std::fs::read(version_path).ok()?;
    let version_info: CatalogVersionInfo = serde_json::from_slice(&version_bytes).ok()?;
    let cache_bytes = std::fs::read(cache_path).ok()?;
    let mut entries: Vec<AppEntry> = serde_json::from_slice(&cache_bytes).ok()?;
    for entry in &mut entries {
        entry.rebuild_derived();
        ensure_cover_fallback(entry);
        slim_entry_strings(entry);
    }
    entries.shrink_to_fit();
    Some((entries, version_info))
}
fn save_cached_catalog_blocking(entries: &[AppEntry], version_info: CatalogVersionInfo) {
    let (cache_path, version_path) = get_catalog_cache_paths();
    if let Some(parent) = std::path::Path::new(cache_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(cache_bytes) = serde_json::to_vec(entries) {
        if std::fs::write(cache_path, cache_bytes).is_err() {
            return;
        }
    } else {
        return;
    }
    if let Ok(version_bytes) = serde_json::to_vec(&version_info) {
        let _ = std::fs::write(version_path, version_bytes);
    }
}
fn drop_unavailable_platforms(mut entries: Vec<AppEntry>) -> Vec<AppEntry> {
    entries.retain(|entry| !matches!(entry.platform, Platform::NpsPsp | Platform::NpsPsx));
    entries.shrink_to_fit();
    entries
}

fn catalog_has_vitadb_official(entries: &[AppEntry]) -> bool {
    use super::SourceCatalog;
    entries
        .iter()
        .any(|entry| SourceCatalog::VitaDb.matches(&entry.source_catalog))
}

fn catalog_has_nps(entries: &[AppEntry]) -> bool {
    use super::SourceCatalog;
    entries
        .iter()
        .any(|entry| SourceCatalog::Nps.matches(&entry.source_catalog))
}

fn catalog_cache_complete(entries: &[AppEntry]) -> bool {
    catalog_has_vitadb_official(entries) && catalog_has_nps(entries)
}

async fn fetch_remote_version() -> anyhow::Result<CatalogVersionInfo> {
    let response = request(reqwest::Method::GET, "/apps/version")
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("catalog version HTTP {}", response.status());
    }
    Ok(response.json().await?)
}

async fn fetch_remote_version_with_retry() -> anyhow::Result<CatalogVersionInfo> {
    match fetch_remote_version().await {
        Ok(info) => Ok(info),
        Err(first) => {
            eprintln!("catalog version failed ({first:#}); retrying once…");
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            fetch_remote_version().await
        }
    }
}

async fn fetch_snapshot(remote_ver: CatalogVersionInfo) -> anyhow::Result<Vec<AppEntry>> {
    eprintln!(
        "Fetching fresh catalog snapshot from server (version: {})...",
        remote_ver.version
    );
    let response = request(reqwest::Method::GET, "/apps/snapshot").send().await?;
    let page: AppsPage = response.json().await?;
    let entries: Vec<AppEntry> = page
        .data
        .into_iter()
        .filter_map(RawApp::into_app_entry)
        .collect();
    let entries = drop_unavailable_platforms(entries);
    let entries = tokio::task::spawn_blocking(move || {
        save_cached_catalog_blocking(&entries, remote_ver);
        entries
    })
    .await
    .map_err(|err| anyhow::anyhow!("catalog cache worker crashed: {err}"))?;
    Ok(entries)
}

pub async fn fetch_catalog() -> anyhow::Result<Vec<AppEntry>> {
    let cached = tokio::task::spawn_blocking(load_cached_catalog_blocking)
        .await
        .unwrap_or(None);
    let remote_version_result = fetch_remote_version_with_retry().await;
    match (cached, remote_version_result) {
        (Some((entries, local_ver)), Ok(remote_ver))
            if local_ver.version == remote_ver.version
                && local_ver.total_apps == remote_ver.total_apps
                && catalog_cache_complete(&entries) =>
        {
            eprintln!(
                "Catalog cache hit! (version: {}, apps: {}). Loaded {} entries instantly from disk.",
                local_ver.version, local_ver.total_apps, entries.len()
            );
            Ok(drop_unavailable_platforms(entries))
        }
        (_, Ok(remote_ver)) => fetch_snapshot(remote_ver).await,
        (Some((entries, local_ver)), Err(err)) if catalog_cache_complete(&entries) => {
            eprintln!(
                "Network offline/error ({err:#}). Using offline cached catalog (version: {}, {} entries).",
                local_ver.version,
                entries.len()
            );
            Ok(drop_unavailable_platforms(entries))
        }
        (cached, Err(err)) => {
            eprintln!(
                "catalog version unavailable ({err:#}); attempting snapshot without version gate…"
            );
            match request(reqwest::Method::GET, "/apps/snapshot").send().await {
                Ok(response) if response.status().is_success() => {
                    let page: AppsPage = response.json().await?;
                    let entries: Vec<AppEntry> = page
                        .data
                        .into_iter()
                        .filter_map(RawApp::into_app_entry)
                        .collect();
                    let entries = drop_unavailable_platforms(entries);
                    let version_info = CatalogVersionInfo {
                        version: cached.as_ref().map(|(_, v)| v.version).unwrap_or(0),
                        total_apps: entries.len() as i64,
                        etag: String::new(),
                    };
                    let entries = tokio::task::spawn_blocking(move || {
                        save_cached_catalog_blocking(&entries, version_info);
                        entries
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("catalog cache worker crashed: {e}"))?;
                    Ok(entries)
                }
                Ok(response) => {
                    anyhow::bail!(
                        "Failed to fetch catalog snapshot (HTTP {}) and no complete local cache: {err:#}",
                        response.status()
                    )
                }
                Err(snap_err) => {
                    anyhow::bail!(
                        "Failed to fetch catalog and no complete local cache (version: {err:#}; snapshot: {snap_err:#})"
                    )
                }
            }
        }
    }
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Social {
    #[serde(default)]
    pub average_rating: f32,
    #[serde(default)]
    pub ratings_count: u32,
    #[serde(default)]
    pub likes_count: u32,
    #[serde(default)]
    pub user_liked: bool,
    #[serde(default)]
    pub user_rating: Option<u8>,
}
pub async fn fetch_social(app_id: &str) -> anyhow::Result<Social> {
    Ok(request(reqwest::Method::GET, &format!("/apps/{app_id}")).send().await?.json().await?)
}
pub async fn fetch_comments(app_id: &str) -> anyhow::Result<Vec<Comment>> {
    let mut comments: Vec<Comment> =
        request(reqwest::Method::GET, &format!("/apps/{app_id}/comments")).send().await?.json().await?;
    for comment in &mut comments {
        crate::app::text::sanitize(&mut comment.author_name);
        crate::app::text::sanitize(&mut comment.content);
    }
    Ok(comments)
}
pub async fn post_comment(app_id: &str, author_name: &str, content: &str) -> anyhow::Result<()> {
    request(reqwest::Method::POST, &format!("/apps/{app_id}/comments"))
        .json(&serde_json::json!({ "author_name": author_name, "content": content }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
pub async fn post_rating(app_id: &str, score: u8) -> anyhow::Result<()> {
    request(reqwest::Method::POST, &format!("/apps/{app_id}/rate"))
        .json(&serde_json::json!({ "score": score }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
pub async fn set_like(app_id: &str, liked: bool) -> anyhow::Result<()> {
    let method = if liked { reqwest::Method::POST } else { reqwest::Method::DELETE };
    request(method, &format!("/apps/{app_id}/like")).send().await?.error_for_status()?;
    Ok(())
}
pub async fn notify_install(app_id: &str) -> anyhow::Result<()> {
    request(reqwest::Method::POST, &format!("/apps/{app_id}/install")).send().await?.error_for_status()?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RailPage {
    #[serde(default)]
    data: Vec<RailItem>,
}
#[derive(Debug, Deserialize)]
struct RailItem {
    id: i64,
}

async fn fetch_rail_ids(path: &str) -> anyhow::Result<Vec<String>> {
    let page: RailPage = request(reqwest::Method::GET, path)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(page.data.into_iter().map(|item| item.id.to_string()).collect())
}

pub async fn fetch_top_installed() -> anyhow::Result<Vec<String>> {
    fetch_rail_ids("/apps/top-installed").await
}

pub async fn fetch_recent(limit: u32, source: Option<&str>) -> anyhow::Result<Vec<String>> {
    let limit = limit.clamp(1, 100);
    let path = match source {
        Some(src) => format!("/apps/recent?limit={limit}&source={src}"),
        None => format!("/apps/recent?limit={limit}"),
    };
    fetch_rail_ids(&path).await
}

pub async fn fetch_discover_rails(source: Option<&str>) -> (Vec<String>, Vec<String>) {
    let (top, recent) = tokio::join!(fetch_top_installed(), fetch_recent(20, source));
    (
        top.unwrap_or_else(|err| {
            eprintln!("top-installed rail failed: {err:#}");
            Vec::new()
        }),
        recent.unwrap_or_else(|err| {
            eprintln!("recent rail failed: {err:#}");
            Vec::new()
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewrites_whatever_format_the_server_suggested() {
        assert_eq!(
            force_format("https://h/api/v1/images/icon/abc.png?size=medium&format=jpeg"),
            "https://h/api/v1/images/icon/abc.png?size=medium&format=png"
        );
        assert_eq!(
            force_format("https://h/api/v1/images/icon/abc.png?format=webp&size=medium"),
            "https://h/api/v1/images/icon/abc.png?format=png&size=medium"
        );
    }
    #[test]
    fn screenshots_stay_jpeg_and_gain_a_format_when_missing() {
        assert_eq!(
            force_format("https://h/api/v1/images/screenshot/a.png?size=medium&format=webp"),
            "https://h/api/v1/images/screenshot/a.png?size=medium&format=jpeg"
        );
        assert_eq!(
            force_format("https://h/scraped_assets/psvita/game/screenshots/01.jpeg"),
            "https://h/scraped_assets/psvita/game/screenshots/01.jpeg?format=jpeg"
        );
    }

    #[test]
    fn synthesizes_cover_url_when_catalog_omitted_it() {
        let mut entry = AppEntry {
            id: "1".into(),
            titleid: "PCSE00586".into(),
            titleid_lower: String::new(),
            content_id: None,
            name: "Akiba's Beat".into(),
            original_name: None,
            name_lower: String::new(),
            author: "x".into(),
            author_lower: String::new(),
            description: String::new(),
            long_description: String::new(),
            requirements: String::new(),
            changelog: String::new(),
            release_page: None,
            category: Category::PsVitaGame,
            platform: Platform::NpsVita,
            kind: "psv_game".into(),
            icon_url: None,
            cover_url: None,
            background_url: None,
            screenshot_urls: Vec::new(),
            download_url: "http://x".into(),
            source: None,
            version: "1.0".into(),
            region: None,
            zrif: None,
            source_catalog: "nps".into(),
            source_labels: Vec::new(),
            hash: String::new(),
            hash2: String::new(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 0,
            downloads: 0,
            rating: 0.0,
            updated_at: String::new(),
            ratings_count: 0,
            likes_count: 0,
            comments_count: 0,
            user_liked: false,
            user_rating: None,
            overview: Vec::new(),
        };
        ensure_cover_fallback(&mut entry);
        let url = entry.cover_url.clone().expect("cover synthesized");
        assert!(url.contains("/images/cover/PCSE00586"), "{url}");
        assert!(url.contains("format=jpeg"), "{url}");
        entry.cover_url = Some("https://vitadeck.josephinoo.dev/assets/covers/PCSE00586.png".into());
        ensure_cover_fallback(&mut entry);
        assert!(
            entry
                .cover_url
                .as_deref()
                .is_some_and(|u| u.contains("/images/cover/PCSE00586")),
            "{:?}",
            entry.cover_url
        );
    }
}
