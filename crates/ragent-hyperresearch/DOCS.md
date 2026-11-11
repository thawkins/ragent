# ragent-hyperresearch

Deep-research engine crate (spec `hyperesearch`), a Rust port of the Python
project `jordan-gibbs/hyperresearch`: a persistent wiki-style vault with fetch
and scholarly providers, claims, a link/contradiction graph, and MCP and HTTP
surfaces. It has no LLM client; synthesis steps are skill text consumed by the
host agent.

**Status: scaffold.** The `core`, `models` and `cli` modules carry the
implemented types below; `export`, `graph`, `indexgen`, `mcp`, `scholar`,
`search`, `serve`, `skills` and `web` are declared placeholder modules. The
`hyperresearch` binary currently implements only `help`. No other workspace
crate depends on this one.

## Workspace Dependencies

- ragent-config
- ragent-types

## External Dependencies

- anyhow, clap, serde, serde_json, serde_norway, thiserror

## Features

None declared.

## Binaries

- **hyperresearch** (`src/main.rs`) — CLI entry point; parses `cli::args::Cli` and calls `cli::run`.

## Public API (crate root)

- **Error**, **Result** — re-exported from `core::error`.

### Module: core

- **config** — `Config`; environment keys `ENV_VAULT_ROOT` (`HYPERRESEARCH_VAULT`), `ENV_PROVIDERS`, `ENV_TIMEOUT_SECS`, `ENV_MAX_BODY_BYTES`, `ENV_PROFILE`; defaults `DEFAULT_TIMEOUT_SECS` (30), `DEFAULT_MAX_BODY_BYTES` (8 MiB).
- **error** — `Error` enum.
- **migrations** — Vault schema versioning: `current_version`, `read_version`, `apply_pending`, `MIGRATIONS_FILE`, `MIGRATIONS`.
- **untrusted** — Fencing of fetched content: `Untrusted`, `FENCE_OPEN`, `FENCE_CLOSE`.
- **vault** — `Vault` (`open`, `root`, `note_path`, `note_assets_dir`, `ensure_note_assets_dir`, `index_path`, `ensure_available`); `ASSETS_DIR`, `NOTE_ASSETS_DIR`.

### Module: models

Serde data types, re-exported from `models`: `Note`, `NoteMeta`, `Claim`,
`VerificationState`, `RunRecord`, `RunStatus`, `ScholarlyRecord`,
`StudyDesign`, `Resolution`. `note::FRONTMATTER_FENCE` marks note front matter.

### Module: cli

- **run** (fn) — Dispatch a parsed `args::Cli`.
- **args** — `Cli`, `Command`, `USAGE`.
- **output** — `Format`, `emit_json`.

_Maintained alongside `crates/ragent-hyperresearch/src/lib.rs` and `crates/ragent-hyperresearch/Cargo.toml`; update when those change._
