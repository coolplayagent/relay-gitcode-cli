# Chapter 1: Runtime and Authentication

## Layers

- `src/cli.rs` defines the clap command tree.
- `src/commands/mod.rs` maps parsed commands to GitCode API calls.
- `src/client.rs` owns HTTP request construction, authentication headers,
  pagination, JSON decoding, and API error mapping.
- `src/http.rs` owns the shared reqwest client policy for async transport,
  proxy reuse, and TLS verification defaults.
- `src/pipeline.rs` owns OpenLibing gateway endpoint construction, OAuth
  callback parsing, request authentication, and pipeline gate API error mapping.
- `src/auth.rs` handles environment and keyring token lookup.
- `src/config.rs` owns non-secret host and API base configuration.

## Authentication Flow

`GD_TOKEN` is checked first, followed by `GITCODE_TOKEN`. If neither is set,
`gd` reads from the system keyring. Authenticated HTTP requests use:

```text
Authorization: Bearer <token>
```

This matches the GitCode API v5 documentation and avoids putting tokens in query
strings by default.

Pipeline gate commands use OpenLibing credentials, not `GITCODE_TOKEN`.
`GD_OPENLIBING_TOKEN`, `GD_OPENLIBING_COOKIE`, and `GD_OPENLIBING_CSRF_TOKEN`
are checked first. Otherwise `gd pipeline auth login` stores the OpenLibing
credential material in a separate keyring entry. OpenLibing requests send
available bearer, cookie, and CSRF headers to the configured gateway.
Repository gate setup is also OpenLibing-scoped: `gd pipeline setup` sends the
GitCode repository URL, PR takeover flags, automatic gate-trigger flags,
CodeCheck rule-set selection, and optional public-account token material to
OpenLibing. Any token read from `--public-token-env` is sent only in the
OpenLibing request body and is redacted from command output.
OpenLibing still authorizes repository maintenance on the server. A `403`
during repository add/update means the account needs project-administrator or
equivalent project-approver permission, and browser automation cannot change
that authorization result.

## Runtime and Network Policy

Command execution uses Tokio for HTTP, filesystem input, stdin reads, and git
subprocesses. Blocking keyring calls are isolated with Tokio's blocking bridge.

The shared reqwest client keeps reqwest system proxy behavior enabled, including
`HTTP_PROXY`/`http_proxy`, `HTTPS_PROXY`/`https_proxy`,
`ALL_PROXY`/`all_proxy`, and `NO_PROXY`/`no_proxy`. TLS certificate
verification is disabled by default for GitCode API calls. `GD_SSL_VERIFY`,
`GITCODE_SSL_VERIFY`, and `SSL_VERIFY` can enable or disable verification, and
any non-empty `GIT_SSL_NO_VERIFY` value is supported as the Git-style disable
switch.

## Command Boundary

First-class commands cover GitCode API equivalents. `gd api` remains the escape
hatch for newer or less common endpoints without expanding the public command
tree prematurely.
