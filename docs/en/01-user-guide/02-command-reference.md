# Chapter 2: Command Reference

`gd` follows gh-style command names only where GitCode exposes equivalent API
behavior.

## Core Commands

```bash
gd auth login --with-token
gd auth status --json
gd api /user --json
gd completion bash
```

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
```

## GitCode Pipeline Commands

Pipeline commands call the CodeArts Pipeline GitCode APIs. Set
`GITCODE_PIPELINE_API_BASE` to the region endpoint, such as
`https://devcloud.ap-southeast-3.myhuaweicloud.com`, and set
`GITCODE_PIPELINE_DOMAIN_ID` to the tenant domain ID. AK/SK signing is preferred
with `HUAWEICLOUD_SDK_AK`/`HUAWEICLOUD_SDK_SK` or
`CLOUD_SDK_AK`/`CLOUD_SDK_SK`; when AK/SK is not configured, `gd` falls back to
the stored GitCode token as a Bearer token.

```bash
gd pipeline register --repo owner/repo --type create --new-file-path .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline run --repo owner/repo --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --pipeline-name ci --status success
gd pipeline view pipeline-id pipeline-run-id
gd pipeline log pipeline-id pipeline-run-id job-run-id
gd pipeline stop pipeline-id pipeline-run-id
gd pipeline retry pipeline-id pipeline-run-id
```

Use `--pipeline-api-base` and `--pipeline-domain-id` to override the environment
for one command. `gd pipeline log` prints raw log text by default; add `--json`
to keep the full response envelope.

## Other GitCode Resources

```bash
gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
```

GitHub-only surfaces such as codespaces, gists, GitHub Actions workflows,
projects, rulesets, extensions, and Copilot are intentionally excluded.
