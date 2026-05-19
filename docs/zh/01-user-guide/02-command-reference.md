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
```

## GitCode 流水线命令

流水线命令管理 `.gitcode/workflows` 下的 GitCode workflow 文件，并读取
GitCode Actions 运行记录和日志。它们与其他命令一样使用 GitCode 个人访问
token，可通过 `GITCODE_TOKEN` 或 `gd auth login --with-token` 提供；不需要
AK/SK。`gd actions` 可作为 `gd pipeline` 的别名。

```bash
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline set --repo owner/repo .gitcode/workflows/ci.yml --mode update --sha file-sha --file workflow.yml
gd pipeline list --repo owner/repo
gd pipeline run --repo owner/repo workflow-id --file-path .gitcode/workflows/ci.yml --branch main --input dry_run=true
gd pipeline runs --repo owner/repo --workflow-name ci --status success
gd pipeline view --repo owner/repo workflow-run-id
gd pipeline log --repo owner/repo workflow-run-id job-id
gd pipeline stop --repo owner/repo workflow-run-id
gd pipeline retry --repo owner/repo workflow-run-id --job-run-id job-id
gd pipeline rerun --repo owner/repo workflow-run-id
```

`gd pipeline set` 通过 GitCode 仓库 contents API 写入 workflow YAML。
`gd pipeline log` 默认输出原始日志文本；添加 `--json` 可保留完整响应结构。

## 其他 GitCode 资源

```bash
gd search repos query
gd search issues query
gd search users query
gd ssh-key list
gd label list --repo owner/repo
gd release list --repo owner/repo
```

codespaces、gists、GitHub Actions workflows、projects、rulesets、extensions、
Copilot 等 GitHub 专属命令不会进入 `gd`。
