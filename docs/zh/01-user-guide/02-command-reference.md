# 第二章：命令参考

`gd` 只在 GitCode 暴露等价 API 能力时采用 gh 风格命令名称。

## 核心命令

```bash
gd auth login --with-token
gd auth status --json
gd auth status --format json
gd api /user --json
gd completion bash
```

`--json` 与 `--format json` 都会将成功命令的输出渲染为 JSON。命令解析失败
时，如果传入任一标志，`gd` 会向 stderr 写入单行 JSON diagnostic，字段包括
`error`、`matched_path`、`unexpected_token`、`expected`、`suggestion` 和
`usage`。文本 diagnostic 会尽量包含 `Try:` 与 `Usage:` 行。

## 仓库命令

```bash
gd repo view owner/repo
gd repo list owner
gd repo clone owner/repo
gd repo create name --private --description "demo"
gd repo fork owner/repo
```

## Issue 与 Pull Request 命令

```bash
gd issue list --repo owner/repo
gd issue view 1 --repo owner/repo
gd issue create --repo owner/repo --title "bug" --body "details"
gd issue comment 1 --repo owner/repo --body "thanks"

gd pr list --repo owner/repo
gd pr view 1 --repo owner/repo
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature
gd pr comments 1 --repo owner/repo --limit 50
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3 --need-to-resolve
gd pr reply 1 discussion-id --repo owner/repo --body "fixed"
```

## 流水线命令

GitCode workflow 命令使用 `GITCODE_TOKEN` 或系统 keyring 中的 GitCode API
凭据。OpenLibing 门禁命令使用独立的 OpenLibing GitCode OAuth 凭据，因为
GitCode pull request 门禁检查由 OpenLibing 提供。交互场景使用
`gd pipeline auth login`；自动化场景可设置 `GD_OPENLIBING_TOKEN` 或
`GD_OPENLIBING_COOKIE`。OpenLibing gateway 默认是
`https://www.openlibing.com/gateway`，可通过 `--openlibing-base` 或
`GD_OPENLIBING_BASE` 覆盖。

```bash
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --mode update --sha file-sha --file workflow.yml
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main --input dry_run=true
gd pipeline runs --repo owner/repo --workflow-name ci --status success
gd pipeline view --repo owner/repo workflow-run-id
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline stop --repo owner/repo workflow-run-id
gd pipeline retry --repo owner/repo workflow-run-id --job-run-id job-id
gd pipeline rerun --repo owner/repo workflow-run-id

gd pipeline auth login
gd pipeline auth status
gd pipeline auth logout
gd pipeline config --project-id openlibing-project-id
gd pipeline setup --project-id openlibing-project-id --repo owner/repo --language Rust --codecheck-rule-set default
gd pipeline prs --project-id openlibing-project-id --repo owner/repo --state open
gd pipeline checks --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-view --project-id openlibing-project-id --repo owner/repo --pr 1
gd pipeline gate-runs --project-id openlibing-project-id --pipeline-name codecheck
```

`gd pipeline set` 通过 GitCode 仓库 contents API 写入 workflow YAML。
`gd pipeline codecheck` 会写入 `.gitcode/workflows/codecheck.yml`，使用
`codecheck-action@0.0.3`，并引用配置的 secret 名称而不是把个人访问 token
写进仓库。生成的 CodeCheck action 会在 pull request 事件中检查源分支，在
pull request 来自 fork 时也会使用源仓库 URL；push 事件会检查配置的仓库 URL
和当前 ref。`gd pipeline log` 默认输出原始日志文本；添加 `--json` 可保留
完整响应结构。

OpenLibing 命令中的 `--project-id` 是 OpenLibing 项目 ID。`gd pipeline
setup` 会在 OpenLibing 中录入或更新 GitCode 仓库，开启 PR 接管和门禁自动触发，
应用指定的 CodeCheck 规则集，并请求 OpenLibing 配置 webhook。如果 OpenLibing
需要仓库机器人 token，可传 `--public-token-env GITCODE_TOKEN` 或其他环境变量名；
token 不会被打印。部分查询可以省略 `--repo owner/repo`，但 PR 检查查询建议
显式传入，因为 OpenLibing 的 GitCode PR endpoint 接收 owner 和 repo 过滤条件。
如果 OpenLibing CICD PR 检查接口不可读，`gd pipeline checks` 会按同一仓库和
PR 回退查询 CodeCheck 任务汇总。`--codecheck-rule-set` 可以传规则集名称，也
可以直接传规则集 ID；当规则集列表不可读时可使用直接 ID。

OpenLibing 会在服务端校验仓库配置权限。帮助中心给出的正确链路是：由项目
管理员或等价项目审批人员在代码仓管理中录入 GitCode 仓库，开启 PR 接管，
选择 CodeCheck 语言和规则集，确保 GitCode 公共账号或机器人账号具备仓库访问
权限，并配置 webhook。无头浏览器仍然使用同一账号，无法绕过这些管理接口的
`403 Forbidden`。

## 其他 GitCode 资源

```bash
gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
gd version check
```

codespaces、gists、GitHub Actions workflows、projects、rulesets、extensions、
Copilot 等 GitHub 专属命令不会进入 `gd`。

## 版本检查

```bash
gd version check
gd version check --json
```

`gd version check` 会读取 GitHub Releases 和 crates.io，报告是否存在更新的
稳定版 `relay-gitcode-cli`。它不会替换当前二进制。

## Skill-over-CLI

仓库随附 `skills/relay-gitcode-cli`，这是一个兼容 ClawHub 的 skill，用于让
LLM agent 通过本地 `gd` CLI 操作 GitCode，并解析 JSON 输出。它覆盖认证检查、
仓库工作流、Issue、Pull Request、搜索、SSH key、标签、Release、OpenLibing
流水线门禁、原始 `gd api` 调用和 shell completion。

该 skill 只保持在 GitCode 支持的 `gd` 行为范围内。除非 GitCode 提供可通过
`gd api` 访问的等价 API，否则它不会增加 GitHub 专属的 `gh` 命令面。

请从发布产物或 ClawHub package 安装该 skill，不要通过本地仓库 checkout
构建 `gd` 来安装 skill。
