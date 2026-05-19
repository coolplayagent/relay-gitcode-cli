[English](README.md) | [中文](README.zh-CN.md)

# relay-gitcode-cli

`relay-gitcode-cli` 提供 `gd`，一个面向 GitCode 的 Rust 命令行工具。它在
GitCode API 具备等价能力的地方采用 gh 风格命令命名，并保持 GitCode 特有行为清晰可见。

GitCode API 文档：https://docs.gitcode.com/docs/apis/

## 快速开始

```bash
cargo install relay-gitcode-cli --force
gd --version
gd version check --json
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token
gd auth status
gd repo view owner/repo --json
```

`GITCODE_TOKEN` 可用于 CI 和临时端到端测试。没有环境变量 token 时，
`gd auth login --with-token` 会把 token 保存到系统 keyring。

OpenLibing 提供的流水线门禁使用独立凭据。交互场景使用
`gd pipeline auth login` 完成浏览器 OAuth；自动化场景可设置
`GD_OPENLIBING_TOKEN` 或 `GD_OPENLIBING_COOKIE`。

每个 GitHub Release 还会包含
`relay-gitcode-cli-skill-<tag>.tar.gz`，这是一个纯文本、兼容 ClawHub
的 skill，用于引导 LLM agent 通过本地 `gd` CLI 使用 GitCode API v5
工作流。该 skill 应从发布包或 ClawHub 安装，不应从本地仓库 checkout
安装。配置 `CLAWHUB_TOKEN` 后，release workflow 可以把同一个
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
gd pr comments 1 --repo owner/repo --limit 50
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3
gd pr reply 1 discussion-id --repo owner/repo --body "fixed"

gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --workflow-name ci
gd pipeline view --repo owner/repo workflow-run-id
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline auth login
gd pipeline auth status
gd pipeline config --project-id openlibing-project-id
gd pipeline setup --project-id openlibing-project-id --repo owner/repo --language Rust --codecheck-rule-set default
gd pipeline prs --project-id openlibing-project-id --repo owner/repo
gd pipeline checks --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-view --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-runs --project-id openlibing-project-id --pipeline-name codecheck

gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd version check
gd completion bash
```

当 OpenLibing CICD PR 检查接口不可读时，`gd pipeline checks` 会回退查询
OpenLibing CodeCheck 任务汇总。`gd pipeline setup --codecheck-rule-set`
既可传规则集名称，也可直接传规则集 ID。
OpenLibing 帮助中心中的正确流程要求先由项目管理员或等价项目审批人员维护
仓库：在代码仓管理中录入 GitCode 仓库，开启 PR 接管，选择 CodeCheck 语言和
规则集，确保 GitCode 公共账号或机器人账号具备仓库访问权限，并允许配置
webhook。浏览器或无头浏览器自动化不能绕过这些服务端权限。

codespaces、gists、GitHub Actions workflow 管理、projects、rulesets、
extensions、Copilot 等 GitHub 专属命令不会进入 `gd`，除非 GitCode 提供等价 API。

## 网络

`gd` 支持 reqwest 系统代理环境变量：`HTTP_PROXY`/`http_proxy`、
`HTTPS_PROXY`/`https_proxy`、`ALL_PROXY`/`all_proxy` 和
`NO_PROXY`/`no_proxy`。GitCode API 调用默认不校验 TLS 证书。

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
