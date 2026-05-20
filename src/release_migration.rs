use std::collections::BTreeSet;

use anyhow::{Context, bail};
use reqwest::{
    StatusCode,
    header::{ACCEPT, AUTHORIZATION, HeaderMap, LOCATION, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

use crate::client::{ApiResponse, GitcodeClient};

#[derive(Debug, Clone)]
pub struct ReleaseMigrationOptions {
    pub gitcode_repo: String,
    pub github_repo: String,
    pub tag: Option<String>,
    pub all: bool,
    pub skip_existing_assets: bool,
    pub update_release: bool,
    pub dry_run: bool,
    pub github_token: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ReleaseMigrationSummary {
    pub gitcode_repo: String,
    pub github_repo: String,
    pub dry_run: bool,
    pub totals: ReleaseMigrationTotals,
    pub releases: Vec<ReleaseMigrationRecord>,
}

impl ReleaseMigrationSummary {
    pub fn text_summary(&self) -> String {
        format!(
            "releases: created={} updated={} skipped={} assets: uploaded={} skipped={} failed={}",
            self.totals.releases_created,
            self.totals.releases_updated,
            self.totals.releases_skipped,
            self.totals.assets_uploaded,
            self.totals.assets_skipped,
            self.totals.assets_failed,
        )
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ReleaseMigrationTotals {
    pub releases_created: u32,
    pub releases_updated: u32,
    pub releases_skipped: u32,
    pub assets_uploaded: u32,
    pub assets_skipped: u32,
    pub assets_failed: u32,
}

#[derive(Debug, Serialize)]
pub struct ReleaseMigrationRecord {
    pub tag: String,
    pub action: String,
    pub assets: Vec<AssetMigrationRecord>,
}

#[derive(Debug, Serialize)]
pub struct AssetMigrationRecord {
    pub name: String,
    pub action: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    target_commitish: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    content_type: Option<String>,
    size: u64,
}

struct GithubClient {
    http: reqwest::Client,
    token: Option<String>,
}

impl GithubClient {
    fn new(token: Option<String>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .build()
            .context("failed to build GitHub HTTP client")?;
        Ok(Self { http, token })
    }

    async fn release_by_tag(&self, repository: &str, tag: &str) -> anyhow::Result<GithubRelease> {
        let url = format!(
            "https://api.github.com/repos/{repository}/releases/tags/{}",
            encode_path_segment(tag)
        );
        self.get_json(&url).await
    }

    async fn releases(&self, repository: &str) -> anyhow::Result<Vec<GithubRelease>> {
        let mut releases = Vec::new();
        for page in 1.. {
            let url = format!(
                "https://api.github.com/repos/{repository}/releases?per_page=100&page={page}"
            );
            let mut page_items: Vec<GithubRelease> = self.get_json(&url).await?;
            let done = page_items.len() < 100;
            releases.append(&mut page_items);
            if done {
                break;
            }
        }
        Ok(releases)
    }

    async fn download_asset(&self, asset: &GithubAsset) -> anyhow::Result<Vec<u8>> {
        let mut builder = self
            .http
            .get(&asset.browser_download_url)
            .header(USER_AGENT, "gd-release-migration")
            .header(ACCEPT, "application/octet-stream");
        if let Some(token) = &self.token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        let response = builder
            .send()
            .await
            .with_context(|| format!("failed to download GitHub release asset {}", asset.name))?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "GitHub release asset download returned {status}: {}",
                asset.name
            );
        }
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed to read GitHub release asset {}", asset.name))?;
        Ok(bytes.to_vec())
    }

    async fn get_json<T>(&self, url: &str) -> anyhow::Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut builder = self
            .http
            .get(url)
            .header(USER_AGENT, "gd-release-migration")
            .header(ACCEPT, "application/vnd.github+json");
        if let Some(token) = &self.token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        let response = builder
            .send()
            .await
            .with_context(|| format!("GitHub API request failed: {url}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read GitHub API response")?;
        if !status.is_success() {
            bail!("GitHub API returned {status}: {text}");
        }
        serde_json::from_str(&text).context("failed to parse GitHub API response")
    }
}

pub async fn migrate_github_releases(
    gitcode: &GitcodeClient,
    options: ReleaseMigrationOptions,
) -> anyhow::Result<ReleaseMigrationSummary> {
    validate_options(&options)?;
    let github_repo = crate::repo::parse_github_repo(&options.github_repo)?;
    let github = GithubClient::new(options.github_token.clone())?;
    let releases = if options.all {
        github.releases(&github_repo).await?
    } else {
        let tag = options
            .tag
            .as_deref()
            .expect("tag is required when --all is not set");
        vec![github.release_by_tag(&github_repo, tag).await?]
    };

    let mut summary = ReleaseMigrationSummary {
        gitcode_repo: options.gitcode_repo.clone(),
        github_repo,
        dry_run: options.dry_run,
        ..ReleaseMigrationSummary::default()
    };

    for release in releases {
        let record = migrate_release(&github, gitcode, &options, release).await?;
        accumulate(&mut summary.totals, &record);
        summary.releases.push(record);
    }

    Ok(summary)
}

fn validate_options(options: &ReleaseMigrationOptions) -> anyhow::Result<()> {
    crate::repo::split_repo(&options.gitcode_repo)?;
    crate::repo::parse_github_repo(&options.github_repo)?;
    if options.all == options.tag.is_some() {
        bail!("set exactly one of --tag or --all");
    }
    Ok(())
}

async fn migrate_release(
    github: &GithubClient,
    gitcode: &GitcodeClient,
    options: &ReleaseMigrationOptions,
    release: GithubRelease,
) -> anyhow::Result<ReleaseMigrationRecord> {
    if release.draft {
        return Ok(ReleaseMigrationRecord {
            tag: release.tag_name,
            action: "skipped_draft".to_string(),
            assets: Vec::new(),
        });
    }

    let existing =
        gitcode_release_by_tag(gitcode, &options.gitcode_repo, &release.tag_name).await?;
    let release_body = gitcode_release_body(&release);
    let (action, release_value) = match existing {
        Some(existing) if options.dry_run && options.update_release => {
            ("would_update".to_string(), existing)
        }
        Some(existing) if options.dry_run => ("would_skip".to_string(), existing),
        Some(_) if options.update_release => {
            let value = gitcode
                .patch(
                    &format!(
                        "repos/{}/releases/{}",
                        options.gitcode_repo,
                        encode_path_segment(&release.tag_name)
                    ),
                    &release_body,
                )
                .await?;
            ("updated".to_string(), value)
        }
        Some(existing) => ("skipped".to_string(), existing),
        None if options.dry_run => ("would_create".to_string(), Value::Null),
        None => {
            let value = gitcode
                .post(
                    &format!("repos/{}/releases", options.gitcode_repo),
                    &release_body,
                )
                .await?;
            ("created".to_string(), value)
        }
    };

    let existing_assets = asset_names(&release_value);
    let mut assets = Vec::new();
    for asset in &release.assets {
        if existing_assets.contains(&asset.name) {
            if options.skip_existing_assets {
                assets.push(AssetMigrationRecord {
                    name: asset.name.clone(),
                    action: "skipped_existing".to_string(),
                    size: asset.size,
                });
                continue;
            }
            bail!("GitCode release asset already exists: {}", asset.name);
        }

        if options.dry_run {
            assets.push(AssetMigrationRecord {
                name: asset.name.clone(),
                action: "would_upload".to_string(),
                size: asset.size,
            });
            continue;
        }

        let upload_url =
            gitcode_release_upload_url(gitcode, &options.gitcode_repo, &release.tag_name).await?;
        let bytes = github.download_asset(asset).await?;
        gitcode
            .upload_multipart_bytes(
                &upload_url,
                &asset.name,
                asset.content_type.as_deref(),
                bytes,
            )
            .await
            .with_context(|| format!("failed to upload GitCode release asset {}", asset.name))?;
        assets.push(AssetMigrationRecord {
            name: asset.name.clone(),
            action: "uploaded".to_string(),
            size: asset.size,
        });
    }

    Ok(ReleaseMigrationRecord {
        tag: release.tag_name,
        action,
        assets,
    })
}

async fn gitcode_release_by_tag(
    gitcode: &GitcodeClient,
    repository: &str,
    tag: &str,
) -> anyhow::Result<Option<Value>> {
    let response = gitcode
        .get_response(
            &format!(
                "repos/{repository}/releases/tags/{}",
                encode_path_segment(tag)
            ),
            &[],
        )
        .await?;
    if response.status.is_success() {
        return Ok(Some(response.body));
    }
    if is_missing_release(&response) {
        return Ok(None);
    }
    Err(api_response_error(response))
}

async fn gitcode_release_upload_url(
    gitcode: &GitcodeClient,
    repository: &str,
    tag: &str,
) -> anyhow::Result<String> {
    let response = gitcode
        .get_response(
            &format!(
                "repos/{repository}/releases/{}/upload_url",
                encode_path_segment(tag)
            ),
            &[],
        )
        .await?;
    if !response.status.is_success() {
        return Err(api_response_error(response));
    }
    extract_upload_url(&response).context("GitCode upload_url response did not include a URL")
}

fn gitcode_release_body(release: &GithubRelease) -> Value {
    let mut body = Map::new();
    body.insert(
        "tag_name".to_string(),
        Value::String(release.tag_name.clone()),
    );
    body.insert(
        "name".to_string(),
        Value::String(
            release
                .name
                .clone()
                .unwrap_or_else(|| release.tag_name.clone()),
        ),
    );
    body.insert(
        "body".to_string(),
        Value::String(release.body.clone().unwrap_or_default()),
    );
    body.insert(
        "target_commitish".to_string(),
        Value::String(release.target_commitish.clone()),
    );
    body.insert("prerelease".to_string(), Value::Bool(release.prerelease));
    Value::Object(body)
}

fn asset_names(value: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_asset_names(value, &mut names);
    if let Some(data) = value.get("data") {
        collect_asset_names(data, &mut names);
    }
    names
}

fn collect_asset_names(value: &Value, names: &mut BTreeSet<String>) {
    let Some(assets) = value.get("assets").and_then(Value::as_array) else {
        return;
    };
    for asset in assets {
        if let Some(name) = asset.get("name").and_then(Value::as_str) {
            names.insert(name.to_string());
        }
    }
}

fn extract_upload_url(response: &ApiResponse) -> Option<String> {
    extract_upload_url_from_body(&response.body).or_else(|| header_url(&response.headers, LOCATION))
}

fn extract_upload_url_from_body(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return is_url(value).then(|| value.to_string());
    }
    for key in ["upload_url", "url", "href"] {
        if let Some(value) = value.get(key).and_then(Value::as_str)
            && is_url(value)
        {
            return Some(value.to_string());
        }
    }
    if let Some(data) = value.get("data") {
        return extract_upload_url_from_body(data);
    }
    None
}

fn header_url(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| is_url(value))
        .map(str::to_string)
}

fn is_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn is_missing_release(response: &ApiResponse) -> bool {
    matches!(
        response.status,
        StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST
    ) && body_message(&response.body)
        .map(|message| message.to_ascii_lowercase().contains("release"))
        .unwrap_or(true)
}

fn api_response_error(response: ApiResponse) -> anyhow::Error {
    let message = body_message(&response.body)
        .map(str::to_string)
        .unwrap_or_else(|| response.body.to_string());
    anyhow::anyhow!("GitCode API returned {}: {message}", response.status)
}

fn body_message(value: &Value) -> Option<&str> {
    value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| value.get("error").and_then(Value::as_str))
        .or_else(|| value.get("error_message").and_then(Value::as_str))
}

fn accumulate(totals: &mut ReleaseMigrationTotals, record: &ReleaseMigrationRecord) {
    match record.action.as_str() {
        "created" | "would_create" => totals.releases_created += 1,
        "updated" | "would_update" => totals.releases_updated += 1,
        _ => totals.releases_skipped += 1,
    }
    for asset in &record.assets {
        match asset.action.as_str() {
            "uploaded" | "would_upload" => totals.assets_uploaded += 1,
            "skipped_existing" => totals.assets_skipped += 1,
            _ => totals.assets_failed += 1,
        }
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderMap;
    use serde_json::json;

    use super::*;

    #[test]
    fn validates_tag_or_all_is_exclusive() {
        let options = ReleaseMigrationOptions {
            gitcode_repo: "owner/repo".to_string(),
            github_repo: "source/repo".to_string(),
            tag: None,
            all: false,
            skip_existing_assets: true,
            update_release: true,
            dry_run: false,
            github_token: None,
        };
        assert!(validate_options(&options).is_err());
    }

    #[test]
    fn extracts_nested_upload_url() {
        let response = ApiResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: json!({"data": {"upload_url": "https://api.gitcode.com/upload"}}),
        };
        assert_eq!(
            extract_upload_url(&response).as_deref(),
            Some("https://api.gitcode.com/upload")
        );
    }

    #[test]
    fn collects_release_asset_names() {
        let value = json!({
            "assets": [{"name": "a.tar.gz"}],
            "data": {"assets": [{"name": "b.zip"}]}
        });
        let names = asset_names(&value);
        assert!(names.contains("a.tar.gz"));
        assert!(names.contains("b.zip"));
    }

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode_path_segment("v1.0.0"), "v1.0.0");
        assert_eq!(encode_path_segment("release/one"), "release%2Fone");
    }
}
