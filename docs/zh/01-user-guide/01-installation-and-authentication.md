# 第一章：安装与认证

## 从源码构建

```bash
cargo build
target/debug/gd --version
```

构建优化后的二进制：

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
`GITCODE_TOKEN` 优先级更高，并且不会把凭证写入磁盘。

## API Host

默认 API base：

```text
https://api.gitcode.com/api/v5
```

测试兼容 host 时，可通过 `--api-base` 或 `GITCODE_API_BASE` 覆盖。
