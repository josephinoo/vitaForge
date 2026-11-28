//! Queue shared with the shell plugin (`plugin/`), which downloads and extracts
#![allow(dead_code)] // not wired into the install flow until the plugin is proven on hardware
//! while VitaForge is closed. Same file protocol as `plugin/jobs.h`:
//!   <id>.job    written here, never modified afterwards
//!   <id>.state  written by the plugin (atomic tmp + rename)
//!   <id>.ctl    "pause" | "resume" | "cancel", written here, consumed by the plugin
//!   heartbeat   unix time, rewritten by the plugin about every 2 s
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where jobs live on the console (the plugin watches this exact directory).
pub const QUEUE_DIR: &str = "ux0:data/vitaforge/bg";
/// Loopback port the plugin serves native-bar downloads on (`proxy_vita.h`).
pub const PROXY_PORT: u16 = 19851;
/// The plugin is considered running if its heartbeat is newer than this.
const ALIVE_SECS: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// zip saved at `dest`, extracted into `extract_to`, then deleted
    Data,
    /// file kept at `dest`; VitaForge installs it when reopened
    Vpk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub url: String,
    pub dest: String,
    pub extract_to: String,
    pub name: String,
    pub md5: String,
    pub kind: Kind,
    pub size: u64,
    pub overwrite: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Queued,
    Downloading,
    Paused,
    Verifying,
    Extracting,
    Completed,
    Failed,
    Cancelled,
}

impl Status {
    fn parse(text: &str) -> Status {
        match text {
            "queued" => Status::Queued,
            "downloading" => Status::Downloading,
            "paused" => Status::Paused,
            "verifying" => Status::Verifying,
            "extracting" => Status::Extracting,
            "completed" => Status::Completed,
            "cancelled" => Status::Cancelled,
            _ => Status::Failed,
        }
    }
    pub fn is_final(self) -> bool {
        matches!(self, Status::Completed | Status::Failed | Status::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub status: Status,
    pub downloaded: u64,
    pub total: u64,
    pub extract_done: u64,
    pub extract_total: u64,
    pub updated: u64,
    pub error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Pause,
    Resume,
    Cancel,
}

fn get<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
        .map(|value| value.trim_end_matches('\r'))
}

fn num(text: &str, key: &str) -> u64 {
    get(text, key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// A value must stay on one line or the plugin's parser would misread the file.
fn one_line(value: &str) -> Result<&str> {
    anyhow::ensure!(!value.contains('\n') && !value.contains('\r'), "job field has a line break");
    Ok(value)
}

fn path(dir: &Path, id: &str, ext: &str) -> PathBuf {
    dir.join(format!("{id}.{ext}"))
}

/// tmp + rename: the plugin never sees a half-written job.
fn write_atomic(target: &Path, body: &str) -> Result<()> {
    let tmp = target.with_extension("tmp");
    std::fs::write(&tmp, body).with_context(|| format!("couldn't write {}", tmp.display()))?;
    std::fs::rename(&tmp, target).with_context(|| format!("couldn't publish {}", target.display()))
}

/// Queues a job; returns its id. The plugin picks it up within ~2 s.
pub fn enqueue(dir: &Path, job: &Job) -> Result<String> {
    enqueue_with(dir, job, false)
}

/// Queues a job whose bytes the native download bar fetches; returns the id and the
/// loopback URL to hand to SceDownload. The size is required: the proxy announces it.
pub fn enqueue_native(dir: &Path, job: &Job) -> Result<(String, String)> {
    anyhow::ensure!(job.size > 0, "a native-bar download needs its size");
    let id = enqueue_with(dir, job, true)?;
    let url = format!("http://127.0.0.1:{PROXY_PORT}/dl/{id}.jpg");
    Ok((id, url))
}

/// Forgets a job that SceDownload refused.
pub fn discard(dir: &Path, id: &str) {
    let _ = std::fs::remove_file(path(dir, id, "job"));
}

/// The plugin runs and its export hook is installed.
pub fn native_ready(dir: &Path, now_unix: u64) -> bool {
    plugin_alive(dir, now_unix) && dir.join("hook").exists()
}

fn enqueue_with(dir: &Path, job: &Job, native: bool) -> Result<String> {
    std::fs::create_dir_all(dir).with_context(|| format!("couldn't create {}", dir.display()))?;
    let id = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis());
    let body = format!(
        "url={}\ndest={}\nextract_to={}\nname={}\nmd5={}\nkind={}\nsize={}\noverwrite={}\ncreated={}\nbgdl={}\n",
        one_line(&job.url)?,
        one_line(&job.dest)?,
        one_line(&job.extract_to)?,
        one_line(&job.name)?,
        one_line(&job.md5)?,
        if job.kind == Kind::Data { "data" } else { "vpk" },
        job.size,
        u8::from(job.overwrite),
        id,
        u8::from(native),
    );
    write_atomic(&path(dir, &id, "job"), &body)?;
    Ok(id)
}

pub fn read_job(dir: &Path, id: &str) -> Option<Job> {
    let text = std::fs::read_to_string(path(dir, id, "job")).ok()?;
    Some(Job {
        url: get(&text, "url")?.to_string(),
        dest: get(&text, "dest")?.to_string(),
        extract_to: get(&text, "extract_to").unwrap_or("").to_string(),
        name: get(&text, "name").unwrap_or("").to_string(),
        md5: get(&text, "md5").unwrap_or("").to_string(),
        kind: if get(&text, "kind") == Some("vpk") { Kind::Vpk } else { Kind::Data },
        size: num(&text, "size"),
        overwrite: get(&text, "overwrite") == Some("1"),
    })
}

/// `None` until the plugin has seen the job (so show it as "queued").
pub fn read_state(dir: &Path, id: &str) -> Option<State> {
    let text = std::fs::read_to_string(path(dir, id, "state")).ok()?;
    Some(State {
        status: Status::parse(get(&text, "status").unwrap_or("")),
        downloaded: num(&text, "downloaded"),
        total: num(&text, "total"),
        extract_done: num(&text, "extract_done"),
        extract_total: num(&text, "extract_total"),
        updated: num(&text, "updated"),
        error: get(&text, "error").unwrap_or("").to_string(),
    })
}

/// Job ids, newest first (ids are hex millisecond timestamps).
pub fn list(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".job").map(str::to_string))
        .collect();
    ids.sort_by_key(|id| std::cmp::Reverse((id.len(), id.clone())));
    ids
}

pub fn control(dir: &Path, id: &str, action: Control) -> Result<()> {
    let word = match action {
        Control::Pause => "pause",
        Control::Resume => "resume",
        Control::Cancel => "cancel",
    };
    // plain write: replacing an existing .ctl via rename isn't reliable on ux0, and it is 5 bytes
    std::fs::write(path(dir, id, "ctl"), word).context("couldn't send the command to the background worker")
}

/// True if the plugin refreshed its heartbeat recently.
pub fn plugin_alive(dir: &Path, now_unix: u64) -> bool {
    std::fs::read_to_string(dir.join("heartbeat"))
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .is_some_and(|beat| now_unix.saturating_sub(beat) <= ALIVE_SECS && beat <= now_unix + ALIVE_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vf_bg_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn job() -> Job {
        Job {
            url: "https://example.com/a.zip?x=1&y=2=3".into(),
            dest: "ux0:data/vitaforge/bg/x.zip".into(),
            extract_to: "ux0:data/Game".into(),
            name: "Game Data".into(),
            md5: "d41d8cd98f00b204e9800998ecf8427e".into(),
            kind: Kind::Data,
            size: 5_368_709_120,
            overwrite: true,
        }
    }

    #[test]
    fn job_round_trips_through_its_file() {
        let dir = scratch("job");
        let id = enqueue(&dir, &job()).unwrap();
        assert_eq!(read_job(&dir, &id), Some(job()));
        assert_eq!(list(&dir), vec![id]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_job_field_with_a_newline_is_refused() {
        let dir = scratch("nl");
        let mut bad = job();
        bad.name = "evil\nurl=http://x".into();
        assert!(enqueue(&dir, &bad).is_err());
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_what_the_c_plugin_writes() {
        let dir = scratch("state");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.state"),
            "status=extracting\ndownloaded=100\ntotal=100\nextract_done=5\nextract_total=9\nupdated=1795000000\nerror=\n",
        )
        .unwrap();
        let state = read_state(&dir, "a").unwrap();
        assert_eq!(state.status, Status::Extracting);
        assert_eq!((state.downloaded, state.extract_done, state.extract_total), (100, 5, 9));
        assert!(read_state(&dir, "missing").is_none());
        std::fs::write(dir.join("b.state"), "status=failed\nerror=network error: gave up after 5 retries\n").unwrap();
        let failed = read_state(&dir, "b").unwrap();
        assert!(failed.status.is_final() && failed.error.starts_with("network error"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_status_is_treated_as_failed() {
        assert_eq!(Status::parse("nonsense"), Status::Failed);
    }

    #[test]
    fn controls_overwrite_each_other() {
        let dir = scratch("ctl");
        std::fs::create_dir_all(&dir).unwrap();
        control(&dir, "a", Control::Pause).unwrap();
        control(&dir, "a", Control::Cancel).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.ctl")).unwrap(), "cancel");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn heartbeat_decides_if_the_plugin_is_alive() {
        let dir = scratch("hb");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!plugin_alive(&dir, 1000), "no heartbeat file");
        std::fs::write(dir.join("heartbeat"), "990").unwrap();
        assert!(plugin_alive(&dir, 1000));
        assert!(!plugin_alive(&dir, 1100), "stale for 110 s");
        std::fs::write(dir.join("heartbeat"), "garbage").unwrap();
        assert!(!plugin_alive(&dir, 1000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn native_jobs_point_the_shell_at_the_proxy() {
        let dir = scratch("native");
        let (id, url) = enqueue_native(&dir, &job()).unwrap();
        assert_eq!(url, format!("http://127.0.0.1:19851/dl/{id}.jpg"));
        assert!(std::fs::read_to_string(dir.join(format!("{id}.job"))).unwrap().contains("\nbgdl=1\n"));
        assert!(enqueue_native(&dir, &Job { size: 0, ..job() }).is_err());
        std::fs::write(dir.join("heartbeat"), "1000").unwrap();
        assert!(!native_ready(&dir, 1000), "no hook");
        std::fs::write(dir.join("hook"), "").unwrap();
        assert!(native_ready(&dir, 1000));
        discard(&dir, &id);
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_is_newest_first() {
        let dir = scratch("list");
        std::fs::create_dir_all(&dir).unwrap();
        for id in ["9", "a", "10", "ff"] {
            std::fs::write(dir.join(format!("{id}.job")), "url=x\ndest=y\n").unwrap();
        }
        std::fs::write(dir.join("a.state"), "status=queued\n").unwrap();
        assert_eq!(list(&dir), vec!["ff", "10", "a", "9"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
