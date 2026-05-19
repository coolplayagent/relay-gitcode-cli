# AGENTS.md

This repository builds `gd`, a Rust CLI for GitCode API v5.

## Working Rules

- Keep the binary name `gd`.
- Keep the package name `relay-gitcode-cli`.
- Do not commit personal access tokens, credentials, cookies, or API responses
  that expose private account data.
- Prefer `GITCODE_TOKEN` for temporary end-to-end tests and system keyring
  storage for interactive `gd auth login --with-token`.
- Keep CLI behavior aligned with gh-style naming only where GitCode has
  equivalent API behavior.
- Do not add GitHub-only command surfaces unless GitCode exposes an equivalent
  API.

## Checks

Run the full local gate before pushing:

```bash
./check.sh
```

The gate runs formatting, clippy with warnings denied, tests, and a build.

## Documentation

Documentation is maintained as a bilingual book:

- English: `docs/en/README.md`
- Chinese: `docs/zh/README.md`

When adding or changing user-facing behavior, update both language trees.
