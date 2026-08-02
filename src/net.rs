
use std::sync::OnceLock;

pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("vitaforge")
                .build()
                .expect("failed to build the shared http client")
        })
        .clone()
}
