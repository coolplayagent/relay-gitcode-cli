# Chapter 3: API and Scripting

Use `gd api` when a GitCode endpoint is not yet represented by a first-class
command.

## Basic Requests

```bash
gd api /users/dengmengmian --json
gd api /user --json
```

`gd` automatically joins relative endpoints under `/api/v5`.

## Methods and Fields

```bash
gd api /user/repos -X POST -F name=demo -F private=true --json
gd api /repos/owner/repo/issues -f state=open --json
```

- `-f, --raw-field` sends string fields.
- `-F, --field` parses booleans, numbers, and `null`.
- `--input <file>` sends a JSON file or raw string body.

## Output

Use `--json` for scripts. Human text output summarizes common fields such as
`html_url`, `full_name`, `name`, `title`, `number`, and `state`.
