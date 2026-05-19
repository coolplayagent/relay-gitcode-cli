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
- When commands, flags, examples, or user-facing command behavior change,
  update `skills/relay-gitcode-cli/SKILL.md` in the same change.
- Prefer fully async, high-throughput Rust paths for runtime work. Use Tokio
  filesystem, stdin, and process APIs in async command execution; isolate
  unavoidable blocking integrations such as keyring access.
- HTTP clients must reuse reqwest system proxy behavior, including
  `HTTP_PROXY`/`http_proxy`, `HTTPS_PROXY`/`https_proxy`,
  `ALL_PROXY`/`all_proxy`, and `NO_PROXY`/`no_proxy`, and default to disabled
  TLS certificate verification for GitCode API calls.

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
