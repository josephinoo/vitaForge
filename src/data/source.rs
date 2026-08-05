use super::AppEntry;
use super::api;

/// The catalog itself is never persisted to disk — only artwork is (see
/// `app::icons`). Every launch re-fetches live rather than trusting a stale
/// snapshot, which sidesteps an entire class of bugs where an on-disk cache
/// silently outlives whatever shape or categorization scheme it was written
/// under. The cost is one network round-trip per launch, which the catalog
/// endpoint already has to serve anyway.
pub async fn fetch_live() -> anyhow::Result<Vec<AppEntry>> {
    api::fetch_catalog().await
}
