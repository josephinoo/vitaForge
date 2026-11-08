//! Registers an installed theme in the shell's `tbl_theme` (ur0:shell/db/app.db).
//! The column plan is pure so it can be tested on the host; the SQLite calls are Vita-only.
use anyhow::{Result, bail};

pub const APP_DB: &str = "ur0:shell/db/app.db";

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Text(String),
    Int(i64),
}

pub struct Col {
    pub name: String,
    pub notnull: bool,
    pub has_default: bool,
}

pub struct ThemeRow<'a> {
    pub dir: &'a str,
    pub title: &'a str,
    pub provider: &'a str,
    pub content_ver: &'a str,
    pub size: i64,
}

/// INSERT statement + bound values for this firmware's `tbl_theme` schema.
pub fn plan_insert(cols: &[Col], row: &ThemeRow) -> Option<(String, Vec<(String, Val)>)> {
    let mut out: Vec<(String, Val)> = Vec::new();
    for col in cols {
        let n = col.name.as_str();
        let val = match n {
            "id" => Some(Val::Text(row.dir.to_owned())),
            "type" => Some(Val::Int(100)),
            "packageImageFilePath" => Some(Val::Text("preview_thumbnail.png".into())),
            "homePreviewFilePath" => Some(Val::Text("preview_page.png".into())),
            "startPreviewFilePath" => Some(Val::Text("preview_lockscreen.png".into())),
            "contentVer" => Some(Val::Int(content_version(row.content_ver))),
            "size" => Some(Val::Int(row.size)),
            _ if n.starts_with("title") && !n.ends_with("Id") => Some(Val::Text(row.title.to_owned())),
            _ if n.starts_with("provider") && !n.ends_with("Id") => Some(Val::Text(row.provider.to_owned())),
            _ if col.notnull && !col.has_default => Some(Val::Int(0)),
            _ => None,
        };
        if let Some(v) = val {
            out.push((n.to_owned(), v));
        }
    }
    if !out.iter().any(|(n, _)| n == "id") {
        return None;
    }
    let names: Vec<String> = out.iter().map(|(n, _)| format!("\"{n}\"")).collect();
    let marks: Vec<String> = (1..=out.len()).map(|i| format!("?{i}")).collect();
    Some((format!("INSERT INTO tbl_theme ({}) VALUES ({})", names.join(","), marks.join(",")), out))
}

/// The shell stores a theme's version as an integer: "01.20" -> 120 (major * 100 + minor).
pub fn content_version(ver: &str) -> i64 {
    let mut parts = ver.trim().splitn(2, '.');
    let number = |p: Option<&str>| p.and_then(|p| p.trim().parse::<i64>().ok());
    match number(parts.next()) {
        Some(major) => major * 100 + number(parts.next()).unwrap_or(0),
        None => 100,
    }
}

/// Text between `<outer><inner>` and `</inner>` (theme.xml's `m_title/m_default` shape) or `<tag>..</tag>`.
pub fn xml_text(xml: &str, path: &[&str]) -> Option<String> {
    let mut rest = xml;
    for tag in path {
        let open = format!("<{tag}>");
        rest = &rest[rest.find(&open)? + open.len()..];
    }
    let end = rest.find('<')?;
    let text = rest[..end].trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(target_os = "vita")]
mod sys {
    use super::*;
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    unsafe extern "C" {
        fn sqlite3_open(name: *const c_char, db: *mut *mut c_void) -> c_int;
        fn sqlite3_close(db: *mut c_void) -> c_int;
        fn sqlite3_prepare_v2(db: *mut c_void, sql: *const c_char, n: c_int, st: *mut *mut c_void, tail: *mut *const c_char) -> c_int;
        fn sqlite3_step(st: *mut c_void) -> c_int;
        fn sqlite3_finalize(st: *mut c_void) -> c_int;
        fn sqlite3_column_text(st: *mut c_void, i: c_int) -> *const u8;
        fn sqlite3_column_int(st: *mut c_void, i: c_int) -> c_int;
        fn sqlite3_bind_text(st: *mut c_void, i: c_int, s: *const c_char, n: c_int, d: isize) -> c_int;
        fn sqlite3_bind_int64(st: *mut c_void, i: c_int, v: i64) -> c_int;
        fn sqlite3_vfs_find(name: *const c_char) -> *mut Vfs;
        fn sqlite3_vfs_register(vfs: *mut Vfs, make_default: c_int) -> c_int;
        fn malloc(n: usize) -> *mut c_void;
        fn realloc(p: *mut c_void, n: usize) -> *mut c_void;
        fn free(p: *mut c_void);
    }
    extern "C" fn sql_malloc(n: c_int) -> *mut c_void {
        unsafe { malloc(n.max(0) as usize) }
    }
    extern "C" fn sql_realloc(p: *mut c_void, n: c_int) -> *mut c_void {
        unsafe { realloc(p, n.max(0) as usize) }
    }
    extern "C" fn sql_free(p: *mut c_void) {
        unsafe { free(p) }
    }
    // Sony's own VFS ("psp2") is read-only: opening a file for writing fails with SQLITE_CANTOPEN, and writes
    // are dropped (a COMMIT "succeeds" and the row is gone). This is the read/write VFS from VitaSmith's
    // libsqlite (after xyzz): same layout as sqlite3.h, overriding xOpen, xWrite, xSync and xDelete with sceIo.
    #[repr(C)]
    struct File {
        methods: *mut IoMethods,
    }
    /// Sony's file struct: the sqlite3_file header followed by a pointer to the fd.
    #[repr(C)]
    struct VfsFile {
        file: File,
        fd: *mut c_int,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct IoMethods {
        version: c_int,
        // xClose xRead xWrite xTruncate xSync xFileSize xLock xUnlock xCheckReservedLock xFileControl
        // xSectorSize xDeviceCharacteristics xShmMap xShmLock xShmBarrier xShmUnmap
        slots: [usize; 16],
    }
    const SLOT_WRITE: usize = 2;
    const SLOT_SYNC: usize = 4;
    type OpenFn = unsafe extern "C" fn(*mut Vfs, *const c_char, *mut File, c_int, *mut c_int) -> c_int;
    type DeleteFn = unsafe extern "C" fn(*mut Vfs, *const c_char, c_int) -> c_int;
    #[repr(C)]
    struct Vfs {
        version: c_int,
        sz_os_file: c_int,
        mx_pathname: c_int,
        next: *mut Vfs,
        name: *const c_char,
        app_data: *mut c_void,
        x_open: Option<OpenFn>,
        x_delete: Option<DeleteFn>,
        // xAccess xFullPathname xDlOpen xDlError xDlSym xDlClose xRandomness xSleep xCurrentTime
        // xGetLastError xCurrentTimeInt64 xSetSystemCall xGetSystemCall xNextSystemCall
        rest: [usize; 14],
    }
    const SQLITE_CANTOPEN: c_int = 14;
    const SQLITE_IOERR_WRITE: c_int = 10 | (3 << 8);
    const SQLITE_IOERR_FSYNC: c_int = 10 | (4 << 8);
    const SQLITE_IOERR_DELETE: c_int = 10 | (10 << 8);
    const OPEN_READWRITE: c_int = 0x2;
    const OPEN_CREATE: c_int = 0x4;
    static RW_METHODS: std::sync::atomic::AtomicPtr<IoMethods> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
    static TEMP_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "C" fn rw_write(file: *mut File, buf: *const c_void, count: c_int, offset: i64) -> c_int {
        let fd = unsafe { *(*(file as *mut VfsFile)).fd };
        if unsafe { vitasdk_sys::sceIoLseek(fd, offset, vitasdk_sys::SCE_SEEK_SET as c_int) } != offset {
            return SQLITE_IOERR_WRITE;
        }
        if unsafe { vitasdk_sys::sceIoWrite(fd, buf, count as _) } != count {
            return SQLITE_IOERR_WRITE;
        }
        0
    }
    unsafe extern "C" fn rw_sync(file: *mut File, _flags: c_int) -> c_int {
        let fd = unsafe { *(*(file as *mut VfsFile)).fd };
        if unsafe { vitasdk_sys::sceIoSyncByFd(fd, 0) } < 0 { SQLITE_IOERR_FSYNC } else { 0 }
    }
    unsafe extern "C" fn rw_delete(_vfs: *mut Vfs, name: *const c_char, _sync_dir: c_int) -> c_int {
        if unsafe { vitasdk_sys::sceIoRemove(name) } < 0 { SQLITE_IOERR_DELETE } else { 0 }
    }
    unsafe extern "C" fn rw_open(vfs: *mut Vfs, name: *const c_char, file: *mut File, flags: c_int, out: *mut c_int) -> c_int {
        use std::sync::atomic::Ordering;
        let org = unsafe { (*vfs).app_data } as *mut Vfs;
        // SQLite passes no name for its scratch files; give them one in our own folder.
        let temp;
        let name = if name.is_null() {
            let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            temp = match CString::new(format!("ux0:data/vitaforge/sqlite-tmp-{n}")) {
                Ok(t) => t,
                Err(_) => return SQLITE_CANTOPEN,
            };
            temp.as_ptr()
        } else {
            name
        };
        // The stock xOpen doesn't create files, so create it first.
        if flags & OPEN_CREATE != 0 {
            let fd = unsafe { vitasdk_sys::sceIoOpen(name, vitasdk_sys::SCE_O_RDONLY as c_int, 0o777) };
            let fd = if fd < 0 { unsafe { vitasdk_sys::sceIoOpen(name, (vitasdk_sys::SCE_O_WRONLY | vitasdk_sys::SCE_O_CREAT) as c_int, 0o777) } } else { fd };
            if fd >= 0 {
                unsafe { vitasdk_sys::sceIoClose(fd) };
            }
        }
        let Some(open) = (unsafe { (*org).x_open }) else { return SQLITE_CANTOPEN };
        let rc = unsafe { open(org, name, file, flags, out) };
        if unsafe { (*file).methods.is_null() } || flags & OPEN_READWRITE == 0 {
            return rc;
        }
        // The stock xOpen also opens read-only even when asked for read/write: reopen with write access.
        let fd = unsafe { (*(file as *mut VfsFile)).fd };
        if !fd.is_null() && unsafe { *fd } >= 0 {
            unsafe {
                vitasdk_sys::sceIoClose(*fd);
                *fd = vitasdk_sys::sceIoOpen(name, vitasdk_sys::SCE_O_RDWR as c_int, 0o777);
                if *fd < 0 {
                    return SQLITE_IOERR_WRITE;
                }
            }
        }
        let mut methods = RW_METHODS.load(Ordering::Acquire);
        if methods.is_null() {
            let mut copy = unsafe { *(*file).methods };
            copy.slots[SLOT_WRITE] = rw_write as usize;
            copy.slots[SLOT_SYNC] = rw_sync as usize;
            methods = Box::into_raw(Box::new(copy));
            RW_METHODS.store(methods, Ordering::Release);
        }
        unsafe { (*file).methods = methods };
        rc
    }
    /// Registers the read/write VFS as the default, once, after the allocator is set.
    fn register_rw_vfs() {
        unsafe {
            let stock = sqlite3_vfs_find(std::ptr::null());
            if stock.is_null() {
                return;
            }
            let rw = Box::into_raw(Box::new(std::ptr::read(stock)));
            (*rw).name = c"psp2_rw".as_ptr();
            (*rw).x_open = Some(rw_open);
            (*rw).x_delete = Some(rw_delete);
            (*rw).app_data = stock as *mut c_void;
            let rc = sqlite3_vfs_register(rw, 1);
            crate::install::log_file(&format!("app.db: read/write vfs registered rc={rc}"));
        }
    }
    /// The system SQLite module has no allocator of its own: until the app hands it one, every
    /// open fails with SQLITE_NOMEM (7).
    fn init_module() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            vitasdk_sys::sceSysmoduleLoadModule(vitasdk_sys::SCE_SYSMODULE_SQLITE);
            let methods = Box::leak(Box::new(vitasdk_sys::SceSqliteMallocMethods {
                xMalloc: Some(sql_malloc),
                xRealloc: Some(sql_realloc),
                xFree: Some(sql_free),
            }));
            vitasdk_sys::sceSqliteConfigMallocMethods(methods);
            register_rw_vfs();
        });
    }
    const SQLITE_ROW: c_int = 100;
    const SQLITE_DONE: c_int = 101;
    const TRANSIENT: isize = -1;

    pub struct Db(*mut c_void);
    impl Drop for Db {
        fn drop(&mut self) {
            unsafe { sqlite3_close(self.0) };
        }
    }
    struct Stmt(*mut c_void);
    impl Drop for Stmt {
        fn drop(&mut self) {
            unsafe { sqlite3_finalize(self.0) };
        }
    }
    impl Db {
        pub fn open(path: &str) -> Result<Self> {
            init_module();
            let c = CString::new(path)?;
            let mut db = std::ptr::null_mut();
            let rc = unsafe { sqlite3_open(c.as_ptr(), &mut db) };
            if rc != 0 || db.is_null() {
                bail!("couldn't open {path} (sqlite {rc})");
            }
            Ok(Db(db))
        }
        fn prepare(&self, sql: &str) -> Result<Stmt> {
            let c = CString::new(sql)?;
            let mut st = std::ptr::null_mut();
            let rc = unsafe { sqlite3_prepare_v2(self.0, c.as_ptr(), -1, &mut st, std::ptr::null_mut()) };
            if rc != 0 {
                bail!("sqlite prepare failed ({rc}): {sql}");
            }
            Ok(Stmt(st))
        }
        pub fn exec(&self, sql: &str) -> Result<()> {
            let st = self.prepare(sql)?;
            let rc = unsafe { sqlite3_step(st.0) };
            if rc != SQLITE_DONE && rc != SQLITE_ROW {
                bail!("sqlite step failed ({rc}): {sql}");
            }
            Ok(())
        }
        /// Steps once and hands back the raw result code, for diagnosing a statement that "succeeds".
        pub fn exec_rc(&self, sql: &str) -> Result<c_int> {
            let st = self.prepare(sql)?;
            Ok(unsafe { sqlite3_step(st.0) })
        }
        pub fn first_text(&self, sql: &str) -> Result<String> {
            let st = self.prepare(sql)?;
            if unsafe { sqlite3_step(st.0) } != SQLITE_ROW {
                bail!("no row from {sql}");
            }
            let p = unsafe { sqlite3_column_text(st.0, 0) };
            Ok(if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p as *const c_char) }.to_string_lossy().into_owned() })
        }
        pub fn columns(&self) -> Result<Vec<Col>> {
            let st = self.prepare("PRAGMA table_info(tbl_theme)")?;
            let mut cols = Vec::new();
            while unsafe { sqlite3_step(st.0) } == SQLITE_ROW {
                let p = unsafe { sqlite3_column_text(st.0, 1) };
                if p.is_null() {
                    continue;
                }
                let name = unsafe { CStr::from_ptr(p as *const c_char) }.to_string_lossy().into_owned();
                let notnull = unsafe { sqlite3_column_int(st.0, 3) } != 0;
                let has_default = !unsafe { sqlite3_column_text(st.0, 4) }.is_null();
                cols.push(Col { name, notnull, has_default });
            }
            Ok(cols)
        }
        pub fn insert(&self, sql: &str, vals: &[(String, Val)]) -> Result<()> {
            let st = self.prepare(sql)?;
            for (i, (_, v)) in vals.iter().enumerate() {
                let idx = i as c_int + 1;
                match v {
                    Val::Text(t) => {
                        let c = CString::new(t.replace('\0', ""))?;
                        unsafe { sqlite3_bind_text(st.0, idx, c.as_ptr(), -1, TRANSIENT) };
                    }
                    Val::Int(n) => unsafe {
                        sqlite3_bind_int64(st.0, idx, *n);
                    },
                }
            }
            if unsafe { sqlite3_step(st.0) } != SQLITE_DONE {
                bail!("insert into tbl_theme failed");
            }
            Ok(())
        }
        pub fn delete_theme(&self, dir: &str) -> Result<()> {
            let q = dir.replace('\'', "''");
            self.exec(&format!("DELETE FROM tbl_theme WHERE id = '{q}' OR id = '{q}/'"))
        }
    }
}

/// Keeps the newest 3 copies of app.db before it is touched.
#[cfg(target_os = "vita")]
fn backup_app_db() -> Result<String> {
    use anyhow::Context;
    const DIR: &str = "ux0:data/vitaforge/backups";
    std::fs::create_dir_all(DIR)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let copy = format!("{DIR}/app.db.{stamp}");
    std::fs::copy(APP_DB, &copy).context("couldn't back up app.db")?;
    let mut old: Vec<_> = std::fs::read_dir(DIR)?.flatten().map(|e| e.path()).collect();
    old.sort();
    while old.len() > 3 {
        let _ = std::fs::remove_file(old.remove(0));
    }
    Ok(copy)
}

/// False when there is no app.db at all (an emulator such as Vita3K): the files are installed and
/// there is nothing to register. Any other error (denied, I/O) is not "absent" and must surface.
#[cfg(target_os = "vita")]
fn app_db_present() -> Result<bool> {
    match std::fs::metadata(APP_DB) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(anyhow::anyhow!("can't read {APP_DB}: {e}")),
    }
}

/// The shell writes app.db too. A live rollback journal starts with this magic; a committed one is
/// deleted, truncated or zeroed. Writing under it would roll the shell's transaction back.
#[cfg(target_os = "vita")]
fn shell_is_writing() -> bool {
    use std::io::Read;
    const MAGIC: [u8; 8] = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];
    let mut head = [0u8; 8];
    std::fs::File::open("ur0:shell/db/app.db-journal")
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && head == MAGIC
}

/// The system SQLite can't be trusted to commit into ur0:shell/db directly (the journal can't be created there, and
/// with an in-memory journal the committed row vanished again), so the edit happens on a copy in ux0, is checked,
/// and only then put back. `check` runs against app.db after the swap; if it fails the backup is restored.
#[cfg(target_os = "vita")]
fn edit_copy(change: impl FnOnce(&sys::Db) -> Result<()>, check: impl FnOnce(&sys::Db) -> Result<()>) -> Result<()> {
    use anyhow::Context;
    const WORK_DB: &str = "ux0:data/vitaforge/app.work.db";
    if !app_db_present()? {
        return Ok(());
    }
    if shell_is_writing() {
        bail!("the system is updating its theme list right now; try again in a moment");
    }
    let backup = backup_app_db()?;
    let clean = || {
        let _ = std::fs::remove_file(WORK_DB);
        let _ = std::fs::remove_file(format!("{WORK_DB}-journal"));
    };
    clean();
    std::fs::copy(APP_DB, WORK_DB).context("couldn't copy app.db to work on it")?;
    let edited = (|| -> Result<()> {
        let db = sys::Db::open(WORK_DB)?;
        if db.first_text("PRAGMA integrity_check")? != "ok" {
            bail!("app.db failed its integrity check; theme list not changed");
        }
        change(&db)?;
        if db.first_text("PRAGMA integrity_check")? != "ok" {
            bail!("the edited copy of app.db failed its integrity check; theme list not changed");
        }
        Ok(())
    })(); // the connection is closed here, before the file is copied
    let result = edited.and_then(|_| {
        std::fs::copy(WORK_DB, APP_DB).context("couldn't write app.db")?;
        check(&sys::Db::open(APP_DB)?).inspect_err(|_| {
            let _ = std::fs::copy(&backup, APP_DB);
        })
    });
    clean();
    result
}

#[cfg(target_os = "vita")]
fn count_for(db: &sys::Db, dir: &str) -> Result<String> {
    db.first_text(&format!("SELECT count(*) FROM tbl_theme WHERE id = '{}'", dir.replace('\'', "''")))
}

#[cfg(target_os = "vita")]
pub fn register(row: &ThemeRow) -> Result<()> {
    edit_copy(
        |db| {
            let (sql, vals) = plan_insert(&db.columns()?, row).ok_or_else(|| anyhow::anyhow!("unexpected tbl_theme schema"))?;
            db.exec("BEGIN")?;
            if let Err(e) = db.delete_theme(row.dir).and_then(|_| db.insert(&sql, &vals)) {
                let _ = db.exec("ROLLBACK");
                return Err(e);
            }
            let rc = db.exec_rc("COMMIT")?;
            let n = count_for(db, row.dir)?;
            crate::install::log_file(&format!("app.db copy: COMMIT rc={rc}, rows={n} for {}", row.dir));
            if rc != 101 || n != "1" {
                bail!("the theme was not saved in the system theme list");
            }
            Ok(())
        },
        |db| {
            let n = count_for(db, row.dir)?;
            crate::install::log_file(&format!("app.db live: rows={n} for {}", row.dir));
            if n != "1" {
                bail!("the system theme list didn't keep the theme (app.db restored)");
            }
            Ok(())
        },
    )
}

#[cfg(target_os = "vita")]
pub fn unregister(dir: &str) -> Result<()> {
    edit_copy(
        |db| {
            db.exec("BEGIN")?;
            db.delete_theme(dir)?;
            db.exec("COMMIT")
        },
        |_| Ok(()),
    )
}

#[cfg(not(target_os = "vita"))]
pub fn register(_row: &ThemeRow) -> Result<()> {
    bail!("theme registration only works on the Vita")
}

#[cfg(not(target_os = "vita"))]
pub fn unregister(_dir: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn col(name: &str, notnull: bool, has_default: bool) -> Col {
        Col { name: name.into(), notnull, has_default }
    }
    #[test]
    fn version_becomes_the_shells_integer() {
        assert_eq!(content_version("01.20"), 120);
        assert_eq!(content_version("2.5"), 205);
        assert_eq!(content_version("3"), 300);
        assert_eq!(content_version("x"), 100);
    }
    #[test]
    fn plan_fills_known_columns_and_zeroes_required_unknowns() {
        let cols = [col("id", true, false), col("type", true, false), col("title", false, false), col("titleId", false, false), col("providerEn", false, false), col("size", true, false), col("mystery", true, false), col("optional", false, false), col("withDefault", true, true)];
        let row = ThemeRow { dir: "ux0:customtheme/Foo", title: "Foo", provider: "Bar", content_ver: "01.00", size: 7 };
        let (sql, vals) = plan_insert(&cols, &row).unwrap();
        let names: Vec<&str> = vals.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["id", "type", "title", "providerEn", "size", "mystery"]);
        assert_eq!(vals[5].1, Val::Int(0));
        assert!(sql.ends_with("VALUES (?1,?2,?3,?4,?5,?6)"));
    }
    #[test]
    fn plan_needs_an_id_column() {
        let row = ThemeRow { dir: "d", title: "t", provider: "p", content_ver: "1", size: 0 };
        assert!(plan_insert(&[col("type", true, false)], &row).is_none());
    }
    #[test]
    fn parses_theme_xml() {
        let xml = "<theme><m_title><m_default>My Theme</m_default></m_title><m_contentVer>01.20</m_contentVer></theme>";
        assert_eq!(xml_text(xml, &["m_title", "m_default"]).as_deref(), Some("My Theme"));
        assert_eq!(xml_text(xml, &["m_contentVer"]).as_deref(), Some("01.20"));
        assert_eq!(xml_text(xml, &["m_provider", "m_default"]), None);
    }
}
