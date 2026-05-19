# 第一章：运行时与认证

## 分层

- `src/cli.rs` 定义 clap 命令树。
- `src/commands/mod.rs` 把解析后的命令映射到 GitCode API 调用。
- `src/client.rs` 负责 HTTP 请求构造、认证 header、分页、JSON 解码和 API 错误映射。
- `src/pipeline.rs` 负责 CodeArts Pipeline endpoint 构造、请求体组织、AK/SK
  请求签名和流水线 API 错误映射。
- `src/auth.rs` 负责环境变量和 keyring token 查找。
- `src/config.rs` 负责非敏感 host 和 API base 配置。

## 认证流程

`gd` 先读取 `GITCODE_TOKEN`。如果未设置，再读取系统 keyring。认证 HTTP
请求使用：

```text
Authorization: Bearer <token>
```

这与 GitCode API v5 文档一致，并避免默认把 token 放到 query string。

流水线命令使用 `GITCODE_PIPELINE_API_BASE` 和
`GITCODE_PIPELINE_DOMAIN_ID` 指定 CodeArts Pipeline host 与租户范围。它们优先通过
`HUAWEICLOUD_SDK_AK` 和 `HUAWEICLOUD_SDK_SK` 进行 AK/SK 签名，也支持
`CLOUD_SDK_AK` 和 `CLOUD_SDK_SK` 作为别名。未配置 AK/SK 时，会使用保存的
GitCode token 作为 Bearer token。

## 命令边界

一等命令只覆盖 GitCode API 等价能力。`gd api` 是较新或低频 endpoint 的出口，
避免过早扩大公开命令树。
