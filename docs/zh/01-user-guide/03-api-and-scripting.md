# 第三章：API 与脚本化

当 GitCode 端点还没有一等命令时，使用 `gd api`。

## 基础请求

```bash
gd api /users/dengmengmian --json
gd api /user --json
```

相对 endpoint 会自动拼接到 `/api/v5` 下。

## 方法与字段

```bash
gd api /user/repos -X POST -F name=demo -F private=true --json
gd api /repos/owner/repo/issues -f state=open --json
```

- `-f, --raw-field` 发送字符串字段。
- `-F, --field` 解析布尔值、数字和 `null`。
- `--input <file>` 发送 JSON 文件或原始字符串请求体。

## 输出

脚本中使用 `--json`。文本输出会摘要常见字段，例如 `html_url`、
`full_name`、`name`、`title`、`number` 和 `state`。
