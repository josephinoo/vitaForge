use sha1::{Digest, Sha1};
use std::sync::OnceLock;

const CLIENT_ID_PATH: &str = "ux0:data/vitaforge/client_id";

/// Frictionless per-device identifier sent as `X-Client-ID` so the API can track
/// likes/ratings/installs without account registration. Generated once and cached
/// on disk; not a cryptographic secret, just needs to be stable and unique-ish.
/// Default display name shown as the comment author, derived from the persisted
/// client id so it stays stable across launches without asking the user to type one.
pub fn display_name() -> String {
    let id = get();
    format!("VitaGuest-{}", &id[..id.len().min(4)])
}

pub fn get() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| std::fs::read_to_string(CLIENT_ID_PATH)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(generate_and_persist))
}

fn generate_and_persist() -> String {
    let id = generate();
    if let Some(parent) = std::path::Path::new(CLIENT_ID_PATH).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(CLIENT_ID_PATH, &id);
    id
}

fn generate() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let stack_marker = &nanos as *const _ as usize;
    let mut hasher = Sha1::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(stack_marker.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
