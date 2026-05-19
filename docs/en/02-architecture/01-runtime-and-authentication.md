# Chapter 1: Runtime and Authentication

## Layers

- `src/cli.rs` defines the clap command tree.
- `src/commands/mod.rs` maps parsed commands to GitCode API calls.
- `src/client.rs` owns HTTP request construction, authentication headers,
  pagination, JSON decoding, and API error mapping.
- `src/pipeline.rs` owns CodeArts Pipeline endpoint construction, request body
  shaping, AK/SK request signing, and pipeline API error mapping.
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

Pipeline commands use `GITCODE_PIPELINE_API_BASE` and
`GITCODE_PIPELINE_DOMAIN_ID` for the CodeArts Pipeline host and tenant scope.
They prefer AK/SK signing through `HUAWEICLOUD_SDK_AK` and
`HUAWEICLOUD_SDK_SK`, with `CLOUD_SDK_AK` and `CLOUD_SDK_SK` as aliases.
If AK/SK is absent, the stored GitCode token is sent as a Bearer token.

## Command Boundary

First-class commands cover GitCode API equivalents. `gd api` remains the escape
hatch for newer or less common endpoints without expanding the public command
tree prematurely.
