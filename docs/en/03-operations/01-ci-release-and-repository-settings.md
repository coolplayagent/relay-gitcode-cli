# Chapter 1: CI, Release, and Repository Settings

## Local Gate

```bash
./check.sh
```

The gate runs formatting, clippy with warnings denied, tests, and a build.

## GitHub Actions

- `PR Checks`: format, clippy, test, build.
- `Qodana`: Rust static analysis.
- `Release`: version validation, package dry run, multi-platform binary builds,
  CLI skill packaging, archive upload, checksum generation, and GitHub Release
  creation.

GitHub Releases include `relay-gitcode-cli-skill-<tag>.tar.gz`, built from
`skills/relay-gitcode-cli` with the same version as `Cargo.toml`. When
`CLAWHUB_TOKEN` is configured, the release workflow also publishes that
directory to ClawHub with `clawhub publish`.

## GitCode Pipeline

GitCode workflows live under `.gitcode/workflows`. The Rust pipeline checks the
same core gates as GitHub Actions inside `repo_workspace`.

## Repository Settings

See [Repository Settings](../../repository-settings.md) for branch protection,
required checks, and secret setup.
