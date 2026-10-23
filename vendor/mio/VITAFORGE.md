# Vita runtime wakeups

Vendored Mio 1.2.2, upstream commit `50ee78de7497f10b941772a94625b9b6b61facba`.
Original crates.io checksum: `30d65c71f1ce40ab09135ce117d742b9b6b61facba`.
The upstream MIT license is retained in LICENSE.

VitaSDK implements pipe2 with loopback TCP sockets. Mio's pipe waker can
therefore fail even when Tokio is only waiting for disk work; Tokio panics
on that wake error. The Vita backend uses a mutex and condition variable
instead. With no socket interests it sleeps until notification or timeout.
With socket interests, poll waits at most 20 ms because Vita has no mixed
socket/condition-variable wait API. This adds up to 20 ms wake latency
during network I/O and up to 50 polling wakeups per second while sockets
have interests. Other targets keep their existing backend.

Changes are in `src/sys/unix/mod.rs`, `waker/condvar.rs`, and
`selector/poll.rs` (including reset-before-token ordering to preserve
concurrent wakeups). Cargo.toml permits a host-only test configuration.
Recheck these changes on Mio upgrades; remove the patch when upstream
provides a network-independent Vita waker.

Host integration test using the actual patched backend, from the app root:

```sh
RUSTFLAGS='--cfg mio_unsupported_force_poll_poll --cfg mio_vita_waker' cargo +1.98.1 test --target aarch64-apple-darwin --offline
rustc +1.98.1 --edition=2021 --test vendor/mio/src/sys/unix/waker/condvar.rs -o /tmp/vitaforge-native-waker-tests
/tmp/vitaforge-native-waker-tests
```

The integration test covers both Tokio runtime types with a live socket,
peer closure, blocking-worker completion, timers and shutdown. Hardware
testing with Wi-Fi disconnects remains necessary.
