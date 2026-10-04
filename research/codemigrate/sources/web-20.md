# Web source

- URL: https://github.com/jafreck/AAMF
- Title: GitHub - jafreck/AAMF: Autonomous Agent Migration Framework - Migrate legacy code bases
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:25:52.264018942+00:00
- Relevance: Medium - multiple title terms match query


```text
AAMF (Autonomous Agent Migration Framework) is a “legacy code base deleter” that translates any-size codebases into another language via purpose-built AI agents iteratively migrating a deterministically computed DAG of tasks. It treats migration as a 9-phase pipeline (0–8) defined in Cadre’s flow DSL: Phase 0/1 use @jafreck/lore to build a SQLite knowledge base/call graph and derive bounded MigrationTask[] (SCC contraction, weighted merge under maxLinesPerTask), while Phases 2–8 use 13 specialized scenarios—knowledge-builder, migration-planner/adjudicator, code-migrator/parity-verifier/test-writer/parity-failure-resolver, final-parity-checker, e2e-test-crafter/documentation-writer, optional idiomatic reviewer/planner/refactorer, and completion. The framework supports Copilot (default) and Claude Code runtimes, per-task/wave-barrier/sync-epoch scheduling, deterministic build/test/parity gates, checkpointed --resume in .aamf/migration/{projectName}/state/checkpoint.json, MCP KB access, tokenBudget thresholds at 80%/100%, CostEstimator for 48 models (cached tokens at 50% input rate), git auto-commit, and observability metrics/reports. It requires Node.js 22+ and authenticated Copilot CLI or Claude Code on PATH; an example port is the lz4 compression library from C to Rust with claude-sonnet-4.6, and the project is MIT licensed.
```
