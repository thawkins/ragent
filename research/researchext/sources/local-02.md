# Local source (in-project)

- Path: CHANGELOG.md
- Relevance: 500 match(es) on: ge, on, or, …(+29) — "# Changelog"
- Captured (UTC): 2026-06-30T09:43:21.914642588+00:00

```text
Excerpt — 500 keyword match(es)

▶    1:    1  # Changelog
     2:    2  
▶    3:    3  ## Version: 0.1.0-alpha.125
     4:    4  
▶    5:    5  ### Changed
▶    6:    6  - **Workspace version** — Bumped to `0.1.0-alpha.125`.
     7:    7  
▶    8:    8  ## Version: 0.1.0-alpha.124
     9:    9  
▶   10:   10  ### Changed
▶   11:   11  - **Workspace version** — Bumped to `0.1.0-alpha.124`.
    12:   12  
▶   13:   13  ## Version: 0.1.0-alpha.122
    14:   14  
▶   15:   15  ### Fixed — `/help` and `/skills` slash output no longer collapses to a single paragraph
▶   16:   16  - **`/help` table preserves per-line layout in the TUI** — The `/help` slash
▶   17:   17    command now wraps its command/skill listing in a bare fenced code block
▶   18:   18    (` ```\n … \n``` `) so the markdown → HTML → text pipeline does not reflow
▶   19:   19    every row into one paragraph. Each command/skill stays on its own line and
▶   20:   20    column alignment is preserved instead of being mangled.
▶   21:   21  - **`/skills` table preserves per-line layout in the TUI** — Same fix
▶   22:   22    applied to the `/skills` listing: the registered-skills table is wrapped in
▶   23:   23    a bare fenced code block, and `try_extract_research_code_block` now detects
▶   24:   24    the block via the generic `From: /<cmd>` prefix (not just `From: /research`)
▶   25:   25    so `/skills` benefits from the same verbatim rendering path.
▶   26:   26  - **`try_extract_research_code_block` generalised to any `From: /<cmd>`
▶   27:   27    response** — Only **bare** fences (a line containing exactly three
▶   28:   28    backticks followed by a newline) are recognised. Responses that use
▶   29:   29    language-tagged fences (e.g. `/tools show` emits multiple ` ```text `
▶   30:   30    blocks) are not intercepted and continue to flow through the normal

… (470 more match(es) elided)

```
