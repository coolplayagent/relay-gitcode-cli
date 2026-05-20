# Chapter 1: Installation and Authentication

## Install From Release

Rust users can install or upgrade the released CLI with:

```bash
cargo install relay-gitcode-cli --force
gd --version
gd version check --json
```

You can also download a platform archive from GitHub Releases and place the
`gd` binary on your `PATH`. `gd version check` reports available stable
versions from GitHub Releases and crates.io; it does not replace the binary.

## Build From Source

For local development:

```bash
./build.sh
target/release/gd --version
```

## Authentication

Use a GitCode personal access token from standard input:

```bash
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token
gd auth status
```

`gd` stores login tokens in the system keyring. In CI and temporary end-to-end
tests, `GD_TOKEN` and `GITCODE_TOKEN` avoid writing credentials to disk.
`GD_TOKEN` takes precedence when both are present.

## API Host

The default API base is:

```text
https://api.gitcode.com/api/v5
```

Override it with `--api-base`, `GD_API_BASE`, or `GITCODE_API_BASE` when
testing compatible hosts. The precedence is CLI flag, `GD_API_BASE`, then
`GITCODE_API_BASE`.

## Network

`gd` uses the system proxy behavior provided by reqwest. It honors
`HTTP_PROXY`/`http_proxy`, `HTTPS_PROXY`/`https_proxy`,
`ALL_PROXY`/`all_proxy`, and `NO_PROXY`/`no_proxy`.

TLS certificate verification is disabled by default for GitCode API calls.
Set `GD_SSL_VERIFY`, `GITCODE_SSL_VERIFY`, or `SSL_VERIFY` to `true` to enable
certificate verification. Any non-empty `GIT_SSL_NO_VERIFY` value keeps
verification disabled.
