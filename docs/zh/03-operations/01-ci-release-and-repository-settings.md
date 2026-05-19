# 第一章：CI、发布与仓库设置

## 本地门禁

```bash
./check.sh
```

该门禁会运行格式检查、clippy warnings denied、测试和构建。

## GitHub Actions

- `PR Checks`：format、clippy、test、build。
- `Qodana`：Rust 静态分析。
- `Release`：版本校验、package dry run、多平台二进制构建、归档上传和
  GitHub Release 创建。

## GitCode 流水线

GitCode workflow 位于 `.gitcode/workflows`。Rust 流水线在 `repo_workspace`
内执行与 GitHub Actions 相同的核心门禁。

## 仓库设置

分支保护、必需检查和 secrets 设置见 [Repository Settings](../../repository-settings.md)。
