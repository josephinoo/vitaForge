use std::sync::OnceLock;
use std::time::Duration;
#[cfg(test)]
mod runtime_tests;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const USER_AGENT: &str = concat!("VitaForge/", env!("CARGO_PKG_VERSION"));
pub fn client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .redirect(reqwest::redirect::Policy::limited(10))
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                .pool_max_idle_per_host(8)
                .tcp_keepalive(Duration::from_secs(60))
                .use_preconfigured_tls(tls_config())
                .build()
                .expect("failed to build the shared http client")
        })
        .clone()
}

/// The Vita's Cortex-A9 has no AES instructions, so software ChaCha20 is
/// several times faster than AES-GCM; offer it first (archive.org honours it).
fn tls_config() -> rustls::ClientConfig {
    use rustls::CipherSuite::*;
    let mut provider = rustls::crypto::ring::default_provider();
    provider.cipher_suites.sort_by_key(|s| {
        !matches!(
            s.suite(),
            TLS13_CHACHA20_POLY1305_SHA256
                | TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                | TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
        )
    });
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}
