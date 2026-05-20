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
  GitHub Release creation, and optional GitCode Release sync.

GitHub Releases include `relay-gitcode-cli-skill-<tag>.tar.gz`, built from
`skills/relay-gitcode-cli` with the same version as `Cargo.toml`. The release
workflow replaces the source `metadata.version` placeholder with the validated
Cargo version, then injects the Linux x64 and Windows x64 `gd` binaries into
the skill `assets` directory before packaging it. When `CLAWHUB_TOKEN` is
configured, the release workflow also publishes the packaged skill bundle to
ClawHub with `clawhub skill publish`, so the ClawHub package contains the same
metadata version and bundled assets as the GitHub Release archive. The skill
should be installed from that published release artifact or ClawHub package,
not from a local checkout.

GitHub stores secret names in uppercase, so a `clawhub_token` secret created in
the repository settings is available to the workflow as `CLAWHUB_TOKEN`.
Configure `GITCODE_TOKEN` to let the release workflow run
`gd repo sync-github` and `gd release migrate-github` after GitHub Release
publication. The workflow uses `gd repo sync-github --method git-push` so the
target remains a regular writable GitCode repository instead of a Pull mirror.
The token needs permission to create or update the target GitCode repository,
push branches and tags over HTTPS, create or update Releases, and upload
Release assets in `plm-cac/relay-gitcode-cli`. If `GITCODE_TOKEN` is absent,
GitCode repository and Release sync are skipped.

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
