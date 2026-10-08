# ragent Tools Reference — Index

The tools available to ragent agents, organised by category. Each category has
its own document with per-tool arguments, required flags, typical values, and
worked examples.

> **Scope:** Tool names, parameter schemas, and usage patterns (151 registered
> tools plus the dynamic `mcp_tool`). For TUI workflow see
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
| 5 | [masterfetch.md](masterfetch.md) | 6 | `masterfetch` |
| 6 | [code-intelligence.md](code-intelligence.md) | 10 | `codeindex` |
| 7 | [memory.md](memory.md) | 5 | always on |
| 8 | [git.md](git.md) | 18 | always on |
| 9 | [github.md](github.md) | 11 | `github` |
| 10 | [gitlab.md](gitlab.md) | 19 | `gitlab` |
| 11 | [office-pdf.md](office-pdf.md) | 2 | always on |
| 12 | [teams.md](teams.md) | 20 | `teams` |
| 13 | [sub-agents.md](sub-agents.md) | 5 | `agents` |
| 14 | [planning.md](planning.md) | 2 | `plan` |
| 15 | [spec-management.md](spec-management.md) | 5 | always on |
| 16 | [task-management.md](task-management.md) | 4 | always on |
| 17 | [scheduling.md](scheduling.md) | 5 | always on |
| 18 | [initiatives.md](initiatives.md) | 1 | always on |
| 19 | [mcp.md](mcp.md) | 1 | always on |
| 20 | [skills.md](skills.md) | 1 | always on |
| 21 | [interactive.md](interactive.md) | 4 | always on |
| 22 | [utility.md](utility.md) | 5 | always on |
| 23 | [communications.md](communications.md) | 2 | always on |

Switches default `off` for `github`, `gitlab`, `teams`, `agents`, `plan`;
the rest default `on`. See `docs/howtos/tool-visibility.md`.

All documents are also available as PDFs under `pdf/`.

## Related Documents

| Document | Covers |
|----------|--------|
| `docs/howtos/tutorial.md` | End-to-end TUI workflow tutorial |
| `docs/howtos/tool-visibility.md` | Hiding and exposing tool families |
| `docs/howtos/teams.md` | Multi-agent team coordination |
| `docs/howtos/communications.md` | Gmail and messaging channel tools |
| `docs/howtos/spec.md` | Spec management and SDD workflow |
| `docs/howtos/reverse.md` | Repository reverse-engineering |
| `docs/howtos/research.md` | Research system and report synthesis |
| `docs/howtos/custom-agents.md` | Custom agent profiles and OASF schema |
