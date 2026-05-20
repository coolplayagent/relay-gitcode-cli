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

当参数语法可在本地确定时，`gd` 会在命令解析阶段校验结构化参数。覆盖范围包括
`owner/repo` 仓库引用、GitHub 仓库输入、HTTP(S) base URL、
`.gitcode/workflows/` 下的 workflow 路径、`key=value` API 与 workflow 输入，
以及 CodeCheck secret 名称。无效参数值会复用未知命令和未知 flag 的文本或 JSON
diagnostic 输出。

## 仓库命令

```bash
gd repo view owner/repo
gd repo list owner
gd repo clone owner/repo
gd repo create name --private --description "demo"
gd repo fork owner/repo
gd repo move owner/repo target-owner/new-name
gd repo move owner/repo target-owner --name new-name
gd repo sync-github coolplayagent/relay-gitcode-cli --org plm-cac --private
gd repo sync-github git@github.com:owner/repo.git --repo target-org/repo --if-exists skip
gd repo sync-github owner/repo --repo target-org/repo --method git-push --if-exists update
gd repo tree main --repo owner/repo --recursive
gd repo contents README.md --repo owner/repo --ref main
gd repo file-create README.md --repo owner/repo --message "init" --content "hello"
gd repo file-update README.md --repo owner/repo --message "update" --sha blob-sha --content-file README.md
gd repo file-delete old.txt --repo owner/repo --message "delete old" --sha blob-sha
gd repo raw README.md --repo owner/repo --ref main
gd repo languages --repo owner/repo
gd repo contributors --repo owner/repo
gd repo settings --repo owner/repo
gd repo settings-edit --repo owner/repo --field has_issues=true
gd repo pr-settings --repo owner/repo
gd repo push-rules --repo owner/repo
gd repo push-rules-edit --repo owner/repo --field max_file_size=10485760
gd repo forks --repo owner/repo
gd repo subscribers --repo owner/repo
gd repo stargazers --repo owner/repo
gd repo download-stats --repo owner/repo --start-date 2026-01-01
gd repo contributor-stats --repo owner/repo --current-user
gd repo events --repo owner/repo --filter push
```

`gd repo move` 可将仓库迁移到另一个 GitCode 用户或组织名下。使用
`target-owner/new-name` 或 `--name new-name` 可在迁移时同步改名；当目标 owner
与当前 owner 相同，它只执行仓库改名。

`gd repo sync-github` 会通过 GitHub 仓库 `import_url` 在 GitCode 创建并导入
仓库。不传 `--org` 或 `--repo` 时导入到当前认证的 GitCode 用户命名空间；
使用 `--org` 可导入组织，使用 `--repo owner/name` 可指定完整目标路径，
使用 `--name` 可重命名导入项目。使用 `--method git-push` 会创建普通 GitCode
仓库，并用 Git 推送 GitHub 分支和标签，而不是启用 Pull 镜像；这样目标仓库仍可
创建 GitCode Release。已有目标默认跳过；`--if-exists update` 会向已有普通目标
推送 refs，`--if-exists recreate` 会删除并重建目标仓库。

部分 GitCode 仓库设置接口请求体较宽，`gd` 使用可重复的 `--field key=value`
暴露这些接口。`true`、`false`、`null` 和数字会作为 JSON 标量发送，其余值作为
字符串发送。

## Issue 与 Pull Request 命令

```bash
gd issue list --repo owner/repo
gd issue view 1 --repo owner/repo
gd issue create --repo owner/repo --title "bug" --body "details"
gd issue edit 1 --repo owner/repo --state close
gd issue comment 1 --repo owner/repo --body "thanks"
gd issue comments 1 --repo owner/repo --limit 50
gd issue repo-comments --repo owner/repo
gd issue comment-view 123 --repo owner/repo
gd issue comment-edit 123 --repo owner/repo --body "updated"
gd issue comment-delete 123 --repo owner/repo
gd issue label-add 1 --repo owner/repo --label bug
gd issue label-remove 1 bug --repo owner/repo
gd issue prs 1 --repo owner/repo
gd issue logs 1 --repo owner/repo
gd issue user --state open
gd issue org my-org --state open
gd issue enterprise-list my-enterprise --state open

gd pr list --repo owner/repo
gd pr org my-org --state open
gd pr enterprise my-enterprise --state open
gd pr view 1 --repo owner/repo
gd pr create --repo owner/repo --title "change" --body "details" --base main --head feature
gd pr edit 1 --repo owner/repo --title "new title"
gd pr merge 1 --repo owner/repo --merge-method squash
gd pr merge-status 1 --repo owner/repo
gd pr commits 1 --repo owner/repo
gd pr files 1 --repo owner/repo
gd pr changes 1 --repo owner/repo
gd pr issues 1 --repo owner/repo
gd pr logs 1 --repo owner/repo
gd pr comments 1 --repo owner/repo --limit 50
gd pr comment 1 --repo owner/repo --body "please fix" --path src/main.rs --position 3 --need-to-resolve
gd pr reply 1 discussion-id --repo owner/repo --body "fixed"
gd pr comment-view 123 --repo owner/repo
gd pr comment-edit 123 --repo owner/repo --body "updated"
gd pr comment-delete 123 --repo owner/repo
gd pr labels 1 --repo owner/repo
gd pr label-add 1 --repo owner/repo --label bug
gd pr label-replace 1 --repo owner/repo --label bug --label docs
gd pr label-remove 1 bug --repo owner/repo
gd pr review approve 1 --repo owner/repo
gd pr review test 1 --repo owner/repo
gd pr review reset-approval 1 --repo owner/repo --reset-all
gd pr review reset-test 1 --repo owner/repo --reset-all
gd pr review assign-approver 1 --repo owner/repo --user alice
gd pr review cancel-approver 1 --repo owner/repo --user alice
gd pr review assign-tester 1 --repo owner/repo --user bob
```

GitCode 的 Pull Request review 模型是审批人与测试人流程，因此 `gd pr review`
映射到 GitCode approval/test 相关接口，而不是 GitHub review state。

## 流水线命令

GitCode workflow 命令使用 `GD_TOKEN`、`GITCODE_TOKEN` 或系统 keyring 中的
GitCode API 凭据。OpenLibing 门禁命令使用独立的 OpenLibing GitCode OAuth 凭据，因为
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
需要仓库机器人 token，可传 `--public-token-env GD_TOKEN`、
`--public-token-env GITCODE_TOKEN` 或其他环境变量名；token 不会被打印。部分查询可以省略 `--repo owner/repo`，但 PR 检查查询建议
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
gd tag list --repo owner/repo
gd tag create v1.0.0 --repo owner/repo --ref main --message "v1.0.0"
gd tag protected-list --repo owner/repo
gd tag protected-create "v*" --repo owner/repo --create-access-level 30
gd milestone list --repo owner/repo
gd milestone create --repo owner/repo --title v1 --due-on 2026-06-01
gd milestone edit 1 --repo owner/repo --state closed
gd release list --repo owner/repo
gd release view v1.0.0 --repo owner/repo
gd release view-id 1 --repo owner/repo
gd release create v1.0.0 --repo owner/repo --title v1.0.0 --notes "notes"
gd release edit 1 --repo owner/repo --title v1.0.0
gd release upload v1.0.0 dist/app.tar.gz --repo owner/repo
gd release migrate-github --repo owner/repo --github-repo source/repo --tag v1.0.0
gd release migrate-github --repo owner/repo --github-repo source/repo --tag v1.0.0 --update-release=false --skip-existing-assets=false
gd version check
```

`gd release migrate-github` 会读取 GitHub Release 元数据和上传的附件，
然后在 GitCode 上创建或更新同 tag 的 Release。使用 `--dry-run` 可预览
迁移内容，使用 `--all` 代替 `--tag` 可补齐历史 Release。GitCode 上已有的
同名附件默认会跳过。设置 `--update-release=false` 可保留已有 Release 元数据，
设置 `--skip-existing-assets=false` 可在 GitCode 已有同名附件时直接失败。

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
LLM agent 通过 `gd` 操作 GitCode，并解析 JSON 输出。release skill 包内置
Linux x64 和 Windows x64 `gd` 二进制，会优先使用内置或 `PATH` 中版本最新的
可用二进制，必要时再回退到发布版在线产物。它覆盖认证检查、仓库工作流、Issue、
Pull Request、搜索、SSH key、标签、Release、OpenLibing 流水线门禁、原始
`gd api` 调用和 shell completion。

该 skill 只保持在 GitCode 支持的 `gd` 行为范围内。除非 GitCode 提供可通过
`gd api` 访问的等价 API，否则它不会增加 GitHub 专属的 `gh` 命令面。

请从发布产物或 ClawHub package 安装该 skill，不要通过本地仓库 checkout
构建 `gd` 来安装 skill。
