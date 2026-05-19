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
tests, `GITCODE_TOKEN` takes precedence and avoids writing credentials to disk.

## API Host

The default API base is:

```text
https://api.gitcode.com/api/v5
```

Override it with `--api-base` or `GITCODE_API_BASE` when testing compatible
hosts.
