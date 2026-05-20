use std::path::PathBuf;

use anyhow::{Context, bail};
use tokio::process::Command;

pub fn split_repo(input: &str) -> anyhow::Result<(&str, &str)> {
    let trimmed = input.trim().trim_matches('/');
    let Some((owner, repo)) = trimmed.split_once('/') else {
        bail!("repository must be in owner/repo form");
    };
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        bail!("repository must be in owner/repo form");
    }
    Ok((owner, repo))
}

pub async fn current_repo() -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .await
        .context("failed to inspect git remote origin")?;
    if !output.status.success() {
        bail!("could not determine repository; pass --repo owner/repo");
    }

    let remote = String::from_utf8_lossy(&output.stdout);
    parse_remote_url(remote.trim()).context("could not parse git remote origin as GitCode repo")
}

pub async fn resolve_repo(
    explicit: Option<&str>,
    default_repo: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(repo) = explicit {
        split_repo(repo)?;
        return Ok(repo.to_string());
    }
    if let Some(repo) = default_repo {
        split_repo(repo)?;
        return Ok(repo.to_string());
    }
    current_repo().await
}

pub fn clone_url(hostname: &str, repository: &str) -> String {
    format!("https://{hostname}/{repository}.git")
}

pub fn github_clone_url(repository: &str) -> String {
    format!("https://github.com/{repository}.git")
}

pub fn parse_github_repo(input: &str) -> anyhow::Result<String> {
    let trimmed = input.trim().trim_matches('/');
    let without_git = trimmed.trim_end_matches(".git");
    if let Some(rest) = without_git.strip_prefix("git@github.com:") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    if let Some(rest) = without_git.strip_prefix("https://github.com/") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    if let Some(rest) = without_git.strip_prefix("http://github.com/") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    if let Some(rest) = without_git.strip_prefix("ssh://git@github.com/") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    split_repo(trimmed)?;
    Ok(trimmed.to_string())
}

pub async fn run_git_clone(
    hostname: &str,
    repository: &str,
    directory: Option<PathBuf>,
    git_flags: &[String],
) -> anyhow::Result<()> {
    split_repo(repository)?;
    let mut command = Command::new("git");
    command.arg("clone").arg(clone_url(hostname, repository));
    if let Some(directory) = directory {
        command.arg(directory);
    }
    command.args(git_flags);
    let status = command.status().await.context("failed to run git clone")?;
    if !status.success() {
        bail!("git clone failed with {status}");
    }
    Ok(())
}

fn parse_remote_url(remote: &str) -> anyhow::Result<String> {
    let remote = remote.trim_end_matches(".git");
    if let Some(rest) = remote.strip_prefix("git@gitcode.com:") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    if let Some(rest) = remote.strip_prefix("https://gitcode.com/") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    if let Some(rest) = remote.strip_prefix("ssh://git@gitcode.com/") {
        split_repo(rest)?;
        return Ok(rest.to_string());
    }
    bail!("unsupported remote URL");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_owner_repo() {
        assert_eq!(split_repo("owner/repo").unwrap(), ("owner", "repo"));
        assert!(split_repo("owner").is_err());
        assert!(split_repo("owner/repo/more").is_err());
    }

    #[test]
    fn parses_gitcode_remote_urls() {
        assert_eq!(
            parse_remote_url("git@gitcode.com:space/project.git").unwrap(),
            "space/project"
        );
        assert_eq!(
            parse_remote_url("https://gitcode.com/space/project.git").unwrap(),
            "space/project"
        );
    }

    #[test]
    fn parses_github_repository_inputs() {
        assert_eq!(
            parse_github_repo("coolplayagent/relay-gitcode-cli").unwrap(),
            "coolplayagent/relay-gitcode-cli"
        );
        assert_eq!(
            parse_github_repo("git@github.com:coolplayagent/relay-gitcode-cli.git").unwrap(),
            "coolplayagent/relay-gitcode-cli"
        );
        assert_eq!(
            parse_github_repo("https://github.com/coolplayagent/relay-gitcode-cli.git").unwrap(),
            "coolplayagent/relay-gitcode-cli"
        );
    }
}
