# 第一章：运行时与认证

## 分层

- `src/cli.rs` 定义 clap 命令树。
- `src/commands/mod.rs` 把解析后的命令映射到 GitCode API 调用。
- `src/client.rs` 负责 HTTP 请求构造、认证 header、分页、JSON 解码和 API 错误映射。
- `src/http.rs` 负责共享 reqwest 客户端策略，包括异步传输、代理复用和 TLS 校验默认值。
- `src/pipeline.rs` 负责 OpenLibing gateway endpoint 构造、OAuth callback
  解析、请求认证和流水线门禁 API 错误映射。
- `src/auth.rs` 负责环境变量和 keyring token 查找。
- `src/config.rs` 负责非敏感 host 和 API base 配置。

## 认证流程

`gd` 先读取 `GD_TOKEN`，再读取 `GITCODE_TOKEN`。如果都未设置，再读取系统
keyring。认证 HTTP 请求使用：

```text
Authorization: Bearer <token>
```

这与 GitCode API v5 文档一致，并避免默认把 token 放到 query string。

流水线门禁命令使用 OpenLibing 凭据，而不是 `GITCODE_TOKEN`。`gd` 会优先读取
`GD_OPENLIBING_TOKEN`、`GD_OPENLIBING_COOKIE` 和
`GD_OPENLIBING_CSRF_TOKEN`；否则 `gd pipeline auth login` 会把 OpenLibing
凭据材料保存到独立 keyring entry。OpenLibing 请求会把可用的 bearer、cookie
和 CSRF header 发送到配置的 gateway。
仓库门禁配置同样只面向 OpenLibing：`gd pipeline setup` 会把 GitCode 仓库
URL、PR 接管开关、门禁自动触发开关、CodeCheck 规则集选择以及可选的公共账号
token 材料发送给 OpenLibing。通过 `--public-token-env` 读取到的 token 只会
进入 OpenLibing 请求体，并会从命令输出中脱敏。
OpenLibing 仍会在服务端校验仓库维护权限。仓库新增或更新返回 `403` 时，
表示当前账号需要项目管理员或等价项目审批人员权限，浏览器自动化不能改变该
授权结果。

## 运行时与网络策略

命令执行使用 Tokio 处理 HTTP、文件输入、stdin 读取和 git 子进程。keyring
这类不可避免的阻塞调用通过 Tokio blocking bridge 隔离。

共享 reqwest 客户端保持 reqwest 系统代理行为，支持
`HTTP_PROXY`/`http_proxy`、`HTTPS_PROXY`/`https_proxy`、
`ALL_PROXY`/`all_proxy` 和 `NO_PROXY`/`no_proxy`。GitCode API 调用默认不校验
TLS 证书。`GD_SSL_VERIFY`/`gd_ssl_verify`、
`GITCODE_SSL_VERIFY`/`gitcode_ssl_verify` 和 `SSL_VERIFY`/`ssl_verify` 可启用或
禁用证书校验，任意非空 `GIT_SSL_NO_VERIFY`/`git_ssl_no_verify` 值会作为 Git 风格的禁用开关受支持。

## 命令边界

一等命令只覆盖 GitCode API 等价能力。`gd api` 是较新或低频 endpoint 的出口，
避免过早扩大公开命令树。
