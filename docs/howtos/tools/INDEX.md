# ragent Tools Reference — Index

The tools available to ragent agents, organised by category. Each category has
its own document with per-tool arguments, required flags, typical values, and
worked examples.

> **Scope:** Tool names, parameter schemas, and usage patterns (168 statically
> registered tools plus the dynamic `mcp_tool`). For TUI workflow see
> `docs/howtos/tutorial.md`. For hiding/exposing tool families see
> `docs/howtos/tool-visibility.md`. For team coordination see
> `docs/howtos/teams.md`.

## Categories

| # | Document | Tools | Visibility Switch |
|---|----------|-------|-------------------|
| 1 | [file-operations.md](file-operations.md) | 18 | always on |
| 2 | [shell.md](shell.md) | 4 | always on |
| 3 | [search.md](search.md) | 1 | always on |
| 4 | [web.md](web.md) | 3 | always on |
| 5 | [browser.md](browser.md) | 1 | `browser` |
| 6 | [masterfetch.md](masterfetch.md) | 6 | `masterfetch` |
| 7 | [code-intelligence.md](code-intelligence.md) | 10 | `codeindex` |
| 8 | [memory.md](memory.md) | 5 | always on |
| 9 | [git.md](git.md) | 18 | always on |
| 10 | [github.md](github.md) | 11 | `github` |
| 11 | [gitlab.md](gitlab.md) | 19 | `gitlab` |
| 12 | [office-pdf.md](office-pdf.md) | 8 | `office` |
| 13 | [teams.md](teams.md) | 20 | `teams` |
| 14 | [sub-agents.md](sub-agents.md) | 5 | `agents` |
| 15 | [planning.md](planning.md) | 2 | `plan` |
| 16 | [spec-management.md](spec-management.md) | 5 | always on |
| 17 | [task-management.md](task-management.md) | 4 | always on |
| 18 | [scheduling.md](scheduling.md) | 5 | always on |
| 19 | [initiatives.md](initiatives.md) | 1 | always on |
| 20 | [mcp.md](mcp.md) | 1 | always on |
| 21 | [skills.md](skills.md) | 1 | always on |
| 22 | [interactive.md](interactive.md) | 4 | always on |
| 23 | [utility.md](utility.md) | 1 | always on |
| 24 | [finance.md](finance.md) | 8 | `finance` |
| 25 | [communications.md](communications.md) | 2 | always on |
| 26 | [plot.md](plot.md) | 6 | always on |

Switches default `off` for `github`, `gitlab`, `teams`, `agents`, `plan`,
`office`; the rest default `on`. See `docs/howtos/tool-visibility.md`.

All documents are also available as PDFs under `pdf/`.

## Related Documents

| Document | Covers |
|----------|--------|
| `docs/howtos/tutorial.md` | End-to-end TUI workflow tutorial |
| `docs/howtos/tool-visibility.md` | Hiding and exposing tool families |
| `docs/howtos/teams.md` | Multi-agent team coordination |
| `docs/howtos/communications.md` | Gmail and messaging channel tools |
| `docs/howtos/finance.md` | Stock and currency tools |
| `docs/howtos/spec.md` | Spec management and SDD workflow |
| `docs/howtos/reverse.md` | Repository reverse-engineering |
| `docs/howtos/research.md` | Research system and report synthesis |
| `docs/howtos/custom-agents.md` | Custom agent profiles and OASF schema |
