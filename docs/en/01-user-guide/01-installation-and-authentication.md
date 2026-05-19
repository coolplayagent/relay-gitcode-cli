# Chapter 1: Installation and Authentication

## Build From Source

```bash
cargo build
target/debug/gd --version
```

For an optimized binary:

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
