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

## GitCode 流水线命令

流水线命令调用 CodeArts Pipeline 的 GitCode API。将
`GITCODE_PIPELINE_API_BASE` 设置为区域 endpoint，例如
`https://devcloud.ap-southeast-3.myhuaweicloud.com`，并将
`GITCODE_PIPELINE_DOMAIN_ID` 设置为租户 domain ID。推荐使用
`HUAWEICLOUD_SDK_AK`/`HUAWEICLOUD_SDK_SK` 或
`CLOUD_SDK_AK`/`CLOUD_SDK_SK` 进行 AK/SK 签名；未配置 AK/SK 时，
`gd` 会回退使用已保存的 GitCode token 作为 Bearer token。

```bash
gd pipeline register --repo owner/repo --type create --new-file-path .gitcode/workflows/ci.yml --file workflow.yml
gd pipeline run --repo owner/repo --file-path .gitcode/workflows/ci.yml --branch main
gd pipeline runs --repo owner/repo --pipeline-name ci --status success
gd pipeline view pipeline-id pipeline-run-id
gd pipeline log pipeline-id pipeline-run-id job-run-id
gd pipeline stop pipeline-id pipeline-run-id
gd pipeline retry pipeline-id pipeline-run-id
```

可用 `--pipeline-api-base` 和 `--pipeline-domain-id` 为单次命令覆盖环境变量。
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
