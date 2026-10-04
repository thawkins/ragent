# Web source

- URL: https://github.com/gety-ai/gety-codex-connector
- Title: GitHub - gety-ai/gety-codex-connector: Indexes local Codex session JSONL logs as searchable markdown documents.
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:59.081339862+00:00
- Relevance: High - title + snippet match query


```text
The Gety custom connector (`github.com/gety-ai/gety-codex-connector`) indexes local Codex session JSONL logs as searchable markdown, creating one `codex:session` document per JSONL file; it uses the session summary or first user prompt as title, adds workspace roots/model when available, indexes user messages and Codex final answers, skips prompt-only sessions, renders Codex as Codex, and never indexes tool outputs, with optional tool-call summaries off by default and archived sessions on by default. It resolves Codex home from `CODEX_HOME` or platform defaults (Windows `%USERPROFILE%\.codex`, then `%HOMEDRIVE%%HOMEPATH%\.codex`; Linux/macOS `$HOME/.codex`) and scans `sessions/` plus optionally `archived_sessions/`. The first poll parses all candidate logs; later polls parse only files with changed mtime/size and use lightweight fingerprints in connector state to update affected session docs and remove disappeared ones. The repo includes `manifest.json`, `assets/codex-color.svg`, `src/index.ts`, `src/codex_log.ts`, committed `dist/main.js` and map, a local SDK shim, and generated `src/gen/manifest.d.ts`; tasks include `deno task verify`, `build`, `test`, and `runner -- --reset-state` with `dev/runner.ts` writing to `dev/runs/<timestamp>/` and persistent state at `dev/.runner/state.json`. Config env vars are `GETY_CONFIG_INCLUDE_TOOL_CALLS`/`INCLUDE_TOOL_CALLS` and `GETY_CONFIG_INCLUDE_ARCHIVED_SESSIONS`/`INCLUDE_ARCHIVED_SESSIONS`; installation is via Gety’s Custom Connectors settings from local folder.
```
