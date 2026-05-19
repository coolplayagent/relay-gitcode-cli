[English](README.md) | [中文](README.zh-CN.md)

# relay-gitcode-cli

`relay-gitcode-cli` provides `gd`, a Rust command line client for GitCode. It
uses gh-style command names where GitCode has equivalent API behavior and keeps
GitCode-specific behavior explicit.

GitCode API documentation: https://docs.gitcode.com/docs/apis/

## Quick Start

```bash
cargo build
target/debug/gd --version
printf '%s' "$GITCODE_TOKEN" | target/debug/gd auth login --with-token
target/debug/gd auth status
target/debug/gd repo view owner/repo --json
```

`GITCODE_TOKEN` is also accepted directly for CI and temporary end-to-end tests.
When no environment token is present, `gd auth login --with-token` stores the
token in the system keyring.

## Commands

```bash
gd auth login --with-token
gd auth status
gd auth token
gd auth logout

gd api /user
gd repo view owner/repo
gd repo list owner
gd repo clone owner/repo
gd repo create name --private --description "demo"
gd repo fork owner/repo

gd issue list --repo owner/repo
gd issue view 1 --repo owner/repo
gd issue create --repo owner/repo --title "bug" --body "details"
gd issue comment 1 --repo owner/repo --body "thanks"

gd pr list --repo owner/repo
gd pr view 1 --repo owner/repo
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature

gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --workflow-name ci
gd pipeline log --repo owner/repo workflow-run-id job-id

gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd completion bash
```

GitHub-only gh commands such as codespaces, gists, GitHub Actions workflow
management, projects, rulesets, extensions, and Copilot are intentionally not
part of `gd` unless GitCode exposes equivalent API behavior.

## Documentation

- [Documentation Bookshelf](docs/README.md)
- [English Documentation](docs/en/README.md)
- [Chinese Documentation](docs/zh/README.md)

## Development

```bash
./build.sh --debug
./check.sh
```

The local quality gates are:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --all-targets --all-features
```

## CI

- GitHub Actions: `.github/workflows/pr-checks.yml`,
  `.github/workflows/code_quality.yml`, and `.github/workflows/release.yml`.
- GitCode Pipeline: `.gitcode/workflows/pr-checks.yml`.

Repository protection and secret setup are documented in
`docs/repository-settings.md`.
