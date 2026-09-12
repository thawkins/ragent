# /codeindex

> Manage codebase index: /codeindex on|off|show|lang|reindex|rebuild|graph <build|export|lang>|explain <symbol>|path <A> <B>|communities|godnodes|help

## Overview

`/codeindex` controls the codebase indexing subsystem: tree-sitter parsing into
a SQLite store, Tantivy full-text search, the file watcher, and the semantic
code graph (FR-015). Enablement persists to `.ragent/ragent.json`; the index
itself lives under `.ragent/codeindex`. Most report subcommands are read-only
and never trigger a scan; only `reindex` and `rebuild` do background work.
Graph queries require a graph dataset, which is built on demand with
`/codeindex graph build`.

## Syntax

```
/codeindex on | enable
/codeindex off | disable
/codeindex show | status
/codeindex reindex
/codeindex lang | languages
/codeindex rebuild
/codeindex graph build
/codeindex graph lang <language>
/codeindex godnodes [N]
/codeindex explain <symbol>
/codeindex path <symbolA> <symbolB>
/codeindex communities
/codeindex help
```

Notes:

- The bare form is equivalent to `/codeindex show`.
- `/codeindex graph export` appears in the help/usage line but is NOT
  implemented in the dispatcher - there is no export arm. Only `build` and
  `lang` work under `graph`.
- `/codeindex skillgen` also exists in the code but is unadvertised; it is not
  part of the supported surface below.

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `/codeindex on` (alias `enable`) | Enable the code index, persist the setting, open/create the index at `.ragent/codeindex`, and start the file watcher (which performs an initial full reindex). |
| `/codeindex off` (alias `disable`) | Stop the watcher and drop the in-memory index and stats cache. |
| `/codeindex show` (aliases `status`, bare) | Print the full status report (see Output). Non-blocking: never waits on the store lock. |
| `/codeindex reindex` | Kick a background re-scan of the codebase. Requires an active index. |
| `/codeindex lang` (alias `languages`) | List supported scanner languages, 5 per line, with the total count. |
| `/codeindex rebuild` | Rebuild the full-text search index from the SQLite store. |
| `/codeindex graph build` | Build the semantic code graph in the background. |
| `/codeindex graph lang <language>` | Show the graph-related language coverage for the code index. |
| `/codeindex godnodes [N]` | Print the N most-connected symbols (default 10, clamped 1-100) as a table. |
| `/codeindex explain <symbol>` | Print node metadata plus incoming and outgoing graph edges for the symbol. |
| `/codeindex path <A> <B>` | Print the shortest symbol-to-symbol path as a hop chain. |
| `/codeindex communities` | Run/print community detection over the graph. |
| `/codeindex help` | Print the help page. |

## Examples

Enable indexing and let the watcher perform the initial scan:

```
/codeindex on
```

Check the index, FTS, and graph state:

```
/codeindex show
```

Refresh the index after a bulk import:

```
/codeindex reindex
```

Find which symbols sit at the centre of the dependency web (top 20 instead of
the default 10):

```
/codeindex godnodes 20
```

Trace the shortest route from an entry point to a helper:

```
/codeindex path main process_message
```

See how the codebase clusters, and which languages the scanner supports:

```
/codeindex communities
/codeindex lang
```

## Output

### on | enable

- `[ok] Code index: enabled and activated. Background reindex in progress.`
- If the config save fails: `[warn] enabled in memory, but saving config
  failed` plus status `codeindex: on (unsaved)`.

### off | disable

- When it was active: `disabled and deactivated. Codeindex tools will no
  longer be available. Use /codeindex on and restart to re-enable.`
- When it was not active: `disabled. It was not currently active.`

### show | status | bare

Prints, per the non-blocking `try_status()` snapshot:

- `Enabled`, `Files indexed`, `Total symbols`, `FTS documents`, `References`,
  `Total size KB`.
- FTS warnings: when the FTS document count is empty, run `rebuild`; when FTS
  documents are below 50 percent of the symbol count, the FTS index may be out
  of sync.
- `Languages` with per-language symbol counts.
- `Last full index` and `Last incremental` timestamps, plus `Index size`.
- `Graph Dataset` section: while building, progress as `done/total files`;
  when edges are 0, `not built. Run /codeindex graph build`; otherwise `Total
  edges`, `Nodes`, edges by confidence (`EXTRACTED`/`INFERRED`),
  `Communities`, and edges by kind (`calls`, `imports`, `inherits`,
  `references`, `mixes_in`, `implements`).
- When another task holds the store lock, a busy banner with reindex progress
  (`done/total files`) and/or graph-build progress, plus guidance to retry.

### reindex

- Requires an active index; status: `[wait] codeindex: reindexing...`

### lang | languages

- Supported language ids, 5 per line, followed by the total count.

### rebuild

- `[ok] FTS rebuild complete: N documents indexed.`
- On failure: `[err] FTS rebuild failed`

### graph build | graph lang

- `graph build` starts the background graph build (progress surfaces in
  `show` and in the busy banner).
- The `graph` usage line mentions `export`, but no export arm is implemented -
  only `build` and `lang <language>` are accepted.

### godnodes [N]

- Table: `# | Symbol | Source File | Degree`. Default N is 10, clamped to the
  range 1-100.

### explain <symbol>

- Header with node metadata (source file, line, community, degree) plus two
  tables, `Incoming` and `Outgoing`: `Symbol | File | Kind | Confidence`.
- Unknown symbol: a `[warn]` not-found message.

### path <A> <B>

- Hop chain, each hop rendered as `` A` --kind--> `B `` with confidence tags.
- No connecting path: `[warn]` no-path message.

### communities

- Table: `Community | Label | Members`.
- No communities detected: `[warn] No communities detected`.

### Shared guards

- Graph subcommands without graph data: `[warn] No graph data available. Run
  /codeindex graph build first...`
- Any subcommand requiring an active index when none is active: `[warn] Code
  index is not active. Enable it first with /codeindex on`

## Related

- Full codeindex manual: `docs/howtos/codeindex.md`.
- `/codeindex on|off` state, language filtering, and the `idx`/`graph` status
  bar busy indicators.
- `codeindex_search`, `codeindex_symbols`, `codeindex_references`,
  `codeindex_dependencies`, `codeindex_explain`, `codeindex_path`,
  `codeindex_communities`, `codeindex_godnodes` LLM tools.
- `/tools codeindex on|off` controls tool visibility rather than indexing.