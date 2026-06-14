# `/compress` — Context-Aware Compression

ragent uses a single slash command, `/compress`, for managing conversation
context when it grows too large for the model's context window. Compression
runs locally and is content-aware.

## Quick Summary

| | `/compress` |
|---|---|
| **Mechanism** | Headroom content-aware compression |
| **What it does** | Intelligently shrinks each message while preserving structure |
| **Data loss** | Low — original content is stashed and recoverable |
| **Reversibility** | Reversible via CCR (Compress-Cache-Retrieve) store |
| **Requires LLM call** | No (runs locally, no API call) |
| **Speed** | Fast (local processing) |
| **Cost** | Free (no LLM call) |
| **Feature flag** | Requires `compression` feature |
| **Auto-trigger** | Yes, at configurable threshold (default 80%) |
| **Subcommands** | `default`, `aggressive`, `conservative`, `help`, `stats` |

---

## `/compress` — Content-Aware Compression

### How it works

The `/compress` command uses the **Headroom** compression pipeline, which
operates locally without any LLM call. It processes each message individually
through a series of specialised compressors:

1. **Token counting** — Estimates current token usage against the context
   window.
2. **Threshold check** — If usage is below the threshold (default 80%),
   nothing happens.
3. **Content detection** — Each message part is classified by type (JSON,
   diff, log, search, code, prose).
4. **Compressor routing** — Classified content is routed to the appropriate
   compressor:
   - **SmartCrusher (JSON)** — Minifies whitespace, deduplicates keys,
     strips redundant fields from tool outputs.
   - **DiffCompressor** — Selects important hunks, trims context lines,
     preserves file paths.
   - **LogCompressor** — Deduplicates repeated lines, filters by priority
     level.
   - **SearchCompressor** — Deduplicates file-level entries, preserves
     symbol locations.
   - **CodeCompressor** (experimental) — AST-based compression of source
     code.
   - **ProseCompressor** (experimental) — ML-based text compression.
5. **CCR stashing** — Original content is stored in a Compress-Cache-Retrieve
   store (SQLite by default) keyed by BLAKE3 hash. The compressed version
   replaces the original in the message.
6. **Protected messages** — System messages and the most recent user message
   are never compressed.

### Subcommands

| Subcommand | Description |
|---|---|
| `/compress` or `/compress default` | Run the default pipeline using your `ragent.json` config |
| `/compress aggressive` | Maximum compression — all compressors on, BM25 relevance filtering, 50% threshold |
| `/compress conservative` | Lossless only — JSON and diff compressors, 90% threshold |
| `/compress help` | Show help text and current configuration |
| `/compress stats` | Show message count, token estimate, and compressor status |

### Modes in detail

**Default** — Uses the `compression` settings from `ragent.json`. By default,
JSON, diff, log, and search compressors are enabled; code and prose are off.

**Aggressive** — Overrides config to:
- Trigger at 50% of context window (instead of 80%)
- Enable all compressors including code and prose
- Enable BM25 relevance filtering (keeps top 15 most relevant messages)

**Conservative** — Overrides config to:
- Trigger at 90% of context window
- Enable only lossless compressors (JSON minification, diff tag protection)
- Disable relevance filtering

### Configuration

Add to `ragent.json`:

```json
{
  "compression": {
    "enabled": true,
    "auto_threshold": 0.80,
    "ccr": {
      "backend": "sqlite",
      "capacity": 1000,
      "ttl_secs": 300
    },
    "compressors": {
      "json": true,
      "diff": true,
      "log": true,
      "search": true,
      "code": false,
      "prose": false
    },
    "relevance": {
      "enabled": false,
      "scorer": "bm25",
      "keep_top_k": 20
    },
    "tokenizer": {
      "backend": "auto"
    }
  }
}
```

### Trade-offs

- ✅ Fast — no LLM call, runs locally
- ✅ Free — no token cost
- ✅ Reversible — originals stored in CCR, retrievable via `headroom_retrieve`
- ✅ Content-aware — different strategies for JSON, diffs, logs, etc.
- ✅ Preserves structure — tool calls and their results stay paired
- ❌ Requires the `compression` Cargo feature at compile time
- ❌ May not reduce context as much as full summarisation
- ❌ CCR store has a capacity limit (default 1000 entries, LRU eviction)

---

## Fallback

When context grows too large and compression is disabled or unavailable,
ragent no longer falls back to truncation. The compression pipeline is the
only automatic context-reduction path; builds without the `compression`
feature keep history unchanged.

---

## Status Bar Indicator

When compression is in progress, the status bar shows:

- `"compressing"` — Headroom compression pipeline is running
