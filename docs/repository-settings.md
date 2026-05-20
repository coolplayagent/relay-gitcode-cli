# Repository Settings

This repository ships CI for both GitHub Actions and GitCode Pipeline.

## GitHub

- Enable branch protection on `main`.
- Require the `PR Checks` jobs: `format`, `clippy`, `test`, and `build`.
- Require `Qodana` if `QODANA_TOKEN` is configured.
- Configure release secrets:
  - `CARGO_REGISTRY_TOKEN` for crates.io publishing.
  - `GITCODE_TOKEN` for optional GitCode repository and Release
    synchronization after GitHub Release publication. The token must be able to
    create the target GitCode repository when missing, create or update
    Releases, and upload Release assets in the target GitCode repository.
  - `CLAWHUB_TOKEN` for optional ClawHub skill publishing.
    GitHub stores secret names in uppercase, so `clawhub_token` is read as
    `CLAWHUB_TOKEN` by the workflow.
  - `QODANA_TOKEN` for JetBrains Qodana Cloud reporting.

## GitCode

- Keep workflow files under `.gitcode/workflows`.
- Enable pull-request and push pipelines for `main`.
- Require the `pr-checks` pipeline before merging.
- Store any private release or deployment credentials in GitCode project secrets, not in workflow files.

## Local Policy

- Do not commit GitCode personal access tokens.
- For local end-to-end testing, prefer `GD_TOKEN` or `GITCODE_TOKEN` in the
  process environment.
- `gd auth login --with-token` stores credentials in the system keyring.
