# Chapter 1: Runtime and Authentication

## Layers

- `src/cli.rs` defines the clap command tree.
- `src/commands/mod.rs` maps parsed commands to GitCode API calls.
- `src/client.rs` owns HTTP request construction, authentication headers,
  pagination, JSON decoding, and API error mapping.
- `src/http.rs` owns the shared reqwest client policy for async transport,
  proxy reuse, and TLS verification defaults.
- `src/pipeline.rs` owns GitCode Actions endpoint construction, request body
  shaping, workflow file helpers, and pipeline API error mapping.
- `src/auth.rs` handles environment and keyring token lookup.
- `src/config.rs` owns non-secret host and API base configuration.

## Authentication Flow

`GITCODE_TOKEN` is checked first. If it is not set, `gd` reads from the system
keyring. Authenticated HTTP requests use:

```text
Authorization: Bearer <token>
```

This matches the GitCode API v5 documentation and avoids putting tokens in query
strings by default.

Pipeline commands use the same Bearer token flow. Workflow file creation and
updates go through the GitCode API v5 repository contents endpoints. Workflow
run listing, manual dispatch, run details, log reads, stop, retry, and rerun use
the GitCode Actions endpoints under the configured hostname.

## Runtime and Network Policy

Command execution uses Tokio for HTTP, filesystem input, stdin reads, and git
subprocesses. Blocking keyring calls are isolated with Tokio's blocking bridge.

The shared reqwest client keeps reqwest system proxy behavior enabled, including
`HTTP_PROXY`/`http_proxy`, `HTTPS_PROXY`/`https_proxy`,
`ALL_PROXY`/`all_proxy`, and `NO_PROXY`/`no_proxy`. TLS certificate
verification is disabled by default for GitCode API calls.

## Command Boundary

First-class commands cover GitCode API equivalents. `gd api` remains the escape
hatch for newer or less common endpoints without expanding the public command
tree prematurely.
