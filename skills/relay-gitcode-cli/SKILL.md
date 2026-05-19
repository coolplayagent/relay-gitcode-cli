---
name: relay-gitcode-cli
description: Use gd, the relay-gitcode-cli GitCode command line client, for GitCode API v5 workflows including authentication, repositories, pull requests, issues, search, SSH keys, labels, releases, GitCode Pipeline operations, raw API calls, JSON automation, version checks, and shell completion. Use when an agent should operate GitCode by running local gd commands. Do not use this skill for GitHub-only gh surfaces unless GitCode exposes an equivalent API through gd or gd api.
metadata:
  openclaw:
    skillKey: relay-gitcode-cli
    homepage: https://github.com/coolplayagent/relay-gitcode-cli
---

# Relay GitCode CLI

## Workflow

Use the compiled `gd` binary as the GitCode control surface. Prefer `--json`
for automation and inspect command help before exposing unfamiliar operations:

```bash
gd --help
gd pr create --help
gd api --help
```

Keep behavior GitCode-specific. Do not introduce GitHub-only command surfaces
such as gists, codespaces, projects, GitHub Actions, rulesets, extensions, or
Copilot unless GitCode exposes equivalent API behavior and the task can be done
through `gd api`.

When `gd` commands, flags, examples, or user-facing behavior change in this
repository, update this `SKILL.md` in the same change so agent workflows stay
aligned with the compiled CLI.

`gd` reuses reqwest system proxy behavior for `HTTP_PROXY`/`http_proxy`,
`HTTPS_PROXY`/`https_proxy`, `ALL_PROXY`/`all_proxy`, and
`NO_PROXY`/`no_proxy`. TLS certificate verification is disabled by default for
GitCode API calls.

## Readiness

Check whether `gd` exists, then inspect version and authentication state:

```bash
command -v gd
gd --version
gd version check --json
gd auth status --json
```

For online install or upgrades, use a released artifact. Prefer the Rust
package first and GitHub Releases second:

```bash
cargo install relay-gitcode-cli --force
gd version check --json
```

Do not install this skill by building `gd` from a local repository checkout.
The skill is published independently and should point agents at released CLI
artifacts.

For temporary CI or end-to-end tests, prefer `GITCODE_TOKEN` in the process
environment. For interactive token login, read the token from stdin and store it
in the system keyring:

```bash
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token --json
gd auth status --json
```

Never print, commit, or persist personal access tokens, cookies, or private API
responses in repository files.

## Core Workflows

Use explicit repositories with `--repo owner/repo` when the current directory
does not define a `gd` default repository. Add `--json` for machine-readable
output:

```bash
gd repo view owner/repo --json
gd repo list owner --limit 50 --json
gd repo clone owner/repo
gd repo create name --private --description "demo" --json
gd repo fork owner/repo --json
```

Manage issues and pull requests with the GitCode-backed command surface:

```bash
gd issue list --repo owner/repo --state open --limit 30 --json
gd issue view 1 --repo owner/repo --json
gd issue create --repo owner/repo --title "bug" --body "details" --json
gd issue comment 1 --repo owner/repo --body "thanks" --json

gd pr list --repo owner/repo --state open --base main --json
gd pr view 1 --repo owner/repo --json
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature --json
```

Use `gd api` for GitCode API v5 endpoints that do not have first-class
subcommands:

```bash
gd api /user --json
gd api /repos/owner/repo -X PATCH -F has_issues=true --json
gd api /repos/owner/repo/issues --paginate --json
```

## Pipelines

Pipeline commands call GitCode Actions APIs. They reuse the same GitCode
personal access token as other `gd` commands through `GITCODE_TOKEN` or
`gd auth login --with-token`, sending it as `Authorization: Bearer <token>`.
Do not configure or ask for AK/SK credentials for pipeline workflows.

```bash
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml --json
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main --json
gd pipeline runs --repo owner/repo --workflow-name ci --status success --json
gd pipeline view --repo owner/repo workflow-run-id --json
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline stop --repo owner/repo workflow-run-id --json
gd pipeline retry --repo owner/repo workflow-run-id --json
```

## Troubleshooting

If a command fails, read the JSON response or the `gd:` error line before
guessing hidden state. Check authentication, repository spelling, hostname, and
API base first:

```bash
gd auth status --json
gd repo view owner/repo --json
gd api /user --json
gd --api-base https://api.gitcode.com/api/v5 api /user --json
```

For more command recipes and API automation patterns, read
`references/cli-workflows.md`.
