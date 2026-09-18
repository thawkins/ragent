# Tools — Memory

Three-tier memory: structured SQLite store (these tools), file blocks, and
optional embedding-based semantic search. Structured memories carry a
category, tags, and a confidence score.

| Tool | Description |
|------|-------------|
| `memory_store` | Store a structured memory (category, tags, confidence). |
| `memory_recall` | Full-text search of structured memories. |
| `memory_forget` | Delete memories by ID or filter. |
| `conversation_search` | Search current session conversation history. |
| `session_search` | Search across all past sessions. |

**Confidence scale:** 0.0 = uncertain, 1.0 = certain. Use 0.7 for ordinary
observations, >= 0.9 for verified facts, <= 0.5 for hunches. Tags are
free-form but should be lowercase and hyphenated (e.g. `rust`, `startup`).

---

## memory_store

Store a structured memory.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `content` | string | yes | The memory content — a fact, pattern, insight, etc. | `"We use anyhow::Result for errors"` |
| `category` | enum | yes | `fact`, `pattern`, `preference`, `insight`, `error`, `workflow` | `"preference"` |
| `tags` | array | no | Lowercase/hyphenated strings for filtering | `["rust","error-handling"]` |
| `confidence` | number | no | 0.0–1.0 (default 0.7) | `0.9` |
| `source` | string | no | Source of the memory | `"manual"`, `"auto-extract"` |

**Example:**
```text
memory_store content="We use anyhow::Result for errors" category="preference" confidence=0.9
memory_recall query="error handling"
```

---

## memory_recall

Full-text search of structured memories. All keyword terms must match (AND);
broaden by removing terms. Access counts are incremented on return.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes | Space-separated search terms | `"error handling"` |
| `categories` | array | no | Restrict to these categories | `["pattern","workflow"]` |
| `tags` | array | no | Memories must have ALL these tags | `["rust"]` |
| `limit` | integer | no | Maximum results | `5` |
| `min_confidence` | number | no | Confidence threshold 0.0–1.0 | `0.5` |

---

## memory_forget

Delete memories by ID or by filter criteria. At least one criterion is
required as a safety measure.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `id` | integer | one criterion | Delete one memory by its row ID |
| `older_than_days` | integer | one criterion | Delete memories not updated in this many days |
| `max_confidence` | number | one criterion | Delete memories with confidence at or below this value |
| `category` | enum | one criterion | Delete all memories in the category |
| `tags` | array | one criterion | Delete memories that have ALL of these tags |

**Example:** `memory_forget max_confidence=0.2`

---

## conversation_search

Search the current session conversation history. Modes: `keyword` (default),
`turn_range`, `stats`.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes for `keyword` | Search query | `"cargo fmt"` |
| `mode` | enum | no | `keyword` (default), `turn_range`, `stats` | `"keyword"` |
| `start_turn` / `end_turn` | integer | yes for `turn_range` | 1-based inclusive turn range | `1` / `20` |
| `limit` | integer | no | Keyword match count | `10` |
| `context_turns` | integer | no | Surrounding turns per keyword match | `0` |

**Example:** `conversation_search query="edit old_string" limit=5`

---

## session_search

Search across all past sessions using FTS5 keyword search.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes | FTS5 keyword search (implicit AND across terms) | `"compaction runner"` |
| `since` / `until` | string | no | ISO-8601 date range bounds | `"2026-09-01"` |
| `working_dir` | string | no | Restrict to sessions in this directory | — |
| `roles` | array | no | Restrict to these roles | `["user","assistant"]` |
| `session_id` | string | no | Restrict to a single session id | — |
| `limit` | integer | no | Maximum total results | `10` |
| `max_per_session` | integer | no | Cap results from any one session | `3` |
| `include_tools` | boolean | no | Include tool-call content (default true) | `false` |
| `include_system` | boolean | no | Include system/compaction messages | `false` |
| `context_turns` | integer | no | Surrounding turns included per result | `0` |

**Example:**
```text
session_search query="govdoc spec" since="2026-09-01" limit=10
```
