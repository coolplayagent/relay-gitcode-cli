# 第二章：命令参考

`gd` 只在 GitCode 暴露等价 API 能力时采用 gh 风格命令名称。

## 核心命令

```bash
gd auth login --with-token
gd auth status --json
gd api /user --json
gd completion bash
```

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
