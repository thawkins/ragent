# ragent Tools Reference — Index

The tools available to ragent agents, organised by category. Each category has
its own document with per-tool arguments, required flags, typical values, and
worked examples.

> **Scope:** Tool names, parameter schemas, and usage patterns (149 registered
> tools in the default build — 150 when a drivable headless browser engine is
> present, so `mf_screenshot` registers — plus the dynamic `mcp_tool`). For TUI
> workflow see `docs/howtos/tutorial.md`. For team coordination see
> `docs/howtos/teams.md`.

## Categories

| # | Document | Tools |
|---|----------|-------|
| 1 | [file-operations.md](file-operations.md) | 18 |
| 2 | [shell.md](shell.md) | 4 |
| 3 | [search.md](search.md) | 1 |
| 4 | [web.md](web.md) | 3 |
| 5 | [masterfetch.md](masterfetch.md) | 6 |
| 6 | [code-intelligence.md](code-intelligence.md) | 10 |
| 7 | [memory.md](memory.md) | 5 |
| 8 | [git.md](git.md) | 18 |
| 9 | [github.md](github.md) | 11 |
| 10 | [gitlab.md](gitlab.md) | 19 |
| 11 | [office-pdf.md](office-pdf.md) | 2 |
| 12 | [teams.md](teams.md) | 20 |
| 13 | [sub-agents.md](sub-agents.md) | 5 |
| 14 | [planning.md](planning.md) | 2 |
| 15 | [spec-management.md](spec-management.md) | 5 |
| 16 | [task-management.md](task-management.md) | 4 |
| 17 | [scheduling.md](scheduling.md) | 5 |
| 18 | [initiatives.md](initiatives.md) | 1 |
| 19 | [mcp.md](mcp.md) | 1 |
| 20 | [skills.md](skills.md) | 1 |
| 21 | [interactive.md](interactive.md) | 4 |
| 22 | [utility.md](utility.md) | 6 |

Every listed tool is advertised to the model whenever ragent's tool registry
registers it; `mf_screenshot` is the one conditional entry (a drivable headless
browser engine must be present).

All documents are also available as PDFs under `pdf/`.

## Related Documents

| Document | Covers |
|----------|--------|
| `docs/howtos/tutorial.md` | End-to-end TUI workflow tutorial |
| `docs/howtos/teams.md` | Multi-agent team coordination |
| `docs/howtos/spec.md` | Spec management and SDD workflow |
| `docs/howtos/reverse.md` | Repository reverse-engineering |
| `docs/howtos/research.md` | Research system and report synthesis |
| `docs/howtos/custom-agents.md` | Custom agent profiles and OASF schema |
