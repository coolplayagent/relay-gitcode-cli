# Relay GitCode CLI Workflows

## Contents

- [Installation and Authentication](#installation-and-authentication)
- [Safe Automation Defaults](#safe-automation-defaults)
- [Repository Workflows](#repository-workflows)
- [Issue and Pull Request Workflows](#issue-and-pull-request-workflows)
- [Search, SSH Keys, Labels, and Releases](#search-ssh-keys-labels-and-releases)
- [Raw GitCode API Calls](#raw-gitcode-api-calls)
- [CodeArts Pipeline Workflows](#codearts-pipeline-workflows)
- [Out of Scope](#out-of-scope)

## Installation and Authentication

Build locally from the repository when a released binary is not available:

```bash
cargo build
target/debug/gd --version
```

Use `GITCODE_TOKEN` for temporary automation and CI:

```bash
GITCODE_TOKEN="$GITCODE_TOKEN" gd auth status --json
```

Use keyring-backed login for interactive local sessions:

```bash
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token --json
gd auth status --json
```

Do not write tokens to shell history, repository files, committed fixtures, or
skill outputs. Do not include private API responses in docs or examples.

## Safe Automation Defaults

- Prefer `--json` for any output an agent will parse.
- Prefer explicit `--repo owner/repo` rather than relying on local default
  repository state.
- Use `--hostname` for GitCode host overrides and `--api-base` for API base
  overrides.
- Keep page sizes bounded with `--limit`.
- Read command help before using a subcommand in a new automation path.
- Use `gd api` only for GitCode API v5 endpoints or another user-approved
  GitCode-compatible API base.

## Repository Workflows

Inspect repositories:

```bash
gd repo view owner/repo --json
gd repo list owner --limit 50 --json
```

Clone with optional Git flags after `--`:

```bash
gd repo clone owner/repo
gd repo clone owner/repo local-dir -- --depth 1
```

Create or fork repositories:

```bash
gd repo create demo --private --description "demo repository" --json
gd repo fork owner/repo --json
```

## Issue and Pull Request Workflows

List and inspect issues:

```bash
gd issue list --repo owner/repo --state open --limit 30 --json
gd issue view 1 --repo owner/repo --json
```

Create and comment on issues:

```bash
gd issue create --repo owner/repo --title "bug" --body "details" --label bug --json
gd issue comment 1 --repo owner/repo --body "thanks" --json
```

List and inspect pull requests:

```bash
gd pr list --repo owner/repo --state open --base main --limit 30 --json
gd pr view 1 --repo owner/repo --json
```

Create pull requests:

```bash
gd pr create \
  --repo owner/repo \
  --title "change" \
  --body "details" \
  --base main \
  --head feature \
  --assignee user \
  --label enhancement \
  --json
```

`gd mr` is a visible alias for `gd pr`. Prefer `gd pr` in reusable
documentation unless the user specifically requests merge-request naming.

## Search, SSH Keys, Labels, and Releases

Search GitCode resources:

```bash
gd search repos "rust cli" --limit 20 --json
gd search issues "repo:owner/repo bug" --limit 20 --json
gd search users "alice" --limit 20 --json
```

Manage SSH keys:

```bash
gd ssh-key list --json
gd ssh-key add ~/.ssh/id_ed25519.pub --title laptop --json
gd ssh-key delete key-id --json
```

Manage repository labels:

```bash
gd label list --repo owner/repo --json
gd label create bug --repo owner/repo --color ff0000 --description "Bug reports" --json
gd label edit bug --repo owner/repo --new-name defect --json
gd label delete defect --repo owner/repo --json
```

Inspect and create releases:

```bash
gd release list --repo owner/repo --json
gd release view v0.1.0 --repo owner/repo --json
gd release create v0.1.0 --repo owner/repo --title "v0.1.0" --notes "Release notes" --json
```

## Raw GitCode API Calls

Use `gd api` when the GitCode API supports an operation that does not yet have a
first-class `gd` command:

```bash
gd api /user --json
gd api /repos/owner/repo -X PATCH -F has_issues=true --json
gd api /repos/owner/repo/issues -X POST -f title="bug" -f body="details" --json
gd api /repos/owner/repo/issues --paginate --json
```

Field modes:

- `-f/--raw-field key=value` sends string values.
- `-F/--field key=value` parses `true`, `false`, `null`, integers, and floats.
- `--input file.json` sends a request body from a file; `--input -` reads stdin.
- `--include` prints HTTP status and headers before the body.
- `--silent` suppresses response printing but still fails on non-success status.

## CodeArts Pipeline Workflows

Configure the regional API base and tenant domain:

```bash
export GITCODE_PIPELINE_API_BASE="https://devcloud.ap-southeast-3.myhuaweicloud.com"
export GITCODE_PIPELINE_DOMAIN_ID="domain-id"
```

Prefer AK/SK signing when available:

```bash
export HUAWEICLOUD_SDK_AK="..."
export HUAWEICLOUD_SDK_SK="..."
```

Register or update a workflow file:

```bash
gd pipeline register \
  --repo owner/repo \
  --type create \
  --new-file-path .gitcode/workflows/ci.yml \
  --file workflow.yml \
  --json
```

Run and inspect pipelines:

```bash
gd pipeline run --repo owner/repo --file-path .gitcode/workflows/ci.yml --branch main --json
gd pipeline runs --repo owner/repo --pipeline-name ci --status success --limit 20 --json
gd pipeline view pipeline-id pipeline-run-id --json
gd pipeline log pipeline-id pipeline-run-id job-run-id
```

Control a run:

```bash
gd pipeline stop pipeline-id pipeline-run-id --json
gd pipeline retry pipeline-id pipeline-run-id --json
```

`gd pipeline log` prints raw log text by default. Add `--json` when callers need
the full response envelope.

## Out of Scope

Do not use this skill for GitHub-only gh functionality unless GitCode has an
equivalent API and the task can be completed through `gd` or `gd api`. Do not
manage GitHub Actions workflows, GitHub projects, gists, codespaces, GitHub
rulesets, extensions, or Copilot through this skill.
