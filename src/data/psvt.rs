//! Vita themes from psvt.ovh (PSVita Custom Themes). There is no JSON API:
//! the list page is scraped. The preview image is named after the theme's
//! sha1, and that same sha1 is the `download.php?id=` key, so the list alone
//! is enough to install.
use super::AppEntry;
use serde_json::json;

const BASE: &str = "http://psvt.ovh";
/// Pages fetched up front; the rest load as the user scrolls (`load_more`). 21 themes per page.
const FIRST_PAGES: u32 = 2;
/// Start the next page when the cursor is this close to the end of the list.
pub const LOAD_AHEAD: usize = 48;

#[derive(Default)]
struct Loaded {
    pages: u32,
    /// Pages the site reports (0 until page 1 arrives).
    last: u32,
    done: bool,
    items: Vec<AppEntry>,
}
static LOADED: std::sync::Mutex<Loaded> = std::sync::Mutex::new(Loaded { pages: 0, last: 0, done: false, items: Vec::new() });
fn loaded() -> std::sync::MutexGuard<'static, Loaded> {
    LOADED.lock().unwrap_or_else(|e| e.into_inner())
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest.find(';').filter(|&e| e <= 10).and_then(|e| {
            let name = &rest[1..e];
            let ch = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                "hellip" => '\u{2026}',
                "ndash" => '\u{2013}',
                "mdash" => '\u{2014}',
                "rsquo" => '\u{2019}',
                "lsquo" => '\u{2018}',
                "rdquo" => '\u{201D}',
                "ldquo" => '\u{201C}',
                _ => {
                    let num = name.strip_prefix('#')?;
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => num.parse().ok()?,
                    };
                    char::from_u32(code).filter(|&c| c != '\0')?
                }
            };
            Some((ch, e + 1))
        });
        match decoded {
            Some((ch, len)) => {
                out.push(ch);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// "2,066 " -> 2066.
fn digits(s: &str) -> u64 {
    s.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = s.find(start)? + start.len();
    let len = s[from..].find(end)?;
    Some(&s[from..from + len])
}

pub fn parse(html: &str) -> Vec<AppEntry> {
    html.split("<a href=\"theme.php?id=")
        .skip(1)
        .filter_map(|row| {
            let id = row.split('"').next()?.trim();
            // Other links to theme.php (banners, related themes) aren't list rows.
            if !row.split('>').next()?.contains("list-group-item-action") {
                return None;
            }
            let hash = between(row, "psvt.ovh/themes/", ".png")?;
            let title = unescape(between(row, "<h5", "</h5>").and_then(|h| h.split_once('>')).map(|(_, t)| t)?.trim());
            let tags = unescape(between(row, "text-muted small\">", "</p>").unwrap_or("").trim());
            let downloads = between(row, "fs-6\">", "<").map(digits).unwrap_or(0);
            if id.is_empty() || hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || title.is_empty() {
                return None;
            }
            let secs: u64 = id.parse().unwrap_or(0); // the id is the Unix time the theme was added
            let mut entry: AppEntry = serde_json::from_value(json!({
                "id": id,
                "titleid": "",
                "name": title,
                "author": "psvt.ovh",
                "description": tags,
                "category": "theme",
                "platform": "theme",
                "kind": "theme",
                // Grid tiles are JPEG so the Vita's hardware decoder takes them. 192x192 is a multiple of the
                // 16px JPEG block in both directions, which is what that decoder mangled on odd sizes (the
                // old 192x109). The proxy crops and scales to the tile, a Lanczos downscale beats GPU stretching.
                "icon_url": format!("http://wsrv.nl/?url=psvt.ovh/themes/{hash}.png&w=192&h=192&fit=cover&output=jpg&q=82&maxage=1y"),
                // Detail covers and screenshots use JPEG through wsrv to enable hardware decoding and save bandwidth
                "cover_url": format!("http://wsrv.nl/?url=psvt.ovh/themes/{hash}.png&w=480&output=jpg&q=82&maxage=1y"),
                "screenshot_urls": [format!("http://wsrv.nl/?url=psvt.ovh/themes/{hash}.png&w=720&output=jpg&q=82&maxage=1y")],
                "download_url": format!("{BASE}/download.php?id={hash}"),
                "hash": hash,
                "version": "",
                "size_bytes": 0,
                "downloads": downloads,
                "rating": 0.0,
                "updated_at": crate::install::installed::format_ymd(secs),
            }))
            .ok()?;
            entry.rebuild_derived();
            Some(entry)
        })
        .collect()
}

/// How psvt.ovh orders a list (`o=`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Newest,
    Trending,
    Popular,
    Downloads,
    Title,
    Author,
}

impl Sort {
    fn param(self) -> &'static str {
        match self {
            Sort::Newest => "id",
            Sort::Trending => "trending",
            Sort::Popular => "popular",
            Sort::Downloads => "downloads",
            Sort::Title => "title",
            Sort::Author => "author",
        }
    }
}

/// A list request: ordering plus the site's own search (`s=`), tag (`t=`) and author (`a=`) filters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub sort: Sort,
    pub search: String,
    pub tag: String,
    pub author: String,
}

/// application/x-www-form-urlencoded value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_owned(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn list_url(query: &Query, page: u32) -> String {
    let mut url = format!("{BASE}/index.php?m=list&p={page}&o={}", query.sort.param());
    for (key, value) in [("s", &query.search), ("t", &query.tag), ("a", &query.author)] {
        if !value.trim().is_empty() {
            url.push_str(&format!("&{key}={}", encode(value.trim())));
        }
    }
    url
}

static SORT: std::sync::Mutex<Sort> = std::sync::Mutex::new(Sort::Newest);
fn current_sort() -> Sort {
    *SORT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Switches the feed's ordering. The loaded pages belong to the old order, so they are dropped
/// (`fetch` then reloads from page 1). Returns false when the order didn't change.
pub fn set_sort(sort: Sort) -> bool {
    let mut current = SORT.lock().unwrap_or_else(|e| e.into_inner());
    if *current == sort {
        return false;
    }
    *current = sort;
    let mut state = loaded();
    state.pages = 0;
    state.last = 0;
    state.done = false;
    state.items.clear();
    true
}

async fn page_html(n: u32) -> Option<String> {
    let query = Query { sort: current_sort(), ..Query::default() };
    crate::net::client().get(list_url(&query, n)).send().await.ok()?.text().await.ok()
}

/// One page of any query (search, sort, tag, author). Stateless: the caller keeps the page number.
/// Empty means the end, or a network failure.
pub async fn search(query: &Query, page: u32) -> Vec<AppEntry> {
    let Some(html) = crate::net::client().get(list_url(query, page)).send().await.ok() else { return Vec::new() };
    html.text().await.map(|h| parse(&h)).unwrap_or_default()
}

/// A page of the feed. `None` when the request failed or the answer isn't a list page at all
/// (the site sometimes answers with a short stub); `Some(empty)` is a real, empty page.
async fn page(n: u32) -> Option<Vec<AppEntry>> {
    let html = page_html(n).await?;
    (html.contains("pagination") || html.contains("list-group")).then(|| parse(&html))
}

const PER_PAGE: usize = 21;
static TOTAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// How many themes psvt.ovh has in all (0 until known), so the sidebar can show it while pages stream in.
pub fn total() -> usize {
    TOTAL.load(std::sync::atomic::Ordering::Relaxed)
}

/// "of 185" next to the page box.
fn total_pages(html: &str) -> u32 {
    between(html, "<span class=\"small ms-1 text-nowrap\">of ", "<").map(|n| digits(n) as u32).unwrap_or(0)
}

/// Total themes = (pages - 1) full pages + the last page's rows (one request, once).
fn count_in_background(pages: u32) {
    if pages < 2 || COUNTING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    tokio::spawn(async move {
        let tail = page(pages).await.map_or(0, |p| p.len());
        if tail > 0 {
            TOTAL.store((pages as usize - 1) * PER_PAGE + tail, std::sync::atomic::Ordering::Relaxed);
            TOTAL_EXACT.store(true, std::sync::atomic::Ordering::Relaxed);
        } else {
            COUNTING.store(false, std::sync::atomic::Ordering::Relaxed); // try again on the next refresh
        }
    });
}
static TOTAL_EXACT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static COUNTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What the theme's own page adds to the list row.
#[derive(Debug, Default, PartialEq)]
pub struct Detail {
    pub version: String,
    pub author: String,
    pub validation: String,
    pub added: String,
    pub downloads: u64,
    pub tags: Vec<String>,
    /// Zip size from the download's Content-Length (not on the page).
    pub size_bytes: u64,
}

pub fn parse_detail(html: &str) -> Detail {
    let after = |marker: &str| html.find(marker).map(|i| &html[i + marker.len()..]);
    let text_to = |s: Option<&str>, end: &str| s.and_then(|s| s.find(end).map(|e| unescape(s[..e].trim()))).unwrap_or_default();
    let author = after("Author (provider):</strong>").and_then(|s| s.split_once('>').map(|(_, r)| r));
    let validation = after("Validation score:").and_then(|s| s.split_once("fw-bold").map(|(_, r)| r)).and_then(|s| s.split_once('>').map(|(_, r)| r));
    let validation = text_to(validation, "<").replace(",00%", "%");
    let tags = after("mt-3\">").map(|s| {
        s.split("href=\"./index.php?t=").skip(1).filter_map(|a| {
            let tag = between(a, "</i>", "</a>")?.trim();
            (!tag.is_empty()).then(|| unescape(tag))
        }).collect()
    });
    Detail {
        version: text_to(after("Version:</strong> <span class=\"badge bg-secondary\">"), "<"),
        author: text_to(author, "<"),
        validation,
        added: text_to(after("Date added:</strong> <span class=\"text-muted\">"), "<"),
        downloads: after("<strong>Downloads:</strong>").map(|s| digits(s.split('<').next().unwrap_or(""))).unwrap_or(0),
        tags: tags.unwrap_or_default(),
        size_bytes: 0,
    }
}

/// The page is ~2 MB (inline images) but the facts sit in the first ~140 KB, so stop reading there.
pub async fn fetch_detail(id: &str, hash: &str) -> Detail {
    let (page, size) = tokio::join!(detail_page(id), zip_size(hash));
    let mut detail = page.unwrap_or_default();
    detail.size_bytes = size;
    detail
}

async fn detail_page(id: &str) -> Option<Detail> {
    let mut response = crate::net::client().get(format!("{BASE}/theme.php?id={id}")).send().await.ok()?;
    let mut buf: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        buf.extend_from_slice(&chunk);
        if buf.len() > 400_000 || String::from_utf8_lossy(&buf).contains("download.php?id=") {
            break;
        }
    }
    Some(parse_detail(&String::from_utf8_lossy(&buf)))
}

/// Zip size without downloading it: HEAD's Content-Length, else a 1-byte ranged GET's Content-Range. 0 if unknown.
async fn zip_size(hash: &str) -> u64 {
    if hash.is_empty() {
        return 0;
    }
    let url = format!("{BASE}/download.php?id={hash}");
    let client = crate::net::client();
    if let Ok(r) = client.head(&url).send().await
        && let Some(n) = r.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok())
        && n > 0
    {
        return n;
    }
    let Ok(r) = client.get(&url).header("Range", "bytes=0-0").send().await else { return 0 };
    r.headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

static SEARCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// True from `begin_search` until its results are merged, so the UI can show a loader.
pub fn searching() -> bool {
    SEARCHING.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn begin_search() {
    SEARCHING.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Asks psvt.ovh for the first pages matching `text` (it searches titles, tags and authors),
/// adds them to the loaded list and returns just those. The normal feed paging is untouched.
pub async fn search_merge(text: &str) -> Option<Vec<AppEntry>> {
    let query = Query { search: text.to_owned(), ..Query::default() };
    let (a, b) = tokio::join!(search(&query, 1), search(&query, 2));
    SEARCHING.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut found = a;
    found.extend(b);
    merge(&mut loaded().items, found.clone(), false);
    Some(found)
}

/// True while psvt.ovh may still have pages we haven't loaded.
pub fn has_more() -> bool {
    let state = loaded();
    state.pages > 0 && !state.done
}

/// Adds `fresh` to the list, newest first, skipping ids already there.
fn merge(items: &mut Vec<AppEntry>, fresh: Vec<AppEntry>, front: bool) {
    let known: std::collections::HashSet<String> = items.iter().map(|e| e.id.clone()).collect();
    let fresh: Vec<AppEntry> = fresh.into_iter().filter(|e| !known.contains(&e.id)).collect();
    if front {
        items.splice(0..0, fresh);
    } else {
        items.extend(fresh);
    }
}

/// Never fails: themes are a bonus on top of the catalog, so a dead psvt.ovh just means an empty tab.
/// Returns everything loaded so far, so the periodic catalog refresh doesn't drop scrolled-in pages.
pub async fn fetch() -> Vec<AppEntry> {
    let have_pages = loaded().pages;
    // Pages 1 and 2 go out together: one round trip instead of two in a row.
    let (first_html, second) = tokio::join!(page_html(1), async {
        if have_pages == 0 { page(2).await.unwrap_or_default() } else { Vec::new() }
    });
    let first = first_html.as_deref().map(parse).unwrap_or_default();
    if let Some(h) = &first_html {
        let pages = total_pages(h);
        if pages > 0 {
            loaded().last = pages;
        }
        if !TOTAL_EXACT.load(std::sync::atomic::Ordering::Relaxed) {
            if total() == 0 {
                // shortcut: show pages * PER_PAGE right away (off by < one page) so the tab never reads 0; the
                // last-page count below replaces it with the exact number.
                TOTAL.store(pages as usize * PER_PAGE, std::sync::atomic::Ordering::Relaxed);
            }
            count_in_background(pages);
        }
    }
    let mut state = loaded();
    if state.pages == 0 && !first.is_empty() {
        state.pages = FIRST_PAGES;
    }
    merge(&mut state.items, first, true);
    merge(&mut state.items, second, false);
    state.items.clone()
}

/// The next page of themes (only that page), or `None` once psvt.ovh runs out, the first load never happened, or the
/// request failed (the caller retries later; a failure never ends the list).
pub async fn load_more() -> Option<Vec<AppEntry>> {
    let next = {
        let mut state = loaded();
        if state.done || state.pages == 0 {
            return None;
        }
        if state.last > 0 && state.pages >= state.last {
            state.done = true;
            return None;
        }
        state.pages + 1
    };
    let fresh = page(next).await?;
    let mut state = loaded();
    if fresh.is_empty() {
        state.done = true;
        return None;
    }
    state.pages = next;
    merge(&mut state.items, fresh.clone(), false);
    Some(fresh) // just this page; the caller appends it
}

#[cfg(test)]
mod tests {
    use super::*;
    const ROW: &str = r#"<a href="theme.php?id=1794027428" class="list-group-item list-group-item-action d-flex">
<img src="//wsrv.nl/?url=psvt.ovh/themes/d2c60918c52f6be6924476387613e6cfb114fe3b.png&w=96&maxage=1y" alt="Preview">
<h5 class="mb-1 text-truncate fw-bold">DEMON&#039;S SOULS</h5>
<p class="mb-1 text-muted small">dark fantasy, knight</p>
<span class="badge bg-secondary rounded-pill fs-6">23 <i class="bi bi-download"></i></span></a>"#;
    #[test]
    fn builds_list_urls() {
        assert_eq!(list_url(&Query::default(), 3), "http://psvt.ovh/index.php?m=list&p=3&o=id");
        let q = Query { sort: Sort::Downloads, search: " dark souls ".into(), tag: "a&b".into(), author: String::new() };
        assert_eq!(list_url(&q, 1), "http://psvt.ovh/index.php?m=list&p=1&o=downloads&s=dark+souls&t=a%26b");
    }
    #[test]
    fn parses_a_list_row() {
        let list = parse(&format!("<div>{ROW}{ROW}</div>"));
        assert_eq!(list.len(), 2);
        let t = &list[0];
        assert_eq!((t.id.as_str(), t.name.as_str(), t.downloads), ("1794027428", "DEMON'S SOULS", 23));
        assert_eq!(t.platform, crate::data::Platform::Theme);
        assert!(t.download_url.ends_with("download.php?id=d2c60918c52f6be6924476387613e6cfb114fe3b"));
    }
    #[test]
    fn parses_a_detail_page() {
        let html = r#"<ul><li><strong>Version:</strong> <span class="badge bg-secondary">01.10</span></li>
<li><strong>Author (provider):</strong> <a href="./index.php?a=bray" class="x">braylaf</a></li>
<li><strong>Validation score:</strong> <span class="fw-bold text-success">
 100,00%
</span></li><li><strong>Downloads:</strong> 2,066 <span>(avg)</span></li>
<li><strong>Date added:</strong> <span class="text-muted">12 September 2026</span></li></ul>
<div class="mt-3"><a href="./index.php?t=comic" class="btn"><i class="bi bi-tag-fill small"></i> comic</a><a href="./index.php?t=jack+kirby" class="btn"><i class="bi"></i> jack kirby</a></div>"#;
        let d = parse_detail(html);
        assert_eq!((d.version.as_str(), d.author.as_str(), d.validation.as_str()), ("01.10", "braylaf", "100%"));
        assert_eq!((d.added.as_str(), d.downloads), ("12 September 2026", 2066));
        assert_eq!(d.tags, ["comic", "jack kirby"]);
    }
    #[test]
    fn skips_rows_without_a_hash() {
        assert!(parse("<a href=\"theme.php?id=1\"><h5 class=\"a\">x</h5>").is_empty());
    }
}
