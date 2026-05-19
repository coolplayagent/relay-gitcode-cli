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
  crate publishing, CLI skill packaging, archive upload, checksum generation,
  and GitHub Release creation.

GitHub Releases include `relay-gitcode-cli-skill-<tag>.tar.gz`, built from
`skills/relay-gitcode-cli` with the same version as `Cargo.toml`. When
`CLAWHUB_TOKEN` is configured, the release workflow also publishes that
directory to ClawHub with `clawhub publish`. The skill should be installed from
that published release artifact or ClawHub package, not from a local checkout.

## GitCode Pipeline

GitCode workflows live under `.gitcode/workflows`. The Rust pipeline checks the
same core gates as GitHub Actions inside `repo_workspace`.

For CodeCheck workflows, create a GitCode project secret such as
`CODECHECK_ACCESS_TOKEN`, then generate the workflow without committing the
token value. Pull request runs check the source repository and branch, while
push runs check the configured repository URL and current ref:

```bash
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
```

## Repository Settings

See [Repository Settings](../../repository-settings.md) for branch protection,
required checks, and secret setup.
