# 第一章：运行时与认证

## 分层

- `src/cli.rs` 定义 clap 命令树。
- `src/commands/mod.rs` 把解析后的命令映射到 GitCode API 调用。
- `src/client.rs` 负责 HTTP 请求构造、认证 header、分页、JSON 解码和 API 错误映射。
- `src/http.rs` 负责共享 reqwest 客户端策略，包括异步传输、代理复用和 TLS 校验默认值。
- `src/pipeline.rs` 负责 GitCode Actions endpoint 构造、请求体组织、
  workflow 文件辅助逻辑和流水线 API 错误映射。
- `src/auth.rs` 负责环境变量和 keyring token 查找。
- `src/config.rs` 负责非敏感 host 和 API base 配置。

## 认证流程

`gd` 先读取 `GITCODE_TOKEN`。如果未设置，再读取系统 keyring。认证 HTTP
请求使用：

```text
Authorization: Bearer <token>
```

这与 GitCode API v5 文档一致，并避免默认把 token 放到 query string。

流水线命令使用同一套 Bearer token 流程。workflow 文件创建和更新走 GitCode
API v5 的仓库 contents endpoint。运行列表、手动触发、运行详情、日志读取、
停止、重试和重新运行走配置 hostname 下的 GitCode Actions endpoint。

## 运行时与网络策略

命令执行使用 Tokio 处理 HTTP、文件输入、stdin 读取和 git 子进程。keyring
这类不可避免的阻塞调用通过 Tokio blocking bridge 隔离。

共享 reqwest 客户端保持 reqwest 系统代理行为，支持
`HTTP_PROXY`/`http_proxy`、`HTTPS_PROXY`/`https_proxy`、
`ALL_PROXY`/`all_proxy` 和 `NO_PROXY`/`no_proxy`。GitCode API 调用默认不校验
TLS 证书。

## 命令边界

一等命令只覆盖 GitCode API 等价能力。`gd api` 是较新或低频 endpoint 的出口，
避免过早扩大公开命令树。
