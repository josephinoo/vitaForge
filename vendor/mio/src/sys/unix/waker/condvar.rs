use std::io;
use std::os::fd::RawFd;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Vita's pipe2 is a loopback TCP connection. Keep runtime wakeups independent
/// of that connection, including when only disk work or timers are pending.
#[derive(Debug)]
pub(crate) struct Waker {
    notified: Mutex<bool>,
    ready: Condvar,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Instant;

    #[test]
    fn idle_wait_sleeps_until_timeout_and_preserves_early_wakes() {
        let waker = Waker::new_unregistered().unwrap();
        let start = Instant::now();
        waker.wait(Some(Duration::from_millis(30)));
        assert!(start.elapsed() >= Duration::from_millis(30));
        assert!(!waker.woken());
        waker.wake().unwrap();
        waker.wait(None);
        assert!(waker.woken());
        waker.ack_and_reset();
        assert!(!waker.woken());
    }

    #[test]
    fn cross_thread_wakes_survive_repeated_wait_and_reset() {
        let waker = Arc::new(Waker::new_unregistered().unwrap());
        let worker = waker.clone();
        let (ack, received) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            for _ in 0..100 {
                worker.wait(Some(Duration::from_secs(2)));
                assert!(worker.woken(), "lost wakeup");
                worker.ack_and_reset();
                ack.send(()).unwrap();
            }
        });
        for _ in 0..100 {
            waker.wake().unwrap();
            received.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        thread.join().unwrap();
    }
}

impl Waker {
    pub(crate) fn new_unregistered() -> io::Result<Self> {
        Ok(Self {
            notified: Mutex::new(false),
            ready: Condvar::new(),
        })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        *self.notified.lock().unwrap() = true;
        self.ready.notify_one();
        Ok(())
    }

    pub(crate) fn fd(&self) -> Option<RawFd> {
        None
    }

    pub(crate) fn woken(&self) -> bool {
        *self.notified.lock().unwrap()
    }

    pub(crate) fn ack_and_reset(&self) {
        *self.notified.lock().unwrap() = false;
    }

    pub(crate) fn wait(&self, timeout: Option<Duration>) {
        let notified = self.notified.lock().unwrap();
        match timeout {
            Some(timeout) => {
                drop(self.ready.wait_timeout_while(notified, timeout, |ready| !*ready).unwrap());
            }
            None => {
                drop(self.ready.wait_while(notified, |ready| !*ready).unwrap());
            }
        }
    }
}
