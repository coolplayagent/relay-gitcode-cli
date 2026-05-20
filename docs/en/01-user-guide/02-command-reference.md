# Chapter 2: Command Reference

`gd` follows gh-style command names only where GitCode exposes equivalent API
behavior.

## Core Commands

```bash
gd auth login --with-token
gd auth status --json
gd auth status --format json
gd api /user --json
gd completion bash
```

`--json` and `--format json` both render successful command output as JSON.
When command parsing fails and either flag is present, `gd` writes a single-line
JSON diagnostic to stderr with `error`, `matched_path`, `unexpected_token`,
`expected`, `suggestion`, and `usage` fields. Text diagnostics include best-effort
`Try:` and `Usage:` lines.

## Repository Commands

```bash
gd repo view owner/repo
gd repo list owner
gd repo clone owner/repo
gd repo create name --private --description "demo"
gd repo fork owner/repo
gd repo move owner/repo target-owner/new-name
gd repo move owner/repo target-owner --name new-name
gd repo sync-github coolplayagent/relay-gitcode-cli --org plm-cac --private
gd repo sync-github git@github.com:owner/repo.git --repo target-org/repo --if-exists skip
gd repo sync-github owner/repo --repo target-org/repo --method git-push --if-exists update
```

`gd repo move` moves a repository to another GitCode user or organization. Pass
`target-owner/new-name` or `--name new-name` to rename during the move. When the
target owner matches the current owner, `gd repo move` only renames the
repository.

`gd repo sync-github` creates a GitCode repository from a GitHub repository
`import_url` by default. Without `--org` or `--repo`, it imports into the
authenticated GitCode user namespace. Use `--org` for an organization, `--repo
owner/name` for an explicit target path, and `--name` to rename the imported
project. Use `--method git-push` to create a regular GitCode repository and
push GitHub branches and tags with Git instead of enabling Pull mirroring; this
keeps the target writable for GitCode Release creation. Existing targets are
skipped by default; `--if-exists update` pushes refs to an existing regular
target, while `--if-exists recreate` deletes and recreates the target.

## Issue and Pull Request Commands

```bash
gd issue list --repo owner/repo
gd issue view 1 --repo owner/repo
gd issue create --repo owner/repo --title "bug" --body "details"
gd issue comment 1 --repo owner/repo --body "thanks"

gd pr list --repo owner/repo
gd pr view 1 --repo owner/repo
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature
gd pr comments 1 --repo owner/repo --limit 50
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3 --need-to-resolve
gd pr reply 1 discussion-id --repo owner/repo --body "fixed"
```

## Pipeline Commands

GitCode workflow commands use GitCode API credentials from `GD_TOKEN`,
`GITCODE_TOKEN`, or the system keyring. OpenLibing gate commands use separate
OpenLibing GitCode OAuth credentials, because GitCode pull-request gate checks
are provided by OpenLibing. Use `gd pipeline auth login` for browser OAuth, or provide
`GD_OPENLIBING_TOKEN` or `GD_OPENLIBING_COOKIE` in automation. The OpenLibing
gateway defaults to `https://www.openlibing.com/gateway` and can be overridden
with `--openlibing-base` or `GD_OPENLIBING_BASE`.

```bash
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --mode update --sha file-sha --file workflow.yml
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main --input dry_run=true
gd pipeline runs --repo owner/repo --workflow-name ci --status success
gd pipeline view --repo owner/repo workflow-run-id
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline stop --repo owner/repo workflow-run-id
gd pipeline retry --repo owner/repo workflow-run-id --job-run-id job-id
gd pipeline rerun --repo owner/repo workflow-run-id

gd pipeline auth login
gd pipeline auth status
gd pipeline auth logout
gd pipeline config --project-id openlibing-project-id
gd pipeline setup --project-id openlibing-project-id --repo owner/repo --language Rust --codecheck-rule-set default
gd pipeline prs --project-id openlibing-project-id --repo owner/repo --state open
gd pipeline checks --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-view --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-runs --project-id openlibing-project-id --pipeline-name codecheck
```

`gd pipeline set` writes workflow YAML through the GitCode repository contents
API. `gd pipeline codecheck` writes `.gitcode/workflows/codecheck.yml` with
`codecheck-action@0.0.3` and references the configured secret name instead of
embedding a personal access token. The generated CodeCheck action checks the
source repository and branch for pull request events, and the configured
repository URL plus current ref for push events. `gd pipeline log` prints raw
log text by default; add `--json` to keep the full response envelope.

For OpenLibing commands, `--project-id` is the OpenLibing project ID. `gd
pipeline setup` records or updates the GitCode repository in OpenLibing,
enables PR takeover and automatic gate triggering, applies the requested
CodeCheck rule set, and asks OpenLibing to configure the webhook. If OpenLibing
needs a repository robot token, pass `--public-token-env GD_TOKEN`,
`--public-token-env GITCODE_TOKEN`, or another environment variable name; the
token is never printed. `--repo
owner/repo` is optional for some queries, but recommended for PR check lookups
because OpenLibing's GitCode PR endpoints accept owner and repository filters.
If the OpenLibing CICD PR check endpoint is not readable, `gd pipeline checks`
falls back to the CodeCheck task summary for the same repository and PR.
`--codecheck-rule-set` may be either a rule-set name or a direct rule-set ID; a
direct ID can be used when the rule-set list is not readable.

OpenLibing enforces repository setup permissions server-side. The correct setup
path from the help center is: a project administrator or equivalent project
approver records the GitCode repository in Code Repository Management, enables
PR takeover, selects the CodeCheck language and rule set, ensures the GitCode
public or robot account can access the repository, and configures the webhook.
Headless browser automation still uses the same account and cannot bypass a
`403 Forbidden` response from these management endpoints.

## Other GitCode Resources

```bash
gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd release migrate-github --repo owner/repo --github-repo source/repo --tag v1.0.0
gd release migrate-github --repo owner/repo --github-repo source/repo --tag v1.0.0 --update-release=false --skip-existing-assets=false
gd version check
```

`gd release migrate-github` reads GitHub Release metadata and uploaded assets,
then creates or updates the matching GitCode Release. Use `--dry-run` to preview
the migration and `--all` instead of `--tag` to backfill historical releases.
Existing GitCode assets with the same name are skipped by default. Set
`--update-release=false` to leave existing Release metadata unchanged, or
`--skip-existing-assets=false` to fail when a matching GitCode asset already
exists.

GitHub-only surfaces such as codespaces, gists, GitHub Actions workflows,
projects, rulesets, extensions, and Copilot are intentionally excluded.

## Version Checks

```bash
gd version check
gd version check --json
```

`gd version check` reads GitHub Releases and crates.io to report whether a
newer stable `relay-gitcode-cli` release is available. It does not replace the
current binary.

## Skill-over-CLI

The repository ships `skills/relay-gitcode-cli`, a ClawHub-compatible skill for
LLM agents that should operate GitCode by invoking `gd` and parsing JSON output.
The release skill package includes Linux x64 and Windows x64 `gd` binaries,
prefers the newest usable bundled or `PATH` binary, and falls back to released
online artifacts when needed. It covers authentication checks, repository
workflows, issues, pull requests, search, SSH keys, labels, releases,
OpenLibing pipeline operations, raw `gd api` calls, and shell completion.

The skill intentionally stays within GitCode-backed `gd` behavior. It does not
add GitHub-only `gh` command surfaces unless GitCode exposes an equivalent API
that can be reached through `gd api`.

Install the skill from its published release artifact or ClawHub package. Do
not install it by building `gd` from a local repository checkout.
