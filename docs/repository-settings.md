# Repository Settings

This repository ships CI for both GitHub Actions and GitCode Pipeline.

## GitHub

- Enable branch protection on `main`.
- Require the `PR Checks` jobs: `format`, `clippy`, `test`, and `build`.
- Require `Qodana` if `QODANA_TOKEN` is configured.
- Configure release secrets:
  - `CARGO_REGISTRY_TOKEN` for crates.io publishing.
  - `CLAWHUB_TOKEN` for optional ClawHub skill publishing.
  - `QODANA_TOKEN` for JetBrains Qodana Cloud reporting.

## GitCode

- Keep workflow files under `.gitcode/workflows`.
- Enable pull-request and push pipelines for `main`.
- Require the `pr-checks` pipeline before merging.
- Store any private release or deployment credentials in GitCode project secrets, not in workflow files.

## Local Policy

- Do not commit GitCode personal access tokens.
- For local end-to-end testing, prefer `GITCODE_TOKEN` in the process environment.
- `gd auth login --with-token` stores credentials in the system keyring.
