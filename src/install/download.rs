use anyhow::{Context, Result, bail};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::watch;
pub trait ByteSink: Send {
    fn resume_offset(&self) -> u64;
    fn write(&mut self, chunk: &[u8]) -> Result<()>;
    // Called when the server ignored our Range header and resent from byte 0.
    fn reset(&mut self) -> Result<()>;
}
// Never truncates on open, or every retry of a multi-GB download would restart from zero.
pub struct FileSink {
    file: std::fs::File,
    written: u64,
}
impl FileSink {
    pub fn open(dest: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(dest)
            .with_context(|| format!("couldn't open {}", dest.display()))?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self { file, written })
    }
}
impl ByteSink for FileSink {
    fn resume_offset(&self) -> u64 {
        self.written
    }
    fn write(&mut self, chunk: &[u8]) -> Result<()> {
        self.file.write_all(chunk).context("couldn't write the download")?;
        self.written += chunk.len() as u64;
        Ok(())
    }
    fn reset(&mut self) -> Result<()> {
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.written = 0;
        Ok(())
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
            Ok(ResumePlan::Continue { from: existing, total })
        }
        StatusCode::OK => {
            if existing > 0 {
                eprintln!("server ignored the Range request, restarting the download from zero");
            }
            Ok(ResumePlan::Restart { total: content_length })
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
    make: fn(u64, Option<u64>) -> crate::install::Progress,
    last_sent: Instant,
    interval: Duration,
}
impl<'a> ProgressReporter<'a> {
    pub fn new(
        tx: &'a watch::Sender<crate::install::Progress>,
        make: fn(u64, Option<u64>) -> crate::install::Progress,
    ) -> Self {
        Self { tx, make, last_sent: Instant::now() - Duration::from_secs(1), interval: Duration::from_millis(250) }
    }
    pub fn record(&mut self, received: u64, total: Option<u64>) {
        if self.last_sent.elapsed() >= self.interval {
            self.last_sent = Instant::now();
            let _ = self.tx.send((self.make)(received, total));
        }
    }
}
const MAX_ATTEMPTS: u32 = 5;
const MAX_BACKOFF: Duration = Duration::from_secs(30);
pub async fn fetch_to(url: &str, sink: &mut dyn ByteSink, rep: &mut ProgressReporter<'_>) -> Result<()> {
    use futures_util::StreamExt;
    let mut last_err = anyhow::anyhow!("download not attempted");
    for attempt in 0u32..MAX_ATTEMPTS {
        if attempt > 0 {
            let backoff = Duration::from_secs(2u64.saturating_pow(attempt)).min(MAX_BACKOFF);
            tokio::time::sleep(backoff).await;
            eprintln!("retrying download (attempt {}): {url}", attempt + 1);
        }
        let start = sink.resume_offset();
        let mut request = crate::net::client().get(url).header("User-Agent", "VitaForge");
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
    fn partial_content_resumes_from_existing_bytes() {
        let plan = plan_resume(1000, reqwest::StatusCode::PARTIAL_CONTENT, Some("bytes 1000-4999/5000"), None).unwrap();
        assert_eq!(plan, ResumePlan::Continue { from: 1000, total: Some(5000) });
    }
    #[test]
    fn ok_status_restarts_even_with_existing_bytes() {
        let plan = plan_resume(1000, reqwest::StatusCode::OK, None, Some(5000)).unwrap();
        assert_eq!(plan, ResumePlan::Restart { total: Some(5000) });
    }
    #[test]
    fn range_not_satisfiable_with_matching_total_is_already_done() {
        let plan =
            plan_resume(5000, reqwest::StatusCode::RANGE_NOT_SATISFIABLE, Some("bytes */5000"), None).unwrap();
        assert_eq!(plan, ResumePlan::AlreadyDone);
    }
    #[test]
    fn range_not_satisfiable_with_mismatched_total_restarts() {
        let plan =
            plan_resume(9000, reqwest::StatusCode::RANGE_NOT_SATISFIABLE, Some("bytes */5000"), None).unwrap();
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
