use std::{path::PathBuf, process::Command};

use anyhow::{Context, bail};

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

pub fn current_repo() -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("failed to inspect git remote origin")?;
    if !output.status.success() {
        bail!("could not determine repository; pass --repo owner/repo");
    }

    let remote = String::from_utf8_lossy(&output.stdout);
    parse_remote_url(remote.trim()).context("could not parse git remote origin as GitCode repo")
}

pub fn resolve_repo(explicit: Option<&str>, default_repo: Option<&str>) -> anyhow::Result<String> {
    if let Some(repo) = explicit {
        split_repo(repo)?;
        return Ok(repo.to_string());
    }
    if let Some(repo) = default_repo {
        split_repo(repo)?;
        return Ok(repo.to_string());
    }
    current_repo()
}

pub fn clone_url(hostname: &str, repository: &str) -> String {
    format!("https://{hostname}/{repository}.git")
}

pub fn run_git_clone(
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
    let status = command.status().context("failed to run git clone")?;
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
}
