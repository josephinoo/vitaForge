use std::time::Duration;

#[test]
fn runtimes_continue_disk_work_after_a_connection_closes() {
    // Run with mio_vita_waker + mio_unsupported_force_poll_poll to exercise
    // the Vita backend on the host, including both runtimes used by the app.
    for multi_thread in [false, true] {
        let (finished, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut builder = if multi_thread {
                let mut builder = tokio::runtime::Builder::new_multi_thread();
                builder.worker_threads(2);
                builder
            } else {
                tokio::runtime::Builder::new_current_thread()
            };
            let runtime = builder.enable_all().build().unwrap();
            runtime.block_on(async {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let socket = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
                let (peer, _) = listener.accept().unwrap();

                // Keep a socket registered while other threads repeatedly wake the reactor.
                let reader = tokio::spawn(async move {
                    loop {
                        socket.readable().await.unwrap();
                        match socket.try_read(&mut [0u8; 1]) {
                            Ok(0) => break,
                            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue,
                            other => panic!("unexpected socket read: {other:?}"),
                        }
                    }
                });
                for n in 0..32 {
                    let actual = tokio::task::spawn_blocking(move || {
                        std::thread::sleep(Duration::from_millis(2));
                        n
                    }).await.unwrap();
                    assert_eq!(actual, n);
                }
                drop(peer);
                drop(listener);
                reader.await.unwrap();

                // No socket remains: completion of disk work must still wake the installer.
                for n in 0..32 {
                    let actual = tokio::task::spawn_blocking(move || {
                        std::thread::sleep(Duration::from_millis(2));
                        n
                    }).await.unwrap();
                    assert_eq!(actual, n);
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
            });
            drop(runtime);
            finished.send(()).unwrap();
        });
        result.recv_timeout(Duration::from_secs(10))
            .expect("runtime stalled during socket close, disk completion, timer or shutdown");
    }
}
