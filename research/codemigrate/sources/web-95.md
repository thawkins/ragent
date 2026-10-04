# Web source

- URL: https://gist.github.com/skorotkiewicz/c9c0b9ce66087bf81ac78e476ecb3cad
- Title: shredder code (hy3 model) | https://github.com/skorotkiewicz/Skills
- Author(s): 262588213843476
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:36:02.104416810+00:00
- Relevance: Medium - multiple title terms match query


```text
The gist defines a Pi “shredder” skill, invoked via `/skill:shredder` or `/shredder`, that is more aggressive than refactoring: it treats the current codebase as legacy behavior documentation and rebuilds it from scratch while preserving intended product behavior, public APIs, tests, build process, and user-facing features unless breaking changes are explicitly requested. It outlines phases for mapping behavior, deciding what to preserve or change, designing a clean replacement, rewriting code, updating tests/docs/scripts/config, and producing `SHREDDER_REPORT.md`; before destructive edits it requires preserving state via a Git branch like `shredder/rewrite` or a `_shredder_backup/` directory, and it recommends running commands such as `npm test`, `npm run lint`, `npm run build`, and `npm start`. It also lists modes including `keep-api`, `preserve-ui`, `modernize`, `simplify`, `max-clean`, and `migrate-to` (e.g., TypeScript, Next.js, FastAPI, Rust), with completion criteria requiring a preserved original state, rewritten implementation, updated tests or test plan, updated run/build instructions, updated dependencies, a completed `SHREDDER_REPORT.md`, and a final user summary.
```
