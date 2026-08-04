use std::sync::OnceLock;
use std::time::Duration;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("VitaForge")
                .redirect(reqwest::redirect::Policy::limited(10))
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                .pool_max_idle_per_host(8)
                .tcp_keepalive(Duration::from_secs(60))
                .build()
                .expect("failed to build the shared http client")
        })
        .clone()
}
