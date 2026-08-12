use super::{AppEntry, Category, Platform};
use crate::data::api::CatalogVersionInfo;
use rusqlite::{params, Connection, Result, Statement};
use std::path::Path;

#[cfg(target_os = "vita")]
const CATALOG_DB_PATH: &str = "/ux0:data/vitaforge/catalog.db";

#[cfg(not(target_os = "vita"))]
const CATALOG_DB_PATH_HOST: &str = "data/vitaforge/catalog.db";

#[cfg(target_os = "vita")]
const LOG_FILE_PATH: &str = "/ux0:data/vitaforge/vitaforge.log";

#[cfg(not(target_os = "vita"))]
const LOG_FILE_PATH: &str = "data/vitaforge/vitaforge.log";

pub fn log_db(msg: &str) {
    eprintln!("[SQLite DB] {}", msg);
    if let Some(parent) = Path::new(LOG_FILE_PATH).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_PATH) {
        use std::io::Write;
        let _ = writeln!(f, "[SQLite DB] {}", msg);
    }
}

pub fn get_catalog_db_path() -> &'static str {
    #[cfg(target_os = "vita")]
    {
        CATALOG_DB_PATH
    }
    #[cfg(not(target_os = "vita"))]
    {
        CATALOG_DB_PATH_HOST
    }
}

pub fn open_db() -> Result<Connection> {
    let path_str = get_catalog_db_path();
    log_db(&format!("Opening/Creating database file at path: {}", path_str));
    if let Some(parent) = Path::new(path_str).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path_str)?;

    conn.execute_batch(
        "PRAGMA journal_mode = MEMORY;
         PRAGMA synchronous = OFF;
         PRAGMA temp_store = MEMORY;",
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS apps (
            id TEXT PRIMARY KEY,
            titleid TEXT NOT NULL,
            content_id TEXT,
            name TEXT NOT NULL,
            original_name TEXT,
            author TEXT NOT NULL,
            description TEXT NOT NULL,
            long_description TEXT NOT NULL,
            requirements TEXT NOT NULL,
            changelog TEXT NOT NULL,
            release_page TEXT,
            category TEXT NOT NULL,
            platform TEXT NOT NULL,
            kind TEXT NOT NULL,
            icon_url TEXT,
            cover_url TEXT,
            background_url TEXT,
            screenshot_urls TEXT NOT NULL,
            download_url TEXT NOT NULL,
            source TEXT,
            version TEXT NOT NULL,
            region TEXT,
            zrif TEXT,
            source_catalog TEXT NOT NULL,
            source_labels TEXT NOT NULL,
            hash TEXT NOT NULL,
            hash2 TEXT NOT NULL,
            data_url TEXT,
            data_size_bytes INTEGER NOT NULL,
            size_bytes INTEGER NOT NULL,
            downloads INTEGER NOT NULL,
            rating REAL NOT NULL,
            updated_at TEXT NOT NULL,
            ratings_count INTEGER NOT NULL,
            likes_count INTEGER NOT NULL,
            comments_count INTEGER NOT NULL,
            user_liked INTEGER NOT NULL,
            user_rating INTEGER,
            overview TEXT NOT NULL
        );",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_apps_category ON apps(category);",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_apps_platform ON apps(platform);",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_apps_downloads ON apps(downloads);",
        [],
    )?;

    Ok(conn)
}

fn try_migrate_legacy_json_cache() -> Option<(Vec<AppEntry>, CatalogVersionInfo)> {
    #[cfg(target_os = "vita")]
    let (cache_path, version_path) = ("/ux0:data/vitaforge/catalog_cache.json", "/ux0:data/vitaforge/catalog_version.json");
    #[cfg(not(target_os = "vita"))]
    let (cache_path, version_path) = ("data/vitaforge/catalog_cache.json", "data/vitaforge/catalog_version.json");

    let version_bytes = std::fs::read(version_path).ok()?;
    let version_info: CatalogVersionInfo = serde_json::from_slice(&version_bytes).ok()?;
    let cache_bytes = std::fs::read(cache_path).ok()?;
    let mut entries: Vec<AppEntry> = serde_json::from_slice(&cache_bytes).ok()?;
    for entry in &mut entries {
        entry.rebuild_derived();
    }
    log_db(&format!("Migrating legacy catalog_cache.json ({} entries) to SQLite catalog.db...", entries.len()));
    let _ = save_cached_catalog_db(&entries, &version_info);
    Some((entries, version_info))
}

pub fn load_cached_catalog_db() -> Option<(Vec<AppEntry>, CatalogVersionInfo)> {
    log_db("Attempting to load cached catalog from SQLite DB...");
    let conn = open_db().ok()?;

    let get_meta = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM metadata WHERE key = ?",
            params![key],
            |row| row.get(0),
        )
        .ok()
    };

    let version: i64 = match get_meta("version").and_then(|v| v.parse().ok()) {
        Some(v) => v,
        None => return try_migrate_legacy_json_cache(),
    };
    let total_apps: i64 = match get_meta("total_apps").and_then(|v| v.parse().ok()) {
        Some(v) => v,
        None => return try_migrate_legacy_json_cache(),
    };
    let etag = get_meta("etag").unwrap_or_default();

    let version_info = CatalogVersionInfo {
        version,
        total_apps,
        etag,
    };

    let mut stmt: Statement = conn
        .prepare(
            "SELECT id, titleid, content_id, name, original_name, author, description,
                    long_description, requirements, changelog, release_page, category,
                    platform, kind, icon_url, cover_url, background_url, screenshot_urls,
                    download_url, source, version, region, zrif, source_catalog,
                    source_labels, hash, hash2, data_url, data_size_bytes, size_bytes,
                    downloads, rating, updated_at, ratings_count, likes_count,
                    comments_count, user_liked, user_rating, overview
             FROM apps",
        )
        .ok()?;

    let app_iter = stmt
        .query_map([], |row| {
            let category_str: String = row.get(11)?;
            let platform_str: String = row.get(12)?;
            let screenshot_urls_json: String = row.get(17)?;
            let source_labels_json: String = row.get(24)?;
            let user_liked_int: i32 = row.get(36)?;
            let user_rating_int: Option<u8> = row.get(37)?;
            let overview_json: String = row.get(38)?;

            let category = serde_json::from_str(&format!("\"{}\"", category_str))
                .unwrap_or(Category::Utility);
            let platform = serde_json::from_str(&format!("\"{}\"", platform_str))
                .unwrap_or(Platform::Vita);
            let screenshot_urls: Vec<String> =
                serde_json::from_str(&screenshot_urls_json).unwrap_or_default();
            let source_labels: Vec<String> =
                serde_json::from_str(&source_labels_json).unwrap_or_default();
            let overview: Vec<(String, String)> =
                serde_json::from_str(&overview_json).unwrap_or_default();

            let mut entry = AppEntry {
                id: row.get(0)?,
                titleid: row.get(1)?,
                titleid_lower: String::new(),
                content_id: row.get(2)?,
                name: row.get(3)?,
                original_name: row.get(4)?,
                name_lower: String::new(),
                author: row.get(5)?,
                author_lower: String::new(),
                description: row.get(6)?,
                long_description: row.get(7)?,
                requirements: row.get(8)?,
                changelog: row.get(9)?,
                release_page: row.get(10)?,
                category,
                platform,
                kind: row.get(13)?,
                icon_url: row.get(14)?,
                cover_url: row.get(15)?,
                background_url: row.get(16)?,
                screenshot_urls,
                download_url: row.get(18)?,
                source: row.get(19)?,
                version: row.get(20)?,
                region: row.get(21)?,
                zrif: row.get(22)?,
                source_catalog: row.get(23)?,
                source_labels,
                hash: row.get(25)?,
                hash2: row.get(26)?,
                data_url: row.get(27)?,
                data_size_bytes: row.get::<_, i64>(28)? as u64,
                size_bytes: row.get::<_, i64>(29)? as u64,
                downloads: row.get::<_, i64>(30)? as u64,
                rating: row.get(31)?,
                updated_at: row.get(32)?,
                ratings_count: row.get::<_, i64>(33)? as u32,
                likes_count: row.get::<_, i64>(34)? as u32,
                comments_count: row.get::<_, i64>(35)? as u32,
                user_liked: user_liked_int != 0,
                user_rating: user_rating_int,
                overview,
            };
            entry.rebuild_derived();
            Ok(entry)
        })
        .ok()?;

    let mut entries = Vec::new();
    for app in app_iter {
        if let Ok(entry) = app {
            entries.push(entry);
        }
    }

    Some((entries, version_info))
}

pub fn save_cached_catalog_db(
    entries: &[AppEntry],
    version_info: &CatalogVersionInfo,
) -> anyhow::Result<()> {
    log_db(&format!("Saving {} catalog entries (version {}) to SQLite DB...", entries.len(), version_info.version));
    let mut conn = open_db()?;
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM metadata", [])?;
    tx.execute("DELETE FROM apps", [])?;

    tx.execute(
        "INSERT INTO metadata (key, value) VALUES ('version', ?)",
        params![version_info.version.to_string()],
    )?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES ('total_apps', ?)",
        params![version_info.total_apps.to_string()],
    )?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES ('etag', ?)",
        params![version_info.etag],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO apps (
                id, titleid, content_id, name, original_name, author, description,
                long_description, requirements, changelog, release_page, category,
                platform, kind, icon_url, cover_url, background_url, screenshot_urls,
                download_url, source, version, region, zrif, source_catalog,
                source_labels, hash, hash2, data_url, data_size_bytes, size_bytes,
                downloads, rating, updated_at, ratings_count, likes_count,
                comments_count, user_liked, user_rating, overview
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?
            )",
        )?;

        for entry in entries {
            let category_str = serde_json::to_string(&entry.category)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            let platform_str = serde_json::to_string(&entry.platform)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            let screenshot_urls_json =
                serde_json::to_string(&entry.screenshot_urls).unwrap_or_else(|_| "[]".to_string());
            let source_labels_json =
                serde_json::to_string(&entry.source_labels).unwrap_or_else(|_| "[]".to_string());
            let overview_json =
                serde_json::to_string(&entry.overview).unwrap_or_else(|_| "[]".to_string());
            let user_liked_int = if entry.user_liked { 1 } else { 0 };

            stmt.execute(params![
                entry.id,
                entry.titleid,
                entry.content_id,
                entry.name,
                entry.original_name,
                entry.author,
                entry.description,
                entry.long_description,
                entry.requirements,
                entry.changelog,
                entry.release_page,
                category_str,
                platform_str,
                entry.kind,
                entry.icon_url,
                entry.cover_url,
                entry.background_url,
                screenshot_urls_json,
                entry.download_url,
                entry.source,
                entry.version,
                entry.region,
                entry.zrif,
                entry.source_catalog,
                source_labels_json,
                entry.hash,
                entry.hash2,
                entry.data_url,
                entry.data_size_bytes as i64,
                entry.size_bytes as i64,
                entry.downloads as i64,
                entry.rating,
                entry.updated_at,
                entry.ratings_count as i64,
                entry.likes_count as i64,
                entry.comments_count as i64,
                user_liked_int,
                entry.user_rating,
                overview_json,
            ])?;
        }
    }

    tx.commit()?;
    log_db(&format!("Successfully committed {} entries to catalog.db!", entries.len()));
    Ok(())
}

#[cfg(target_os = "vita")]
mod vita_sqlite_stubs {
    use std::os::raw::{c_char, c_int, c_uint, c_ulong};

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dlopen(_filename: *const c_char, _flags: c_int) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dlclose(_handle: *mut std::ffi::c_void) -> c_int {
        0
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dlsym(_handle: *mut std::ffi::c_void, _symbol: *const c_char) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dlerror() -> *const c_char {
        b"dynamic loading not supported\0".as_ptr() as *const c_char
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn fchown(_fd: c_int, _owner: c_uint, _group: c_uint) -> c_int {
        -1
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn readlink(_path: *const c_char, _buf: *mut c_char, _bufsiz: c_ulong) -> isize {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_catalog_save_and_load() {
        let version_info = CatalogVersionInfo {
            version: 42,
            total_apps: 1,
            etag: "test_etag".to_string(),
        };

        let mut sample_app = AppEntry {
            id: "app_123".to_string(),
            titleid: "VITAFORGE".to_string(),
            titleid_lower: String::new(),
            content_id: Some("EP0001-VITAFORGE_00-0000000000000000".to_string()),
            name: "Test App".to_string(),
            original_name: None,
            name_lower: String::new(),
            author: "Developer".to_string(),
            author_lower: String::new(),
            description: "A test application".to_string(),
            long_description: "Long test description".to_string(),
            requirements: "None".to_string(),
            changelog: "v1.0".to_string(),
            release_page: None,
            category: Category::Utility,
            platform: Platform::Vita,
            kind: "vpk".to_string(),
            icon_url: Some("https://example.com/icon.png".to_string()),
            cover_url: None,
            background_url: None,
            screenshot_urls: vec!["https://example.com/ss1.png".to_string()],
            download_url: "https://example.com/download.vpk".to_string(),
            source: None,
            version: "1.0.0".to_string(),
            region: Some("EU".to_string()),
            zrif: None,
            source_catalog: "rinnegatamante".to_string(),
            source_labels: vec!["Utility".to_string()],
            hash: "abc".to_string(),
            hash2: "def".to_string(),
            data_url: None,
            data_size_bytes: 1024,
            size_bytes: 2048,
            downloads: 500,
            rating: 4.5,
            updated_at: "2026-08-12".to_string(),
            ratings_count: 10,
            likes_count: 20,
            comments_count: 5,
            user_liked: true,
            user_rating: Some(5),
            overview: vec![("Developer".to_string(), "DevName".to_string())],
        };
        sample_app.rebuild_derived();

        let save_res = save_cached_catalog_db(&[sample_app.clone()], &version_info);
        assert!(save_res.is_ok(), "SQLite save failed: {:?}", save_res);

        let (loaded_apps, loaded_ver) =
            load_cached_catalog_db().expect("SQLite load failed");
        assert_eq!(loaded_ver.version, 42);
        assert_eq!(loaded_ver.total_apps, 1);
        assert_eq!(loaded_ver.etag, "test_etag");

        assert_eq!(loaded_apps.len(), 1);
        assert_eq!(loaded_apps[0].id, "app_123");
        assert_eq!(loaded_apps[0].titleid, "VITAFORGE");
        assert_eq!(loaded_apps[0].name, "Test App");
        assert_eq!(loaded_apps[0].downloads, 500);
        assert!(loaded_apps[0].user_liked);
        assert_eq!(loaded_apps[0].user_rating, Some(5));
        assert_eq!(loaded_apps[0].overview.len(), 1);
    }
}
