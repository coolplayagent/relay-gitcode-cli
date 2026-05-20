# 第一章：CI、发布与仓库设置

## 本地门禁

```bash
./check.sh
```

该门禁会运行格式检查、clippy warnings denied、测试和构建。

## GitHub Actions

- `PR Checks`：format、clippy、test、build。
- `Qodana`：Rust 静态分析。
- `Release`：版本校验、package dry run、多平台二进制构建、crate 发布、
  归档上传、CLI skill 打包、checksum 生成、GitHub Release 创建，以及可选的
  GitCode Release 同步。

GitHub Release 包含从 `skills/relay-gitcode-cli` 构建的
`relay-gitcode-cli-skill-<tag>.tar.gz`，版本跟随 `Cargo.toml`。release
workflow 会在打包前把 Linux x64 和 Windows x64 的 `gd` 二进制注入 skill
的 `assets` 目录。配置 `CLAWHUB_TOKEN` 后，release workflow 还会用
`clawhub skill publish` 把打包后的 skill bundle 发布到 ClawHub，因此
ClawHub package 会包含与 GitHub Release 归档相同的内置 assets。该 skill
应从发布产物或 ClawHub package 安装，不应从本地 checkout 安装。

GitHub 会把 secret 名称按大写保存，因此在仓库设置中创建的 `clawhub_token`
会在 workflow 中作为 `CLAWHUB_TOKEN` 读取。配置 `GITCODE_TOKEN` 后，release
workflow 会在 GitHub Release 发布后运行 `gd release migrate-github`。该 token
需要具备在 `plm-cac/relay-gitcode-cli` 创建或更新 Release、上传 Release
附件的权限。未配置 `GITCODE_TOKEN` 时会跳过 GitCode Release 同步。

## GitCode 流水线

GitCode workflow 位于 `.gitcode/workflows`。Rust 流水线在 `repo_workspace`
内执行与 GitHub Actions 相同的核心门禁。

CodeCheck workflow 应先在 GitCode 项目中配置 `CODECHECK_ACCESS_TOKEN` 等
secret，再生成 workflow，避免把 token 值写进仓库。pull request 运行会检查
源仓库和源分支，push 运行会检查配置的仓库 URL 和当前 ref：

```bash
gd pipeline codecheck --repo owner/repo --language SHELL --access-token-secret CODECHECK_ACCESS_TOKEN
```

## 仓库设置

分支保护、必需检查和 secrets 设置见 [Repository Settings](../../repository-settings.md)。
