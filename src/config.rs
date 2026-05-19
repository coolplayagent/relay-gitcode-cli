use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use url::Url;

pub const DEFAULT_HOSTNAME: &str = "gitcode.com";
pub const DEFAULT_API_BASE: &str = "https://api.gitcode.com/api/v5";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub hostname: String,
    pub api_base: String,
    pub default_repo: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hostname: DEFAULT_HOSTNAME.to_string(),
            api_base: DEFAULT_API_BASE.to_string(),
            default_repo: None,
        }
    }
}

impl Config {
    pub async fn load() -> anyhow::Result<Self> {
        let path = config_path()?;
        if !tokio::fs::try_exists(&path)
            .await
            .with_context(|| format!("failed to inspect {}", path.display()))?
        {
            return Ok(Self::default());
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        self.validate()?;
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, format!("{content}\n"))
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn apply_overrides(
        &mut self,
        hostname: Option<&str>,
        api_base: Option<&str>,
    ) -> anyhow::Result<()> {
        if let Some(hostname) = hostname {
            self.hostname = hostname.to_string();
        }
        if let Some(api_base) = api_base {
            self.api_base = api_base.to_string();
        }
        self.validate()
    }

    pub fn api_base_url(&self) -> anyhow::Result<Url> {
        parse_api_base(&self.api_base)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.hostname.trim().is_empty() {
            bail!("hostname cannot be empty");
        }
        parse_api_base(&self.api_base)?;
        Ok(())
    }
}

fn parse_api_base(value: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(value).with_context(|| format!("invalid api base URL: {value}"))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        bail!("api base URL must use http or https");
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url)
}

fn config_path() -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var("GD_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }
    let dir = if let Ok(path) = std::env::var("GD_CONFIG_DIR") {
        PathBuf::from(path)
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join("gd")
    };
    Ok(dir.join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_api_base_with_trailing_slash() {
        let url = parse_api_base("https://api.gitcode.com/api/v5").unwrap();
        assert_eq!(url.as_str(), "https://api.gitcode.com/api/v5/");
    }

    #[test]
    fn rejects_invalid_api_base_scheme() {
        assert!(parse_api_base("file:///tmp/api").is_err());
    }
}
