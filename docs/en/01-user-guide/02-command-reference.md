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
```

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

## GitCode Pipeline Commands

Pipeline commands manage GitCode workflow files under `.gitcode/workflows` and
read GitCode Actions run records and logs. They use the same GitCode personal
access token as other commands through `GITCODE_TOKEN` or
`gd auth login --with-token`; no AK/SK credentials are required. `gd actions`
is available as an alias for `gd pipeline`.

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
```

`gd pipeline set` writes workflow YAML through the GitCode repository contents
API. `gd pipeline codecheck` writes `.gitcode/workflows/codecheck.yml` with
`codecheck-action@0.0.3` and references the configured secret name instead of
embedding a personal access token. The generated CodeCheck action checks the
source repository and branch for pull request events, and the configured
repository URL plus current ref for push events.
`gd pipeline log` prints raw log text by default; add `--json` to keep the full
response envelope.

## Other GitCode Resources

```bash
gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd version check
```

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
LLM agents that should operate GitCode by invoking the local `gd` CLI and
parsing JSON output. It covers authentication checks, repository workflows,
issues, pull requests, search, SSH keys, labels, releases, GitCode Pipeline
operations, raw `gd api` calls, and shell completion.

The skill intentionally stays within GitCode-backed `gd` behavior. It does not
add GitHub-only `gh` command surfaces unless GitCode exposes an equivalent API
that can be reached through `gd api`.

Install the skill from its published release artifact or ClawHub package. Do
not install it by building `gd` from a local repository checkout.
