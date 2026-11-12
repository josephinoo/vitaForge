use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[cfg(not(test))]
const AUTH_PATH: &str = "ux0:data/vitaforge/auth.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthSession {
    #[serde(default)]
    pub user_id: i64,
    pub username: String,
    pub token: String,
}

static CURRENT_AUTH: RwLock<Option<AuthSession>> = RwLock::new(None);
static AUTH_INITIALIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn auth_path() -> std::path::PathBuf {
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!("vitaforge-auth-test-{}.json", std::process::id()))
    }
    #[cfg(not(test))]
    {
        std::path::PathBuf::from(AUTH_PATH)
    }
}

fn ensure_loaded() {
    if !AUTH_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let path = auth_path();
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(session) = serde_json::from_slice::<AuthSession>(&bytes) {
                if let Ok(mut lock) = CURRENT_AUTH.write() {
                    *lock = Some(session);
                }
            }
        }
    }
}

pub fn current() -> Option<AuthSession> {
    ensure_loaded();
    CURRENT_AUTH.read().ok().and_then(|guard| guard.clone())
}

pub fn token() -> Option<String> {
    current().map(|s| s.token)
}

pub fn username() -> Option<String> {
    current().map(|s| s.username)
}

pub fn is_logged_in() -> bool {
    current().is_some()
}

pub fn save(session: AuthSession) {
    let path = auth_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec_pretty(&session) {
        let _ = std::fs::write(&path, json);
    }
    if let Ok(mut lock) = CURRENT_AUTH.write() {
        *lock = Some(session);
    }
    AUTH_INITIALIZED.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn logout() {
    let path = auth_path();
    let _ = std::fs::remove_file(path);
    if let Ok(mut lock) = CURRENT_AUTH.write() {
        *lock = None;
    }
    AUTH_INITIALIZED.store(true, std::sync::atomic::Ordering::SeqCst);
}
