use super::client_id;
use super::{AppEntry, Category, Platform};
use serde::Deserialize;

const DEFAULT_API_URL: &str = "https://vitaforge.josephinoo.dev/api/v1";
const PAGE_LIMIT: u32 = 100;
const MAX_PAGES: u32 = 200;

pub fn api_url() -> &'static str {
    option_env!("SERVER_URL").unwrap_or(DEFAULT_API_URL)
}

fn endpoint(path: &str) -> String {
    format!("{}{path}", api_url().trim_end_matches('/'))
}

/// The API returns image/asset URLs as host-relative paths (e.g. `/api/v1/images/...`).
/// Turns one into an absolute URL using the API's scheme+host, leaving already-absolute
/// URLs (e.g. a GitHub-hosted asset) untouched.
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

/// Pins the `format` query parameter to something the `image` crate is built to
/// decode. The server's own default has changed before (it now hands out
/// `format=jpeg` for icons), so rewrite whatever is there instead of matching on
/// one particular value — otherwise icons silently lose their transparency.
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
    name: String,
    #[serde(default)]
    original_name: Option<String>,
    /// NPS scrape metadata: publisher, developer, system, game rating.
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

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

impl RawApp {
    /// Returns `None` for entries whose download link is a dynamic PHP redirect
    /// (`get_hb_url.php`) rather than a direct asset — the installer can't stage
    /// those reliably, so they're excluded from the catalog just like before.
    fn into_app_entry(self) -> Option<AppEntry> {
        if self.download_url.is_empty() || self.download_url.contains("get_hb_url.php") {
            return None;
        }
        let platform = Platform::from_api_type(&self.kind);
        let category = Category::from_api(&self.category, &self.kind);
        
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

        let mut entry = AppEntry {
            id: self.id.to_string(),
            titleid,
            name_lower: String::new(), // filled in by `rebuild_derived` below
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
            icon_url,
            cover_url: self.cover_url.as_ref().map(|u| absolute(u)).or_else(|| self.screenshot_urls.first().map(|u| absolute(u))),
            background_url: self.background_url.as_ref().map(|u| absolute(u)),
            screenshot_urls: self.screenshot_urls.iter().map(|u| absolute(u)).collect(),
            download_url: if platform.is_nps() {
                endpoint(&format!("/apps/{}/download", self.id))
            } else {
                self.download_url.clone()
            },
            source: source.filter(|s| s.contains("github.com")),
            version,
            region: self.region,
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
        // Sanitizes every string and derives the search key. Also runs when the
        // catalog is restored from disk, so both paths agree on what a name is.
        entry.rebuild_derived();
        Some(entry)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    #[serde(default)]
    pub id: String,
    pub author_name: String,
    pub content: String,
    #[serde(default)]
    pub created_at: String,
}

fn request(method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    crate::net::client().request(method, endpoint(path)).header("X-Client-ID", client_id::get())
}

#[derive(Debug, Deserialize)]
struct AppsPage {
    data: Vec<RawApp>,
}

/// Fetches the full catalog by paging through `GET /apps` until a short page.
/// Returns `None` when the server reports the catalog is unchanged (304 on page 1).
pub async fn fetch_catalog(etag: Option<&str>) -> anyhow::Result<CatalogFetch> {
    let mut first_request = request(reqwest::Method::GET, "/apps")
        .query(&[("page", "1"), ("limit", &PAGE_LIMIT.to_string())]);
    if let Some(etag) = etag {
        first_request = first_request.header("If-None-Match", etag);
    }
    let first_response = first_request.send().await?;
    if first_response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(CatalogFetch::NotModified);
    }
    let new_etag = first_response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let first_page: AppsPage = first_response.json().await?;
    let mut last_page_len = first_page.data.len();
    let mut entries: Vec<AppEntry> =
        first_page.data.into_iter().filter_map(RawApp::into_app_entry).collect();

    let mut page = 2;
    while page <= MAX_PAGES && last_page_len as u32 == PAGE_LIMIT {
        let raw: AppsPage = request(reqwest::Method::GET, "/apps")
            .query(&[("page", &page.to_string()), ("limit", &PAGE_LIMIT.to_string())])
            .send()
            .await?
            .json()
            .await?;
        last_page_len = raw.data.len();
        entries.extend(raw.data.into_iter().filter_map(RawApp::into_app_entry));
        page += 1;
    }
    Ok(CatalogFetch::Fresh { entries, etag: new_etag })
}

pub enum CatalogFetch {
    Fresh { entries: Vec<AppEntry>, etag: Option<String> },
    NotModified,
}

pub async fn fetch_comments(app_id: &str) -> anyhow::Result<Vec<Comment>> {
    let mut comments: Vec<Comment> =
        request(reqwest::Method::GET, &format!("/apps/{app_id}/comments")).send().await?.json().await?;
    // Comments are the one piece of catalog text that arrives at runtime, so
    // they get sanitized here rather than in `rebuild_derived`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_whatever_format_the_server_suggested() {
        // The server hands out `format=jpeg` for icons these days; the old
        // `replace("format=webp", ..)` silently did nothing, so icons arrived
        // as opaque JPEGs and lost their transparency.
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
        // Scraped NPS assets are plain paths with no query at all.
        assert_eq!(
            force_format("https://h/scraped_assets/psvita/game/screenshots/01.jpeg"),
            "https://h/scraped_assets/psvita/game/screenshots/01.jpeg?format=jpeg"
        );
    }
}
