use super::AppEntry;
use super::api;
pub async fn fetch_live() -> anyhow::Result<Vec<AppEntry>> {
    api::fetch_catalog().await
}
