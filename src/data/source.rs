use super::AppEntry;
use super::{api, psvt};
pub async fn fetch_live() -> anyhow::Result<Vec<AppEntry>> {
    let (catalog, themes) = tokio::join!(api::fetch_catalog(), psvt::fetch());
    let mut entries = catalog?;
    entries.extend(themes);
    Ok(entries)
}
