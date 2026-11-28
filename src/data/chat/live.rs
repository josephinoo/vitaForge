//! The server's chat socket, used only as a "something changed" signal: messages are
//! still fetched over REST, so frame payloads are skipped.

use std::time::Duration;

use reqwest::{Method, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq)]
pub enum Event {
    Up,
    Changed,
    Down,
}

/// Closes the socket when dropped.
pub struct Socket(JoinHandle<()>);

impl Drop for Socket {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub fn open(events: mpsc::UnboundedSender<Event>) -> Socket {
    Socket(tokio::spawn(async move {
        loop {
            if let Err(err) = listen(&events).await {
                crate::install::log_file(&format!("chat socket: {err:#}"));
            }
            if events.send(Event::Down).is_err() {
                return;
            }
            tokio::time::sleep(RETRY).await;
        }
    }))
}

async fn listen(events: &mpsc::UnboundedSender<Event>) -> anyhow::Result<()> {
    let request = crate::data::api::request(Method::GET, "/me/chat/ws")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        // Only guards against caching proxies; TLS already authenticates the server.
        .header("Sec-WebSocket-Key", "dml0YWZvcmdlLWNoYXQxIQ==");
    let response = tokio::time::timeout(Duration::from_secs(20), request.send()).await??;
    anyhow::ensure!(response.status() == StatusCode::SWITCHING_PROTOCOLS, "HTTP {}", response.status());
    let mut stream = response.upgrade().await?;
    events.send(Event::Up)?;
    while next_frame(&mut stream).await? {
        events.send(Event::Changed)?;
    }
    Ok(())
}

/// Waits for the next data frame; `false` once the server closes.
/// shortcut: no client pings, so an idle proxy may drop the socket and we reconnect.
async fn next_frame(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<bool> {
    loop {
        let mut head = [0; 2];
        stream.read_exact(&mut head).await?;
        let length = match head[1] & 0x7f {
            126 => u64::from(stream.read_u16().await?),
            127 => stream.read_u64().await?,
            short => u64::from(short),
        };
        tokio::io::copy(&mut (&mut *stream).take(length), &mut tokio::io::sink()).await?;
        match head[0] & 0x0f {
            1 | 2 => return Ok(true),
            8 => return Ok(false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_frames_signal_pings_are_skipped_and_close_ends() {
        let mut bytes = vec![0x89, 0, 0x81, 126, 0, 200];
        bytes.extend([b'x'; 200]);
        bytes.extend([0x88, 0]);
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        runtime.block_on(async {
            let mut stream = bytes.as_slice();
            assert!(next_frame(&mut stream).await.unwrap());
            assert!(!next_frame(&mut stream).await.unwrap());
            assert!(next_frame(&mut stream).await.is_err());
        });
    }
}
