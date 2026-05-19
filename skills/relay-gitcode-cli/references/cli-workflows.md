# Relay GitCode CLI Workflows

## Contents

- [Installation and Authentication](#installation-and-authentication)
- [Safe Automation Defaults](#safe-automation-defaults)
- [Repository Workflows](#repository-workflows)
- [Issue and Pull Request Workflows](#issue-and-pull-request-workflows)
- [Search, SSH Keys, Labels, and Releases](#search-ssh-keys-labels-and-releases)
- [Raw GitCode API Calls](#raw-gitcode-api-calls)
- [GitCode Pipeline Workflows](#gitcode-pipeline-workflows)
- [Workflow YAML References](#workflow-yaml-references)
- [Out of Scope](#out-of-scope)

## Installation and Authentication

Install and upgrade from released artifacts. Prefer the Rust package first and
GitHub Releases second:

```bash
cargo install relay-gitcode-cli --force
gd --version
gd version check --json
```

Do not install from a local repository checkout as part of this skill workflow.
The skill is independently published and should use the released `gd` CLI.

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

List, add, and reply to Pull Request review comments:

```bash
gd pr comments 1 --repo owner/repo --limit 50 --json
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3 --need-to-resolve --json
gd pr reply 1 discussion-id --repo owner/repo --body "fixed" --json
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

## GitCode Pipeline Workflows

GitCode workflow commands operate against GitCode and reuse the normal `gd`
authentication path. Create or update workflow YAML and CodeCheck workflow files
through the repository contents API:

```bash
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml --json
gd pipeline codecheck \
  --repo owner/repo \
  --language SHELL \
  --access-token-secret CODECHECK_ACCESS_TOKEN \
  --json
```

The generated CodeCheck workflow references the configured secret expression and
does not commit a token value. It filters push and pull request events by the
configured target branch, then passes the pull request source repository and
branch or the current push repository/ref to `codecheck-action@0.0.3`.

Run, inspect, and control GitCode workflow runs:

```bash
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main --json
gd pipeline runs --repo owner/repo --workflow-name ci --status success --limit 20 --json
gd pipeline view --repo owner/repo workflow-run-id --json
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline stop --repo owner/repo workflow-run-id --json
gd pipeline retry --repo owner/repo workflow-run-id --job-run-id job-run-id --json
gd pipeline rerun --repo owner/repo workflow-run-id --json
```

`gd pipeline log` prints raw log text by default. Add `--json` when callers need
the full response envelope.

OpenLibing provides GitCode PR gate and CodeCheck status. Authenticate
separately from `GITCODE_TOKEN`:

```bash
gd pipeline auth login
gd pipeline auth status --json
```

For automation, provide `GD_OPENLIBING_TOKEN` or `GD_OPENLIBING_COOKIE`.
OpenLibing commands require the OpenLibing project id:

```bash
gd pipeline config --project-id openlibing-project-id --json
gd pipeline setup --project-id openlibing-project-id --repo owner/repo --language Rust --codecheck-rule-set default --json
gd pipeline prs --project-id openlibing-project-id --repo owner/repo --state open --json
gd pipeline checks --project-id openlibing-project-id --repo owner/repo --pr 1 --json
gd pipeline gate-view --project-id openlibing-project-id --repo owner/repo --pr 1 --json
gd pipeline gate-runs --project-id openlibing-project-id --pipeline-name codecheck --limit 20 --json
```

`gd pipeline checks` falls back to the OpenLibing CodeCheck task summary when
the CICD PR check endpoint is not readable. `gd pipeline setup
--codecheck-rule-set` accepts either a rule-set name or a direct rule-set ID.
For setup failures on OpenLibing repository add/update, check the documented
OpenLibing prerequisites before retrying: project administrator or equivalent
project approver role, repository recorded in Code Repository Management, PR
takeover enabled, CodeCheck language/rule set selected, GitCode public or robot
account repository access, and webhook configuration permission.

## Workflow YAML References

For examples of `.gitcode/workflows/ci.yml` structure, event triggers, runners,
checkout steps, and language-specific CI templates, read
`references/gitcode-workflow-yml.md`.

## Out of Scope

Do not use this skill for GitHub-only gh functionality unless GitCode has an
equivalent API and the task can be completed through `gd` or `gd api`. Do not
manage GitHub Actions workflows, GitHub projects, gists, codespaces, GitHub
rulesets, extensions, or Copilot through this skill.
