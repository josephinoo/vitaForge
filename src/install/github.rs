use serde::Deserialize;
const USER_AGENT: &str = "vitaforge";
const DEFAULT_GITHUB_API_URL: &str = "https://api.github.com";
fn github_api_url() -> &'static str {
    option_env!("GITHUB_API_URL").unwrap_or(DEFAULT_GITHUB_API_URL)
}
#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
pub struct LatestRelease {
    pub vpk_url: String,
    pub tag: String,
}
fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let rest = url.split("github.com/").nth(1)?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_owned(), repo.to_owned()))
}
pub async fn latest_release(repo_url: &str) -> Option<LatestRelease> {
    let (owner, repo) = parse_owner_repo(repo_url)?;
    let api = format!("{}/repos/{owner}/{repo}/releases/latest", github_api_url());
    let response = crate::net::client()
        .get(&api)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            eprintln!("github release not found (404) for {owner}/{repo}: repo has no releases yet or is private");
        } else {
            eprintln!("github api returned status {} for {api}", response.status());
        }
        return None;
    }
    let release: Release = response.json().await.ok()?;
    if release.tag_name.trim().is_empty() {
        return None;
    }
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name.to_lowercase().ends_with(".vpk"))?;
    Some(LatestRelease { vpk_url: asset.browser_download_url, tag: release.tag_name })
}
