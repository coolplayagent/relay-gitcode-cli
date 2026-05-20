# 第一章：安装与认证

## 从发布版本安装

Rust 用户可以用发布到 crates.io 的包安装或升级 CLI：

```bash
cargo install relay-gitcode-cli --force
gd --version
gd version check --json
```

也可以从 GitHub Releases 下载对应平台归档，并把 `gd` 二进制放到
`PATH` 中。`gd version check` 会从 GitHub Releases 和 crates.io
报告可用稳定版本；它不会替换当前二进制。

## 从源码构建

本地开发时可构建优化后的二进制：

```bash
./build.sh
target/release/gd --version
```

## 认证

从标准输入传入 GitCode 个人访问令牌：

```bash
printf '%s' "$GITCODE_TOKEN" | gd auth login --with-token
gd auth status
```

`gd` 会把登录 token 保存到系统 keyring。CI 和临时端到端测试中，
`GD_TOKEN` 和 `GITCODE_TOKEN` 都能避免把凭证写入磁盘；两者同时存在时
`GD_TOKEN` 优先级更高。

## API Host

默认 API base：

```text
https://api.gitcode.com/api/v5
```

测试兼容 host 时，可通过 `--api-base`、`GD_API_BASE` 或
`GITCODE_API_BASE` 覆盖。优先级依次为 CLI flag、`GD_API_BASE`、
`GITCODE_API_BASE`。

## 网络

`gd` 使用 reqwest 提供的系统代理行为，支持
`HTTP_PROXY`/`http_proxy`、`HTTPS_PROXY`/`https_proxy`、
`ALL_PROXY`/`all_proxy` 和 `NO_PROXY`/`no_proxy`。

GitCode API 调用默认不校验 TLS 证书。设置 `GD_SSL_VERIFY`/`gd_ssl_verify`、
`GITCODE_SSL_VERIFY`/`gitcode_ssl_verify` 或 `SSL_VERIFY`/`ssl_verify` 为
`true` 可启用证书校验；`GIT_SSL_NO_VERIFY`/`git_ssl_no_verify` 为任意非空值时会保持禁用校验。
