# /doctor

> Run environment diagnostics: /doctor help|check

## Overview

`/doctor` checks the environment ragent runs in and prints a diagnostic
report. The check runs asynchronously after a "Running diagnostics..." line,
so the report appears in the message window a moment later. Each row carries a
status marker: `[ok]`, `[err]`, `[warn]`, or an informational note for
optional items.

## Syntax

```
/doctor
/doctor help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| (bare) | Run the diagnostics and print the report |
| `help` | Show the subcommand table |

## Checks performed

| Check | Result marker | Notes on failure |
|-------|---------------|------------------|
| git | `[ok]` / `[err]` | `git --version` must succeed |
| ripgrep | `[ok]` / `[err]` | suggests installing ripgrep |
| GitHub token | `[ok]` / `[warn]` | suggests `/github login` |
| config | `[ok]` / `[err]` | reports loaded file count or parse error |
| storage database | `[ok]` / `[err]` | SQLite journal mode and db path |
| provider / model | `[ok]` / `[warn]` | provider name, source (env/database/auto-discovered), model, health state |
| memory directory | `[ok]` / `[err]` | `~/.ragent/memory/` |
| memory system | `[ok]` / note when disabled | blocks/structured/semantic flags |
| project `.ragent/` | `[ok]` / `[err]` | created if missing |
| project guidelines | `[ok]` / note | AGENTS.md path, or global fallback, or absent |
| MCP servers | `[ok]` / note | count of configured MCP servers (optional) |
| tool registry | `[ok]` | number of registered tools |
| code index | `[ok]` / `[wait]` / note | files/symbols/size when enabled; busy/locked marker; note when disabled |
| update status | `[ok]` / `[warn]` | latest release version; suggests `/update` |

The report ends with `*Diagnostics complete.*`

## Examples

```
From: /doctor
Running diagnostics...
```

then, a moment later:

```
# Diagnostic Report
[ok] git
[ok] ripgrep (rg)
[warn]  no GitHub token  -  run /github login
[ok] ragent config (loaded 2 config file(s))
[ok] storage database
  SQLite journal_mode=wal (/home/user/.local/share/ragent/ragent.db)
[ok] provider: Anthropic (anthropic via env)
  model=claude-sonnet-4-20250514 [ok] reachable
...
*Diagnostics complete.*
```

```
From: /doctor
(with ripgrep missing)
[err] ripgrep not found  -  install at https://github.com/BurntSushi/ripgrep
```

```
From: /doctor
(with code index busy)
[wait] code index: enabled, status busy/locked
```

```
From: /doctor help
| /doctor | Check git, ripgrep, GitHub token, config, and other environment
prerequisites, then print a diagnostic report |
| /doctor help | Show this help |
```

## Output

`Running diagnostics...` appears immediately; the full `# Diagnostic Report`
block is appended when the async check finishes. Every line starts with its
status marker so failures are easy to scan. The status bar shows
`doctor: help` for the help form.

## Related

- `/provider`  -  configure a provider when the report flags none
- `/github login`  -  add the missing GitHub token
- `/codeindex on`  -  enable the code index when disabled
- `/update`  -  install an available update flagged by the report
- `/bug-report`  -  write a file report of session state instead