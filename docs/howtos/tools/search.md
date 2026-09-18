# Tools — Search

Text and pattern search across files. For code-symbol queries (functions,
types, imports) use the [Code Intelligence](code-intelligence.md) tools
instead — the codeindex is faster and returns structured results.

| Tool | Description |
|------|-------------|
| `grep` | Search file contents for a regex pattern (ripgrep). |

**Use cases:** finding TODO/FIXME comments, matching text in config files and
markdown, locating log fragments.

**System instruction:** "Use `grep` for arbitrary text and pattern matching.
Use codeindex tools for code symbol queries."

---

## grep

Search file contents for a Rust-syntax regex pattern using ripgrep, respecting
`.gitignore` rules. Binary files are skipped automatically.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `pattern` | string | yes | Regex pattern (Rust regex syntax) | `"TODO"`, `"fn main"` |
| `path` | string | no | Directory or file to search (default: working directory) | `"src"` |
| `include` | string | no | Glob restricting which files are searched | `"*.rs"`, `"**/*.ts"` |
| `exclude` | string | no | Glob of files/directories to exclude | `"vendor/**"` |
| `case_insensitive` | boolean | no | Case-insensitive matching | `false` |
| `multiline` | boolean | no | `^`/`$` match line boundaries across lines | `false` |
| `max_results` | integer | no | Maximum matches returned (max 500) | `500` |

**Example:**
```text
grep pattern="TODO" path="src" include="*.rs"
grep pattern="unsafe code" case_insensitive=true
```
