# ragent Tools Reference — Index

The tools available to ragent agents, organised by category. Each category has
its own document with per-tool arguments, required flags, typical values, and
worked examples.

> **Scope:** Tool names, parameter schemas, and usage patterns for every
> registered tool plus the dynamic `mcp_tool`. This is the canonical place for the
> tool count: the default build registers 152 tools at v1.0.169, and
> `ragent --dry-run --json run x` reports the live figure (the `total` item of its
> `tools` section, followed by one item per permission category). `mf_screenshot`
> adds one when a drivable headless browser engine is present. For TUI
> workflow see `docs/howtos/tutorial.md`. For team coordination see
> `docs/howtos/teams.md`.

## Categories

| # | Document |
|---|----------|
| 1 | [file-operations.md](file-operations.md) |
| 2 | [shell.md](shell.md) |
| 3 | [search.md](search.md) |
| 4 | [web.md](web.md) |
| 5 | [masterfetch.md](masterfetch.md) |
| 6 | [code-intelligence.md](code-intelligence.md) |
| 7 | [memory.md](memory.md) |
| 8 | [git.md](git.md) |
| 9 | [github.md](github.md) |
| 10 | [gitlab.md](gitlab.md) |
| 11 | [office-pdf.md](office-pdf.md) |
| 12 | [teams.md](teams.md) |
| 13 | [sub-agents.md](sub-agents.md) |
| 14 | [planning.md](planning.md) |
| 15 | [spec-management.md](spec-management.md) |
| 16 | [task-management.md](task-management.md) |
| 17 | [scheduling.md](scheduling.md) |
| 18 | [initiatives.md](initiatives.md) |
| 19 | [mcp.md](mcp.md) |
| 20 | [skills.md](skills.md) |
| 21 | [interactive.md](interactive.md) |
| 22 | [utility.md](utility.md) |

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
