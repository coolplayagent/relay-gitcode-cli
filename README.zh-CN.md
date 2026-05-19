[English](README.md) | [中文](README.zh-CN.md)

# relay-gitcode-cli

`relay-gitcode-cli` 提供 `gd`，一个面向 GitCode 的 Rust 命令行工具。它在
GitCode API 具备等价能力的地方采用 gh 风格命令命名，并保持 GitCode 特有行为清晰可见。

GitCode API 文档：https://docs.gitcode.com/docs/apis/

## 快速开始

```bash
cargo build
target/debug/gd --version
printf '%s' "$GITCODE_TOKEN" | target/debug/gd auth login --with-token
target/debug/gd auth status
target/debug/gd repo view owner/repo --json
```

`GITCODE_TOKEN` 可用于 CI 和临时端到端测试。没有环境变量 token 时，
`gd auth login --with-token` 会把 token 保存到系统 keyring。

每个 GitHub Release 还会包含
`relay-gitcode-cli-skill-<tag>.tar.gz`，这是一个纯文本、兼容 ClawHub
的 skill，用于引导 LLM agent 通过本地 `gd` CLI 使用 GitCode API v5
工作流。配置 `CLAWHUB_TOKEN` 后，release workflow 可以把同一个
`skills/relay-gitcode-cli` 目录发布到 ClawHub：

```bash
clawhub publish skills/relay-gitcode-cli \
  --slug relay-gitcode-cli \
  --name "Relay GitCode CLI" \
  --version <version>
```

这条 skill-over-CLI 路径只覆盖 GitCode 支持的 `gd` 命令和 GitCode 原始
API 调用；GitHub 专属的 `gh` 命令面仍不在范围内。

## 命令

```bash
gd auth login --with-token
gd auth status
gd auth token
gd auth logout

gd api /user
gd repo view owner/repo
gd repo list owner
gd repo clone owner/repo
gd repo create name --private --description "demo"
gd repo fork owner/repo

gd issue list --repo owner/repo
gd issue view 1 --repo owner/repo
gd issue create --repo owner/repo --title "bug" --body "details"
gd issue comment 1 --repo owner/repo --body "thanks"

gd pr list --repo owner/repo
gd pr view 1 --repo owner/repo
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature

gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --workflow-name ci
gd pipeline log --repo owner/repo workflow-run-id job-id

gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd completion bash
```

codespaces、gists、GitHub Actions workflow 管理、projects、rulesets、
extensions、Copilot 等 GitHub 专属命令不会进入 `gd`，除非 GitCode 提供等价 API。

## 文档

- [文档书架](docs/README.md)
- [英文文档](docs/en/README.md)
- [中文文档](docs/zh/README.md)

## 开发

```bash
./build.sh --debug
./check.sh
```

本地质量门禁：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --all-targets --all-features
```

## CI

- GitHub Actions：`.github/workflows/pr-checks.yml`、
  `.github/workflows/code_quality.yml` 和 `.github/workflows/release.yml`。
- GitCode 流水线：`.gitcode/workflows/pr-checks.yml`。

仓库保护和 secret 配置见 `docs/repository-settings.md`。
