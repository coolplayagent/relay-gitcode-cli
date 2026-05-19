use std::{
    cmp::Ordering,
    fmt,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use reqwest::{StatusCode, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const DEFAULT_GITHUB_REPO: &str = "coolplayagent/relay-gitcode-cli";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateSource {
    Github,
    CratesIo,
}

impl UpdateSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::CratesIo => "crates.io",
        }
    }

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" | "github-releases" => Ok(Self::Github),
            "crates" | "crates.io" | "crates-io" => Ok(Self::CratesIo),
            other => {
                bail!("invalid GD_UPDATE_SOURCES value '{other}', expected github or crates.io")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateConfig {
    enabled: bool,
    sources: Vec<UpdateSource>,
    github_repo: String,
}

impl UpdateConfig {
    pub fn from_environment() -> anyhow::Result<Self> {
        let enabled = match std::env::var("GD_UPDATE_CHECK_ENABLED") {
            Ok(value) => parse_bool(&value).with_context(|| {
                format!("invalid GD_UPDATE_CHECK_ENABLED value '{value}', expected true or false")
            })?,
            Err(_) => true,
        };

        Ok(Self {
            enabled,
            sources: parse_sources(std::env::var("GD_UPDATE_SOURCES").ok().as_deref())?,
            github_repo: parse_github_repo(
                std::env::var("GD_UPDATE_GITHUB_REPO")
                    .ok()
                    .as_deref()
                    .unwrap_or(DEFAULT_GITHUB_REPO),
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCheckResponse {
    pub project_name: String,
    pub binary_name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub source: Option<String>,
    pub release_url: Option<String>,
    pub checked_at_unix_ms: u64,
    pub install_hint: String,
    pub diagnostics: Vec<VersionCheckDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCheckDiagnostic {
    pub source: Option<String>,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseCandidate {
    source: UpdateSource,
    version: ComparableVersion,
    release_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComparableVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: bool,
}

impl Ord for ComparableVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.major,
            self.minor,
            self.patch,
            release_precedence(self.prerelease),
        )
            .cmp(&(
                other.major,
                other.minor,
                other.patch,
                release_precedence(other.prerelease),
            ))
    }
}

impl PartialOrd for ComparableVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for ComparableVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

const fn release_precedence(prerelease: bool) -> u8 {
    if prerelease { 0 } else { 1 }
}

pub async fn check_for_updates(config: &UpdateConfig) -> VersionCheckResponse {
    let checked_at_unix_ms = current_time_millis();
    if !config.enabled {
        return response_from_candidates(
            current_version(),
            Vec::new(),
            vec![diagnostic(
                None,
                "update_check_disabled",
                "update checks are disabled by GD_UPDATE_CHECK_ENABLED",
                false,
            )],
            checked_at_unix_ms,
        );
    }

    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("gd/{}", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return response_from_candidates(
                current_version(),
                Vec::new(),
                vec![diagnostic(
                    None,
                    "client_build_failed",
                    error.to_string(),
                    false,
                )],
                checked_at_unix_ms,
            );
        }
    };

    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    for source in &config.sources {
        match fetch_source(&client, config, *source).await {
            Ok(candidate) => candidates.push(candidate),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    response_from_candidates(
        current_version(),
        candidates,
        diagnostics,
        checked_at_unix_ms,
    )
}

pub fn render_version_check_text(response: &VersionCheckResponse) -> String {
    let mut output = match (
        response.update_available,
        response.latest_version.as_deref(),
        response.source.as_deref(),
    ) {
        (true, Some(latest), Some(source)) => format!(
            "{} update available: current={} latest={} source={}\n",
            response.binary_name, response.current_version, latest, source
        ),
        (true, Some(latest), None) => format!(
            "{} update available: current={} latest={}\n",
            response.binary_name, response.current_version, latest
        ),
        (false, Some(latest), Some(source)) => format!(
            "{} is current: current={} latest={} source={}\n",
            response.binary_name, response.current_version, latest, source
        ),
        _ => format!(
            "{} latest version unavailable: current={} diagnostics={}\n",
            response.binary_name,
            response.current_version,
            response.diagnostics.len()
        ),
    };

    if let Some(release_url) = &response.release_url {
        output.push_str("Release: ");
        output.push_str(release_url);
        output.push('\n');
    }
    output.push_str("Install: ");
    output.push_str(&response.install_hint);
    output.push('\n');
    output
}

async fn fetch_source(
    client: &reqwest::Client,
    config: &UpdateConfig,
    source: UpdateSource,
) -> Result<ReleaseCandidate, VersionCheckDiagnostic> {
    match source {
        UpdateSource::Github => fetch_github_release(client, &config.github_repo).await,
        UpdateSource::CratesIo => fetch_crates_release(client).await,
    }
}

async fn fetch_github_release(
    client: &reqwest::Client,
    repo: &str,
) -> Result<ReleaseCandidate, VersionCheckDiagnostic> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let response = client
        .get(&url)
        .header(header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| transport_diagnostic(UpdateSource::Github, error))?;
    let status = response.status();
    if !status.is_success() {
        return Err(status_diagnostic(UpdateSource::Github, status));
    }

    let payload = read_json_response::<GithubLatestRelease>(response, UpdateSource::Github).await?;
    github_candidate(payload)
}

async fn fetch_crates_release(
    client: &reqwest::Client,
) -> Result<ReleaseCandidate, VersionCheckDiagnostic> {
    let url = format!("https://crates.io/api/v1/crates/{}", env!("CARGO_PKG_NAME"));
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| transport_diagnostic(UpdateSource::CratesIo, error))?;
    let status = response.status();
    if !status.is_success() {
        return Err(status_diagnostic(UpdateSource::CratesIo, status));
    }

    let payload =
        read_json_response::<CratesPackageResponse>(response, UpdateSource::CratesIo).await?;
    crates_candidate(payload)
}

async fn read_json_response<T>(
    response: reqwest::Response,
    source: UpdateSource,
) -> Result<T, VersionCheckDiagnostic>
where
    T: DeserializeOwned,
{
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(response_body_too_large_diagnostic(source));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| transport_diagnostic(source, error))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(response_body_too_large_diagnostic(source));
    }

    serde_json::from_slice(&bytes).map_err(|error| {
        diagnostic(
            Some(source),
            "invalid_response_json",
            error.to_string(),
            false,
        )
    })
}

#[derive(Debug, Deserialize)]
struct GithubLatestRelease {
    tag_name: String,
    html_url: String,
    prerelease: bool,
}

fn github_candidate(
    release: GithubLatestRelease,
) -> Result<ReleaseCandidate, VersionCheckDiagnostic> {
    if release.prerelease {
        return Err(diagnostic(
            Some(UpdateSource::Github),
            "prerelease_ignored",
            format!("GitHub release '{}' is a prerelease", release.tag_name),
            false,
        ));
    }
    let version = stable_version(&release.tag_name).map_err(|message| {
        diagnostic(
            Some(UpdateSource::Github),
            "invalid_version",
            message,
            false,
        )
    })?;

    Ok(ReleaseCandidate {
        source: UpdateSource::Github,
        version,
        release_url: release.html_url,
    })
}

#[derive(Debug, Deserialize)]
struct CratesPackageResponse {
    #[serde(rename = "crate")]
    package: CratesPackage,
}

#[derive(Debug, Deserialize)]
struct CratesPackage {
    max_stable_version: Option<String>,
}

fn crates_candidate(
    response: CratesPackageResponse,
) -> Result<ReleaseCandidate, VersionCheckDiagnostic> {
    let Some(max_stable_version) = response.package.max_stable_version else {
        return Err(diagnostic(
            Some(UpdateSource::CratesIo),
            "stable_version_unavailable",
            "crates.io response did not include a stable release version",
            false,
        ));
    };
    let version = stable_version(&max_stable_version).map_err(|message| {
        diagnostic(
            Some(UpdateSource::CratesIo),
            "invalid_version",
            message,
            false,
        )
    })?;

    Ok(ReleaseCandidate {
        source: UpdateSource::CratesIo,
        version,
        release_url: format!("https://crates.io/crates/{}", env!("CARGO_PKG_NAME")),
    })
}

fn response_from_candidates(
    current_version: ComparableVersion,
    candidates: Vec<ReleaseCandidate>,
    diagnostics: Vec<VersionCheckDiagnostic>,
    checked_at_unix_ms: u64,
) -> VersionCheckResponse {
    let latest = candidates
        .into_iter()
        .max_by(|left, right| left.version.cmp(&right.version));
    let update_available = latest
        .as_ref()
        .is_some_and(|candidate| candidate.version > current_version);

    VersionCheckResponse {
        project_name: env!("CARGO_PKG_NAME").to_string(),
        binary_name: "gd".to_string(),
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: latest
            .as_ref()
            .map(|candidate| candidate.version.to_string()),
        update_available,
        source: latest
            .as_ref()
            .map(|candidate| candidate.source.as_str().to_string()),
        release_url: latest.map(|candidate| candidate.release_url),
        checked_at_unix_ms,
        install_hint: format!(
            "cargo install {} --force, or download a platform archive from GitHub Releases",
            env!("CARGO_PKG_NAME")
        ),
        diagnostics,
    }
}

fn stable_version(value: &str) -> Result<ComparableVersion, String> {
    let version = comparable_version(value)?;
    if version.prerelease {
        return Err(format!("release version '{value}' is a prerelease"));
    }
    Ok(version)
}

fn comparable_version(value: &str) -> Result<ComparableVersion, String> {
    let trimmed = value.trim().trim_start_matches('v');
    let without_build = trimmed.split('+').next().unwrap_or(trimmed);
    let prerelease = without_build.contains('-');
    let core = without_build.split('-').next().unwrap_or(without_build);
    let mut parts = core.split('.');
    let Some(major) = parts.next() else {
        return Err(format!("release version '{value}' is not semver"));
    };
    let Some(minor) = parts.next() else {
        return Err(format!("release version '{value}' is not semver"));
    };
    let Some(patch) = parts.next() else {
        return Err(format!("release version '{value}' is not semver"));
    };
    if parts.next().is_some() {
        return Err(format!("release version '{value}' is not semver"));
    }

    Ok(ComparableVersion {
        major: parse_version_component(value, major)?,
        minor: parse_version_component(value, minor)?,
        patch: parse_version_component(value, patch)?,
        prerelease,
    })
}

fn parse_version_component(value: &str, component: &str) -> Result<u64, String> {
    if component.is_empty()
        || !component
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(format!("release version '{value}' is not semver"));
    }

    component
        .parse::<u64>()
        .map_err(|_| format!("release version '{value}' is not semver"))
}

fn current_version() -> ComparableVersion {
    comparable_version(env!("CARGO_PKG_VERSION")).expect("Cargo package version must be semver")
}

fn parse_sources(value: Option<&str>) -> anyhow::Result<Vec<UpdateSource>> {
    let value = value.unwrap_or("github,crates.io");
    let sources = value
        .split(',')
        .map(str::trim)
        .filter(|source| !source.is_empty())
        .map(UpdateSource::parse)
        .collect::<anyhow::Result<Vec<_>>>()?;
    if sources.is_empty() {
        bail!("GD_UPDATE_SOURCES must include github or crates.io");
    }
    Ok(sources)
}

fn parse_github_repo(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    let Some((owner, repo)) = trimmed.split_once('/') else {
        bail!("GD_UPDATE_GITHUB_REPO must be owner/name, got '{trimmed}'");
    };
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        bail!("GD_UPDATE_GITHUB_REPO must be owner/name, got '{trimmed}'");
    }
    Ok(trimmed.to_string())
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("expected true or false"),
    }
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn diagnostic(
    source: Option<UpdateSource>,
    code: impl Into<String>,
    message: impl Into<String>,
    retryable: bool,
) -> VersionCheckDiagnostic {
    VersionCheckDiagnostic {
        source: source.map(|source| source.as_str().to_string()),
        code: code.into(),
        message: message.into(),
        retryable,
    }
}

fn transport_diagnostic(source: UpdateSource, error: reqwest::Error) -> VersionCheckDiagnostic {
    diagnostic(Some(source), "transport_failed", error.to_string(), true)
}

fn status_diagnostic(source: UpdateSource, status: StatusCode) -> VersionCheckDiagnostic {
    diagnostic(
        Some(source),
        "http_status",
        format!("release metadata request returned HTTP {}", status.as_u16()),
        status.is_server_error()
            || status == StatusCode::REQUEST_TIMEOUT
            || status == StatusCode::TOO_MANY_REQUESTS,
    )
}

fn response_body_too_large_diagnostic(source: UpdateSource) -> VersionCheckDiagnostic {
    diagnostic(
        Some(source),
        "response_body_too_large",
        format!("release metadata response exceeded {MAX_RESPONSE_BYTES} bytes"),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stable_versions_for_comparison() {
        assert!(stable_version("v1.2.3").unwrap() > stable_version("1.2.2").unwrap());
        assert!(stable_version("1.10.0").unwrap() > stable_version("1.9.9").unwrap());
    }

    #[test]
    fn rejects_prerelease_candidates() {
        let error = stable_version("v1.2.3-rc.1").unwrap_err();
        assert!(error.contains("prerelease"));
    }

    #[test]
    fn current_prerelease_compares_lower_than_stable() {
        assert!(comparable_version("1.2.3").unwrap() > comparable_version("1.2.3-rc.1").unwrap());
    }

    #[test]
    fn picks_highest_candidate_source() {
        let response = response_from_candidates(
            stable_version("1.0.0").unwrap(),
            vec![
                ReleaseCandidate {
                    source: UpdateSource::Github,
                    version: stable_version("1.1.0").unwrap(),
                    release_url: "https://github.com/example/repo/releases/tag/v1.1.0".to_string(),
                },
                ReleaseCandidate {
                    source: UpdateSource::CratesIo,
                    version: stable_version("1.2.0").unwrap(),
                    release_url: "https://crates.io/crates/example".to_string(),
                },
            ],
            Vec::new(),
            10,
        );

        assert_eq!(response.latest_version.as_deref(), Some("1.2.0"));
        assert_eq!(response.source.as_deref(), Some("crates.io"));
        assert!(response.update_available);
    }

    #[test]
    fn validates_source_list() {
        assert_eq!(
            parse_sources(Some("github,crates.io")).unwrap(),
            vec![UpdateSource::Github, UpdateSource::CratesIo]
        );
        assert!(parse_sources(Some("")).is_err());
        assert!(parse_sources(Some("npm")).is_err());
    }

    #[test]
    fn renders_text_with_install_hint() {
        let response =
            response_from_candidates(stable_version("0.1.0").unwrap(), Vec::new(), Vec::new(), 10);
        let text = render_version_check_text(&response);
        assert!(text.contains("Install: cargo install relay-gitcode-cli --force"));
    }
}
