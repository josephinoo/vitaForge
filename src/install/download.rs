use anyhow::{Context, Result, bail};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tokio::sync::watch;
pub trait ByteSink: Send {
    fn resume_offset(&self) -> u64;
    fn write(&mut self, chunk: &[u8]) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
}
pub struct FileSink {
    file: std::io::BufWriter<std::fs::File>,
    written: u64,
}
impl FileSink {
    pub fn open(dest: &Path) -> Result<Self> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(dest)
            .with_context(|| format!("couldn't open {}", dest.display()))?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        file.seek(SeekFrom::End(0))?;
        let file = std::io::BufWriter::with_capacity(256 * 1024, file);
        Ok(Self { file, written })
    }
}
impl ByteSink for FileSink {
    fn resume_offset(&self) -> u64 {
        self.written
    }
    fn write(&mut self, chunk: &[u8]) -> Result<()> {
        self.file
            .write_all(chunk)
            .context("couldn't write the download")?;
        self.written += chunk.len() as u64;
        Ok(())
    }
    fn reset(&mut self) -> Result<()> {
        self.file.flush()?;
        let inner = self.file.get_mut();
        inner.set_len(0)?;
        inner.seek(SeekFrom::Start(0))?;
        self.written = 0;
        Ok(())
    }
}
impl Drop for FileSink {
    fn drop(&mut self) {
        let _ = self.file.flush();
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ResumePlan {
    Continue { from: u64, total: Option<u64> },
    Restart { total: Option<u64> },
    AlreadyDone,
}
pub fn plan_resume(
    existing: u64,
    status: reqwest::StatusCode,
    content_range: Option<&str>,
    content_length: Option<u64>,
) -> Result<ResumePlan> {
    use reqwest::StatusCode;
    match status {
        StatusCode::PARTIAL_CONTENT => {
            let total = content_range
                .and_then(|range| range.rsplit('/').next())
                .and_then(|total| total.parse::<u64>().ok());
            Ok(ResumePlan::Continue {
                from: existing,
                total,
            })
        }
        StatusCode::OK => {
            if existing > 0 {
                eprintln!("server ignored the Range request, restarting the download from zero");
            }
            Ok(ResumePlan::Restart {
                total: content_length,
            })
        }
        StatusCode::RANGE_NOT_SATISFIABLE => {
            let total = content_range
                .and_then(|range| range.rsplit('/').next())
                .and_then(|total| total.parse::<u64>().ok());
            if total.is_some_and(|total| total == existing) {
                Ok(ResumePlan::AlreadyDone)
            } else {
                Ok(ResumePlan::Restart { total })
            }
        }
        other => bail!("download server returned {other}"),
    }
}
pub struct ProgressReporter<'a> {
    tx: &'a watch::Sender<crate::install::Progress>,
    make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    last_sent: Instant,
    interval: Duration,
    started_at: Instant,
    base: Option<u64>,
}
impl<'a> ProgressReporter<'a> {
    pub fn new(
        tx: &'a watch::Sender<crate::install::Progress>,
        make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    ) -> Self {
        Self {
            tx,
            make,
            last_sent: Instant::now() - Duration::from_secs(1),
            interval: Duration::from_millis(250),
            started_at: Instant::now(),
            base: None,
        }
    }
    pub fn record(&mut self, received: u64, total: Option<u64>) {
        if self.last_sent.elapsed() >= self.interval {
            self.last_sent = Instant::now();
            let base = *self.base.get_or_insert(received);
            let elapsed = eta_elapsed(self.started_at.elapsed().as_secs_f64(), received, base);
            let _ = self.tx.send((self.make)(received, total, elapsed));
        }
    }
}
/// Elapsed time to report so the UI's `remaining = left * elapsed / received`
/// ETA reflects this session's speed, not bytes resumed from an earlier run.
fn eta_elapsed(elapsed_secs: f64, received: u64, base: u64) -> u32 {
    let fresh = received.saturating_sub(base).max(1) as f64;
    (elapsed_secs * received as f64 / fresh) as u32
}
const MAX_ATTEMPTS: u32 = 5;
const MAX_BACKOFF: Duration = Duration::from_secs(30);
pub const PARALLEL_PARTS: usize = 4;
const MIN_PARALLEL_BYTES: u64 = 4 * 1024 * 1024;

pub fn split_ranges(total: u64, parts: usize) -> Vec<(u64, u64)> {
    if total == 0 || parts == 0 {
        return Vec::new();
    }
    let parts = (parts as u64).min(total);
    let chunk = total / parts;
    (0..parts)
        .map(|i| {
            let start = i * chunk;
            let end = if i + 1 == parts { total - 1 } else { (i + 1) * chunk - 1 };
            (start, end)
        })
        .collect()
}

struct ParallelProbe {
    final_url: String,
    total: u64,
}

async fn probe_parallel(url: &str) -> Option<ParallelProbe> {
    // shortcut: 1-byte Range probe instead of HEAD because GitHub/Archive redirects strip Accept-Ranges on HEAD, upgrade if custom CDN added
    let resp = crate::net::client()
        .get(url)
        .header("Range", "bytes=0-0")
        .send()
        .await
        .ok()?;
    if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return None;
    }
    let total = resp
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|r| r.rsplit('/').next())
        .and_then(|t| t.parse::<u64>().ok())?;
    if total < MIN_PARALLEL_BYTES {
        return None;
    }
    Some(ParallelProbe {
        final_url: resp.url().as_str().to_owned(),
        total,
    })
}

async fn fetch_range_part(
    url: &str,
    dest: &Path,
    start: u64,
    end: u64,
    received_total: &std::sync::atomic::AtomicU64,
    last_sent_ms: &std::sync::atomic::AtomicU64,
    tx: &watch::Sender<crate::install::Progress>,
    make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    started_at: Instant,
    total: u64,
    base: u64,
    cancel: &AtomicBool,
) -> Result<()> {
    use futures_util::StreamExt;
    use std::sync::atomic::Ordering;
    let mut last_err = anyhow::anyhow!("range part not attempted");
    let done = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    let file = if done <= end - start + 1 {
        std::fs::OpenOptions::new().create(true).append(true).open(dest)
    } else {
        std::fs::File::create(dest)
    }
    .with_context(|| format!("couldn't open {}", dest.display()))?;
    let mut pos = start + file.metadata()?.len();
    let mut writer = std::io::BufWriter::with_capacity(512 * 1024, file);
    for attempt in 0u32..MAX_ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            bail!(crate::install::CANCELLED_MESSAGE);
        }
        if attempt > 0 {
            let backoff = Duration::from_secs(2u64.saturating_pow(attempt)).min(MAX_BACKOFF);
            tokio::time::sleep(backoff).await;
        }
        if pos > end {
            writer.flush()?;
            return Ok(());
        }
        let resp = match crate::net::client()
            .get(url)
            .header("Range", format!("bytes={pos}-{end}"))
            .send()
            .await
        {
            Ok(r) if r.status() == reqwest::StatusCode::PARTIAL_CONTENT => r,
            Ok(r) => {
                last_err = anyhow::anyhow!("range server returned {}", r.status());
                continue;
            }
            Err(err) => {
                last_err = anyhow::Error::new(err).context("range connection failed");
                continue;
            }
        };
        let mut stream = resp.bytes_stream();
        let mut failed = false;
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                bail!(crate::install::CANCELLED_MESSAGE);
            }
            let chunk = match chunk {
                Ok(c) => c,
                Err(err) => {
                    last_err = anyhow::Error::new(err).context("range stream interrupted");
                    failed = true;
                    break;
                }
            };
            let remaining = (end - pos + 1) as usize;
            let slice = &chunk[..chunk.len().min(remaining)];
            writer.write_all(slice)?;
            pos += slice.len() as u64;
            let cur = received_total.fetch_add(slice.len() as u64, Ordering::Relaxed)
                + slice.len() as u64;
            let elapsed_ms = started_at.elapsed().as_millis() as u64;
            let prev_ms = last_sent_ms.load(Ordering::Relaxed);
            if elapsed_ms.saturating_sub(prev_ms) >= 250
                && last_sent_ms
                    .compare_exchange(prev_ms, elapsed_ms, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                let elapsed = eta_elapsed(elapsed_ms as f64 / 1000.0, cur, base);
                let _ = tx.send(make(cur, Some(total), elapsed));
            }
            if pos > end {
                break;
            }
        }
        if !failed && pos > end {
            writer.flush()?;
            return Ok(());
        }
    }
    Err(last_err)
}

/// `https://archive.org/download/<id>/<file>` -> (`id`, `<file>` still percent-encoded).
fn archive_parts(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://archive.org/download/")
        .or_else(|| url.strip_prefix("http://archive.org/download/"))?;
    let (id, file) = rest.split_once('/')?;
    (!id.is_empty() && !file.is_empty()).then(|| (id.to_owned(), file.to_owned()))
}

/// archive.org's redirect picks one storage node, often a slow one; its mirrors
/// can differ 8x. Like PSVitaAlive: list the item's nodes, time a 256 KB range
/// from each, and download from the fastest. None keeps the original url.
/// Nodes are fetched over plain http (TLS decrypt was the Vita's CPU ceiling);
/// the md5 from the https metadata is returned so the file can be verified.
async fn fastest_archive_node(url: &str) -> Option<(String, Option<String>)> {
    use futures_util::StreamExt;
    const PROBE_BYTES: usize = 256 * 1024;
    let (id, file) = archive_parts(url)?;
    let meta: serde_json::Value = crate::net::client()
        .get(format!("https://archive.org/metadata/{id}"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let dir = meta["dir"].as_str()?;
    let name = percent_encoding::percent_decode_str(&file).decode_utf8().ok()?;
    let md5 = meta["files"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|f| f["name"].as_str() == Some(&*name))
        .and_then(|f| f["md5"].as_str())
        .map(str::to_owned);
    let mut hosts: Vec<&str> = Vec::new();
    let listed = meta["workable_servers"].as_array().into_iter().flatten();
    for host in listed.chain([&meta["d1"], &meta["d2"], &meta["server"]]).filter_map(|v| v.as_str()) {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    let mut best: Option<(f64, String)> = None;
    for host in hosts {
        // shortcut: plain http only when the md5 can vouch for the bytes.
        let scheme = if md5.is_some() { "http" } else { "https" };
        let candidate = format!("{scheme}://{host}{dir}/{file}");
        let started = Instant::now();
        let probe = async {
            let resp = crate::net::client()
                .get(&candidate)
                .header("Range", format!("bytes=0-{}", PROBE_BYTES - 1))
                .send()
                .await
                .ok()?;
            if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return None;
            }
            let mut got = 0usize;
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                got += chunk.ok()?.len();
            }
            (got > 0).then_some(got)
        };
        let Ok(Some(got)) = tokio::time::timeout(Duration::from_secs(12), probe).await else {
            crate::install::log_file(&format!("archive node {host}: probe failed"));
            continue;
        };
        let speed = got as f64 / started.elapsed().as_secs_f64().max(0.001);
        crate::install::log_file(&format!("archive node {host}: {:.0} KB/s", speed / 1024.0));
        if best.as_ref().is_none_or(|(s, _)| speed > *s) {
            best = Some((speed, candidate));
        }
    }
    best.map(|(_, u)| (u, md5))
}

/// Range part `i` of `dest`. Vita's ux0 can't grow a file past its end
/// (EINVAL from ftruncate and from seek+write), so each parallel part streams
/// into its own file; part 0 is `dest` itself, so a single-stream partial is
/// its prefix. The rest are appended onto `dest` once all parts finish.
pub fn part_path(dest: &Path, i: usize) -> std::path::PathBuf {
    if i == 0 {
        dest.to_path_buf()
    } else {
        std::path::PathBuf::from(format!("{}.part{i}", dest.display()))
    }
}
pub fn remove_parts(dest: &Path) {
    for i in 1..PARALLEL_PARTS {
        let _ = std::fs::remove_file(part_path(dest, i));
    }
}
/// Bytes on disk for `dest` across all its parts.
fn downloaded_len_of(dest: &Path, i: usize) -> u64 {
    std::fs::metadata(part_path(dest, i)).map(|m| m.len()).unwrap_or(0)
}
pub fn downloaded_len(dest: &Path) -> u64 {
    (0..PARALLEL_PARTS)
        .map(|i| std::fs::metadata(part_path(dest, i)).map(|m| m.len()).unwrap_or(0))
        .sum()
}
/// Streams `paths` in order through `sink`, reporting whole-percent progress.
fn stream_files(
    paths: &[(std::path::PathBuf, u64)],
    mut sink: impl FnMut(&[u8]) -> std::io::Result<()>,
    mut on_percent: impl FnMut(u32),
) -> Result<()> {
    use std::io::Read;
    let total: u64 = paths
        .iter()
        .filter_map(|(p, skip)| std::fs::metadata(p).ok().map(|m| m.len().saturating_sub(*skip)))
        .sum();
    let mut done = 0u64;
    let mut last = u32::MAX;
    let mut buf = vec![0u8; 1024 * 1024];
    for (path, skip) in paths {
        let mut src = std::fs::File::open(path).with_context(|| format!("couldn't read {}", path.display()))?;
        src.seek(SeekFrom::Start(*skip))?;
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            sink(&buf[..n])?;
            done += n as u64;
            let pct = (done * 100 / total.max(1)) as u32;
            if pct != last {
                last = pct;
                on_percent(pct);
            }
        }
    }
    Ok(())
}
/// Appends parts 1.. onto `dest`. `starts[i]` is part i's byte offset; a join
/// cut short leaves `dest` a valid prefix, so it resumes from `dest`'s length.
fn join_parts(dest: &Path, starts: &[u64], on_percent: impl FnMut(u32)) -> Result<()> {
    let mut out = std::fs::OpenOptions::new().append(true).open(dest)?;
    let joined = out.metadata()?.len();
    let parts: Vec<_> = (1..PARALLEL_PARTS)
        .filter(|&i| joined < starts[i] + std::fs::metadata(part_path(dest, i)).map(|m| m.len()).unwrap_or(0))
        .map(|i| (part_path(dest, i), joined.saturating_sub(starts[i])))
        .collect();
    stream_files(&parts, |b| out.write_all(b), on_percent)?;
    out.flush()?;
    remove_parts(dest);
    Ok(())
}

/// Reads `dest` and its `.partN` files as the one file they make up. Part i
/// sits at byte i * (total / parts); `dest` comes first, so a half-joined
/// `dest` that already covers part of part 1 wins over the part file.
pub struct PartsReader {
    files: Vec<(std::fs::File, u64, u64)>,
    pos: u64,
    total: u64,
}
impl PartsReader {
    pub fn open(dest: &Path) -> std::io::Result<Self> {
        let lens: Vec<u64> = (0..PARALLEL_PARTS).map(|i| downloaded_len_of(dest, i)).collect();
        let mut files = vec![(std::fs::File::open(dest)?, 0, lens[0])];
        if part_path(dest, PARALLEL_PARTS - 1).exists() {
            // Only read once every part is complete, so part 1 is exactly total / parts long.
            let chunk = lens[1];
            for i in 1..PARALLEL_PARTS {
                files.push((std::fs::File::open(part_path(dest, i))?, i as u64 * chunk, lens[i]));
            }
        }
        let total = files.iter().map(|(_, start, len)| start + len).max().unwrap_or(0);
        Ok(Self { files, pos: 0, total })
    }
}
impl std::io::Read for PartsReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let pos = self.pos;
        let Some((file, start, len)) = self.files.iter_mut().find(|(_, s, l)| pos >= *s && pos < *s + *l) else {
            return Ok(0);
        };
        let off = pos - *start;
        file.seek(SeekFrom::Start(off))?;
        let want = buf.len().min((*len - off) as usize);
        let n = file.read(&mut buf[..want])?;
        self.pos += n as u64;
        Ok(n)
    }
}
impl std::io::Seek for PartsReader {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let next = match to {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::End(n) => self.total as i128 + n as i128,
            SeekFrom::Current(n) => self.pos as i128 + n as i128,
        };
        if next < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

/// Md5s every byte of the stream exactly once, in order, while the zip reader
/// reads it in roughly that order; `finish` hashes whatever it skipped
/// (the central directory, any gaps) from the first unhashed byte to the end.
pub struct HashingReader<R> {
    inner: R,
    pos: u64,
    pub(crate) hashed: u64,
    hasher: md5::Md5,
}
impl<R: std::io::Read + std::io::Seek> HashingReader<R> {
    pub fn new(inner: R) -> Self {
        use md5::Digest;
        Self { inner, pos: 0, hashed: 0, hasher: md5::Md5::new() }
    }
    pub fn finish(
        mut self,
        mut on_progress: impl FnMut(u32) -> std::io::Result<()>,
    ) -> std::io::Result<String> {
        use md5::Digest;
        let total = self.inner.seek(SeekFrom::End(0))?;
        self.inner.seek(SeekFrom::Start(self.hashed))?;
        on_progress((self.hashed.saturating_mul(100) / total.max(1)).min(100) as u32)?;
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = self.inner.read(&mut buf)?;
            if n == 0 {
                break;
            }
            self.hasher.update(&buf[..n]);
            self.hashed += n as u64;
            on_progress((self.hashed.saturating_mul(100) / total.max(1)).min(100) as u32)?;
        }
        on_progress(100)?;
        Ok(format!("{:x}", self.hasher.finalize()))
    }
}
impl<R: std::io::Read> std::io::Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use md5::Digest;
        let n = self.inner.read(buf)?;
        let end = self.pos + n as u64;
        if self.pos <= self.hashed && self.hashed < end {
            self.hasher.update(&buf[(self.hashed - self.pos) as usize..n]);
            self.hashed = end;
        }
        self.pos = end;
        Ok(n)
    }
}
impl<R: std::io::Read + std::io::Seek> std::io::Seek for HashingReader<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        use md5::Digest;
        let target = self.inner.seek(to)?;
        // zip skips each entry's name/extra field with a seek; hash that small gap
        // now, or `hashed` stalls at the first entry and finish() re-reads the whole file.
        if target > self.hashed && target - self.hashed <= 64 * 1024 && self.pos >= self.hashed {
            self.inner.seek(SeekFrom::Start(self.hashed))?;
            let mut gap = vec![0u8; (target - self.hashed) as usize];
            self.inner.read_exact(&mut gap)?;
            self.hasher.update(&gap);
            self.hashed = target;
        }
        self.pos = target;
        Ok(self.pos)
    }
}

pub async fn fetch_file(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<crate::install::Progress>,
    make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    cancel: &AtomicBool,
) -> Result<()> {
    let Some((node_url, md5)) = fastest_archive_node(url).await else {
        return fetch_file_from(url, dest, tx, make, cancel, true).await;
    };
    fetch_file_from(&node_url, dest, tx, make, cancel, true).await?;
    let Some(expected) = md5 else {
        return Ok(());
    };
    crate::install::log_file("verifying md5");
    let path = dest.to_path_buf();
    let tx_md5 = tx.clone();
    let actual = tokio::task::spawn_blocking(move || {
        use md5::{Digest, Md5};
        let mut hasher = Md5::new();
        stream_files(
            &[(path, 0)],
            |b| {
                hasher.update(b);
                Ok(())
            },
            |percent| {
                let _ = tx_md5.send(crate::install::Progress::Finishing { step: "Verifying", percent });
            },
        )
        .ok()
        .map(|()| format!("{:x}", hasher.finalize()))
    })
    .await
    .ok()
    .flatten();
    crate::install::log_file(&format!("md5 {}", if actual.as_deref() == Some(expected.as_str()) { "ok" } else { "MISMATCH" }));
    if actual.as_deref() != Some(expected.as_str()) {
        let _ = std::fs::remove_file(dest);
        bail!("the download was corrupted (md5 mismatch), please try again");
    }
    Ok(())
}

/// Like `fetch_file`, but leaves parallel parts unjoined and doesn't verify:
/// the caller reads them through `PartsReader` and checks the returned md5
/// while extracting, so the 3 GB on the memory card is read once, not three times.
pub async fn fetch_parts(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<crate::install::Progress>,
    make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    cancel: &AtomicBool,
) -> Result<Option<String>> {
    let Some((node_url, md5)) = fastest_archive_node(url).await else {
        fetch_file_from(url, dest, tx, make, cancel, false).await?;
        return Ok(None);
    };
    fetch_file_from(&node_url, dest, tx, make, cancel, false).await?;
    Ok(md5)
}

async fn fetch_file_from(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<crate::install::Progress>,
    make: fn(u64, Option<u64>, u32) -> crate::install::Progress,
    cancel: &AtomicBool,
    join: bool,
) -> Result<()> {
    let probe = probe_parallel(url).await;
    if probe.is_none() {
        crate::install::log_file("parallel: range probe failed, using one connection");
    }
    let ranges = probe.as_ref().map(|p| split_ranges(p.total, PARALLEL_PARTS));
    // A single-stream partial longer than part 0 can't be split; finish it as one stream.
    let fits = ranges.as_ref().is_some_and(|r| {
        r.len() == PARALLEL_PARTS
            && std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0) <= r[0].1 + 1
    });
    let complete_parts = ranges.as_ref().is_some_and(|r| {
        r.len() == PARALLEL_PARTS
            && (0..PARALLEL_PARTS).all(|i| downloaded_len_of(dest, i) == r[i].1 - r[i].0 + 1 || i == 0)
            && part_path(dest, 1).exists()
    });
    if let (Some(probe), Some(ranges), true) = (probe.as_ref(), ranges.as_ref(), complete_parts && !fits) {
        if !join {
            // PartsReader reads the half-joined prefix from `dest`, the rest from parts.
            let _ = tx.send(make(probe.total, Some(probe.total), 0));
            return Ok(());
        }
        crate::install::log_file("parallel: resuming an interrupted join");
        let starts: Vec<u64> = ranges.iter().map(|r| r.0).collect();
        let owned = dest.to_path_buf();
        let tx_join = tx.clone();
        tokio::task::spawn_blocking(move || {
            join_parts(&owned, &starts, |percent| {
                let _ = tx_join.send(crate::install::Progress::Finishing { step: "Joining parts", percent });
            })
        })
        .await
        .context("the join worker crashed")??;
        let _ = tx.send(make(probe.total, Some(probe.total), 0));
        return Ok(());
    }
    if let (Some(probe), Some(ranges), true) = (probe, ranges, fits) {
        let base = downloaded_len(dest);
        crate::install::log_file(&format!("parallel: {PARALLEL_PARTS} part files, resuming at {base} bytes"));
        let received = std::sync::atomic::AtomicU64::new(base);
        let last_sent_ms = std::sync::atomic::AtomicU64::new(0);
        let started_at = Instant::now();
        let parts: Vec<_> = (0..PARALLEL_PARTS).map(|i| part_path(dest, i)).collect();
        let starts: Vec<u64> = ranges.iter().map(|r| r.0).collect();
        let futs = ranges.into_iter().zip(&parts).map(|((start, end), part)| {
            fetch_range_part(
                &probe.final_url,
                part,
                start,
                end,
                &received,
                &last_sent_ms,
                tx,
                make,
                started_at,
                probe.total,
                base,
                cancel,
            )
        });
        // Parts are kept on error so the next try resumes each one.
        futures_util::future::try_join_all(futs).await?;
        if !join {
            crate::install::log_file("parallel: all parts done");
            let elapsed = eta_elapsed(started_at.elapsed().as_secs_f64(), probe.total, base);
            let _ = tx.send(make(probe.total, Some(probe.total), elapsed));
            return Ok(());
        }
        crate::install::log_file("parallel: all parts done, joining");
        let owned = dest.to_path_buf();
        let tx_join = tx.clone();
        tokio::task::spawn_blocking(move || {
            join_parts(&owned, &starts, |percent| {
                let _ = tx_join.send(crate::install::Progress::Finishing { step: "Joining parts", percent });
            })
        })
        .await
        .context("the join worker crashed")??;
        crate::install::log_file("parallel: joined");
        let elapsed = eta_elapsed(started_at.elapsed().as_secs_f64(), probe.total, base);
        let _ = tx.send(make(probe.total, Some(probe.total), elapsed));
        return Ok(());
    }
    remove_parts(dest);
    let mut sink = FileSink::open(dest)?;
    let mut reporter = ProgressReporter::new(tx, make);
    fetch_to(url, &mut sink, &mut reporter, cancel).await
}

pub async fn fetch_to(
    url: &str,
    sink: &mut dyn ByteSink,
    rep: &mut ProgressReporter<'_>,
    cancel: &AtomicBool,
) -> Result<()> {
    use futures_util::StreamExt;
    let mut last_err = anyhow::anyhow!("download not attempted");
    for attempt in 0u32..MAX_ATTEMPTS {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!(crate::install::CANCELLED_MESSAGE);
        }
        if attempt > 0 {
            let backoff = Duration::from_secs(2u64.saturating_pow(attempt)).min(MAX_BACKOFF);
            tokio::time::sleep(backoff).await;
            eprintln!("retrying download (attempt {}): {url}", attempt + 1);
        }
        let start = sink.resume_offset();
        let mut request = crate::net::client().get(url);
        if start > 0 {
            request = request.header("Range", format!("bytes={start}-"));
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                last_err = anyhow::Error::new(err).context("couldn't reach the download server");
                continue;
            }
        };
        let status = response.status();
        let content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        let plan = match plan_resume(start, status, content_range.as_deref(), content_length) {
            Ok(plan) => plan,
            Err(err) => {
                last_err = err;
                continue;
            }
        };
        let total = match plan {
            ResumePlan::AlreadyDone => return Ok(()),
            ResumePlan::Restart { total } => {
                if let Err(err) = sink.reset() {
                    last_err = err;
                    continue;
                }
                total
            }
            ResumePlan::Continue { total, .. } => total,
        };
        let mut received = sink.resume_offset();
        let mut stream = response.bytes_stream();
        let mut attempt_failed = false;
        while let Some(chunk) = stream.next().await {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                bail!(crate::install::CANCELLED_MESSAGE);
            }
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    last_err = anyhow::Error::new(err).context("the download was interrupted");
                    attempt_failed = true;
                    break;
                }
            };
            if let Err(err) = sink.write(&chunk) {
                last_err = err;
                attempt_failed = true;
                break;
            }
            received += chunk.len() as u64;
            rep.record(received, total);
        }
        if !attempt_failed {
            return Ok(());
        }
    }
    Err(last_err)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_parts_appends_in_order_and_cleans_up() {
        let dest = std::env::temp_dir().join(format!("vf_join_{}", std::process::id()));
        std::fs::write(&dest, b"aa").unwrap();
        for i in 1..PARALLEL_PARTS {
            std::fs::write(part_path(&dest, i), format!("{i}{i}")).unwrap();
        }
        assert_eq!(downloaded_len(&dest), 2 * PARALLEL_PARTS as u64);
        join_parts(&dest, &[0, 2, 4, 6], |_| {}).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"aa112233");
        // A join cut off one byte into part 2 picks up from there.
        std::fs::write(&dest, b"aa112").unwrap();
        for i in 1..PARALLEL_PARTS {
            std::fs::write(part_path(&dest, i), format!("{i}{i}")).unwrap();
        }
        join_parts(&dest, &[0, 2, 4, 6], |_| {}).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"aa112233");
        assert!(!part_path(&dest, 1).exists());
        let _ = std::fs::remove_file(&dest);
    }
    #[test]
    fn eta_elapsed_ignores_resumed_bytes() {
        // 100 s for 1 GB fresh on top of 2 GB resumed: 3 GB at the fresh rate is 300 s.
        assert_eq!(eta_elapsed(100.0, 3_000, 2_000), 300);
        assert_eq!(eta_elapsed(100.0, 1_000, 0), 100);
    }
    #[test]
    fn archive_parts_splits_id_and_encoded_file() {
        assert_eq!(
            archive_parts("https://archive.org/download/mkwiita/Game%20Files.zip"),
            Some(("mkwiita".into(), "Game%20Files.zip".into()))
        );
        assert_eq!(archive_parts("https://github.com/a/b/releases/x.zip"), None);
        assert_eq!(archive_parts("https://archive.org/download/onlyid"), None);
    }
    #[test]
    fn split_ranges_covers_exact_total_without_gaps() {
        assert_eq!(
            split_ranges(100, 4),
            vec![(0, 24), (25, 49), (50, 74), (75, 99)]
        );
        assert_eq!(
            split_ranges(103, 4),
            vec![(0, 24), (25, 49), (50, 74), (75, 102)]
        );
    }
    #[test]
    fn partial_content_resumes_from_existing_bytes() {
        let plan = plan_resume(
            1000,
            reqwest::StatusCode::PARTIAL_CONTENT,
            Some("bytes 1000-4999/5000"),
            None,
        )
        .unwrap();
        assert_eq!(
            plan,
            ResumePlan::Continue {
                from: 1000,
                total: Some(5000)
            }
        );
    }
    #[test]
    fn ok_status_restarts_even_with_existing_bytes() {
        let plan = plan_resume(1000, reqwest::StatusCode::OK, None, Some(5000)).unwrap();
        assert_eq!(plan, ResumePlan::Restart { total: Some(5000) });
    }
    #[test]
    fn range_not_satisfiable_with_matching_total_is_already_done() {
        let plan = plan_resume(
            5000,
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE,
            Some("bytes */5000"),
            None,
        )
        .unwrap();
        assert_eq!(plan, ResumePlan::AlreadyDone);
    }
    #[test]
    fn range_not_satisfiable_with_mismatched_total_restarts() {
        let plan = plan_resume(
            9000,
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE,
            Some("bytes */5000"),
            None,
        )
        .unwrap();
        assert_eq!(plan, ResumePlan::Restart { total: Some(5000) });
    }
    #[test]
    fn other_status_is_an_error() {
        assert!(plan_resume(0, reqwest::StatusCode::NOT_FOUND, None, None).is_err());
    }
    #[test]
    fn file_sink_reset_truncates_and_reports_zero() {
        let dir = std::env::temp_dir().join(format!("vitaforge_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sink.bin");
        let mut sink = FileSink::open(&path).unwrap();
        sink.write(b"hello").unwrap();
        assert_eq!(sink.resume_offset(), 5);
        sink.reset().unwrap();
        assert_eq!(sink.resume_offset(), 0);
        assert_eq!(std::fs::read(&path).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn file_sink_reopen_does_not_truncate() {
        let dir = std::env::temp_dir().join(format!("vitaforge_test2_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sink.bin");
        {
            let mut sink = FileSink::open(&path).unwrap();
            sink.write(b"hello world").unwrap();
        }
        let sink = FileSink::open(&path).unwrap();
        assert_eq!(sink.resume_offset(), 11);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
