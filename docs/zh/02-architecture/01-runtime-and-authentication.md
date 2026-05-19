# 第一章：运行时与认证

## 分层

- `src/cli.rs` 定义 clap 命令树。
- `src/commands/mod.rs` 把解析后的命令映射到 GitCode API 调用。
- `src/client.rs` 负责 HTTP 请求构造、认证 header、分页、JSON 解码和 API 错误映射。
- `src/auth.rs` 负责环境变量和 keyring token 查找。
- `src/config.rs` 负责非敏感 host 和 API base 配置。

## 认证流程

`gd` 先读取 `GITCODE_TOKEN`。如果未设置，再读取系统 keyring。认证 HTTP
请求使用：

```text
Authorization: Bearer <token>
```

这与 GitCode API v5 文档一致，并避免默认把 token 放到 query string。

## 命令边界

一等命令只覆盖 GitCode API 等价能力。`gd api` 是较新或低频 endpoint 的出口，
避免过早扩大公开命令树。
