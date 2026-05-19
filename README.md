[English](README.md) | [中文](README.zh-CN.md)

# relay-gitcode-cli

`relay-gitcode-cli` provides `gd`, a Rust command line client for GitCode. It
uses gh-style command names where GitCode has equivalent API behavior and keeps
GitCode-specific behavior explicit.

GitCode API documentation: https://docs.gitcode.com/docs/apis/

## Quick Start

```bash
cargo install relay-gitcode-cli --force
gd --version
gd version check --json
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token
gd auth status
gd repo view owner/repo --json
```

`GITCODE_TOKEN` is also accepted directly for CI and temporary end-to-end tests.
When no environment token is present, `gd auth login --with-token` stores the
token in the system keyring.

OpenLibing-backed pipeline gate checks use separate credentials. Use
`gd pipeline auth login` for browser OAuth, or set `GD_OPENLIBING_TOKEN` or
`GD_OPENLIBING_COOKIE` for automation.

Each GitHub Release also includes
`relay-gitcode-cli-skill-<tag>.tar.gz`, a text-only ClawHub-compatible skill
that teaches LLM agents to use the local `gd` CLI for GitCode API v5 workflows.
The skill is installed from published releases or ClawHub, not from a local
repository checkout. The release workflow can publish the same
`skills/relay-gitcode-cli` directory to ClawHub when `CLAWHUB_TOKEN` is
configured:

```bash
clawhub publish skills/relay-gitcode-cli \
  --slug relay-gitcode-cli \
  --name "Relay GitCode CLI" \
  --version <version>
```

This skill-over-CLI path is limited to GitCode-backed `gd` commands and raw
GitCode API calls; GitHub-only `gh` command surfaces remain out of scope.

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
gd pr comments 1 --repo owner/repo --limit 50
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3
gd pr reply 1 discussion-id --repo owner/repo --body "fixed"

gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --workflow-name ci
gd pipeline view --repo owner/repo workflow-run-id
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline auth login
gd pipeline auth status
gd pipeline config --project-id openlibing-project-id
gd pipeline setup --project-id openlibing-project-id --repo owner/repo --language Rust --codecheck-rule-set default
gd pipeline prs --project-id openlibing-project-id --repo owner/repo
gd pipeline checks --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-view --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-runs --project-id openlibing-project-id --pipeline-name codecheck

gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd version check
gd completion bash
```

`gd pipeline checks` falls back to the OpenLibing CodeCheck task summary when
the CICD PR check endpoint is not readable. `gd pipeline setup
--codecheck-rule-set` accepts either a rule-set name or a direct rule-set ID.
The OpenLibing help workflow requires the repository to be maintained in
OpenLibing by a project administrator or equivalent project approver first:
record the GitCode repository in Code Repository Management, enable PR
takeover, select the CodeCheck language and rule set, make sure the GitCode
public or robot account can access the repository, and allow webhook
configuration. Browser or headless-browser automation cannot bypass these
server-side permissions.

GitHub-only gh commands such as codespaces, gists, GitHub Actions workflow
management, projects, rulesets, extensions, and Copilot are intentionally not
part of `gd` unless GitCode exposes equivalent API behavior.

## Network

`gd` honors reqwest system proxy environment variables:
`HTTP_PROXY`/`http_proxy`, `HTTPS_PROXY`/`https_proxy`,
`ALL_PROXY`/`all_proxy`, and `NO_PROXY`/`no_proxy`. TLS certificate
verification is disabled by default for GitCode API calls.

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
