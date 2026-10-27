# SQLite review and remediation plan

Scope: every `rusqlite` use in the ragent workspace. Five stores, five files,
236 SQL sites. Findings below are evidence-backed: query plans and timings were
measured against this repository's own live databases (see
[Appendix A](#appendix-a--measurement-harness)).

Status: analysis complete, no code changed. Each remediation item is a separate,
independently verifiable task.

---

## 0. Executive summary

The SQLite layer is in better shape than its size suggests. Prepared statements
are cached almost everywhere, explicit transactions wrap the multi-statement
writes, the code-index queries all use indexes, and the FTS warm-up was already
fixed once. There is **one structural problem**, **one correctness bug**, and a
set of index and retention gaps.

| # | Finding | Severity | Evidence |
|---|---|---|---|
| P-01 | `messages.parts` is the whole store: 1.57 GB of a 2.06 GB db, with 35% of it reasoning text. Nothing prunes it. | Critical | `dbstat`: `messages` 1607 MiB, `messages_fts_content` 283 MiB, `messages_fts_data` 96 MiB |
| P-02 | `messages_fts` duplicates message text with **no benefit**: its only two queries filter on `session_id`, which the table declares `UNINDEXED`, so every search scans the entire vtab. | Critical | `session filter` probe: 50 ms vs 0.5 ms unfiltered |
| P-03 | `memories_fts` declares `content=memories` (external content) but **no triggers**, and `update_memory_content` assumes SQLite maintains the index. It does not. Search silently misses edited memories. | Critical (correctness) | `storage.rs:1008`, `:3527-3535` |
| P-04 | `IndexStore` lives behind a `std::sync::Mutex`; `query_symbols(..)` defaults to "load every symbol" and `derive_edges_for_file` runs once per changed file. 98 ms of blocked readers per file. | High | measured 98.5 ms per file; 4.9 s per 50-file watcher batch |
| P-05 | `sessions` has no index for its only list query. Every session picker does a full table scan plus a temp b-tree sort. | Medium | `SCAN sessions` + `USE TEMP B-TREE FOR ORDER BY` |
| P-06 | `todos` list queries sort in a temp b-tree (`ORDER BY created_at` is not a prefix of `(session_id, status)`). | Medium | `USE TEMP B-TREE FOR ORDER BY` on both list variants |
| P-07 | Activity log has no retention: 259 runs / 65 783 events / 90 MiB, growing forever. `expire_runs_older_than` exists and has **no production caller**. | Medium | test-only call sites; file size |
| P-08 | The code-index WAL is never checkpointed (`wal_autocheckpoint` unset). 204 MB WAL against a 468 MB db. | Medium | `ls .ragent/codeindex/`: `index.db-wal` 204 MB |
| P-09 | `graph_edges` carries `idx_edges_kind`, which no query can use (single-column, low cardinality, never the sole predicate). Writes pay for it on every edge upsert. | Low | no `WHERE kind = ?` without a symbol predicate |
| P-10 | `storage.rs` never sets `PRAGMA foreign_keys = ON`, so its nine `FOREIGN KEY`/`ON DELETE CASCADE` declarations are inert. | Low | 9 `FOREIGN KEY` declarations, 0 `foreign_keys` pragmas |
| P-11 | `SourceVault` and `ContentCache` use `conn.prepare(..)` (not `prepare_cached`) on their hot paths; `ContentCache` also runs the full eviction query after **every** insert. | Low | 0 `prepare_cached` in both files |
| P-12 | Dead schema: `snapshots` and `mcp_servers` tables have no CRUD anywhere in the tree. | Low | grep: zero `FROM snapshots` / `FROM mcp_servers` |

Nothing here requires new dependencies, `unsafe`, or an architecture change.

---

## 1. Inventory

### 1.1 Dependency

```toml
# Cargo.toml:206
rusqlite = { version = "0.40", features = ["bundled"] }
```

`libsqlite3-sys` is the only transitive SQLite crate. No `sqlx`, no pool
crate, no `tokio-rusqlite`, no ORM.

| Crate | Use |
|---|---|
| `ragent-storage` | `storage.rs`, `activity_log.rs` |
| `ragent-codeindex` | `store.rs` |
| `ragent-research` | `source_vault.rs` |
| `ragent-tools-extended` | `masterfetch/cache.rs` |
| `ragent-types` | `error.rs` only (`#[from] rusqlite::Error`) |

`ragent-telemetry`, `ragent-server`, `ragent-config`, `ragent-connectors`,
`ragent-plugins`, `ragent-surface`, `ragent-tools-core` contain **no SQLite**.

### 1.2 The five stores

| Store | File | Lines | Database | Tables | Indexes | Concurrency |
|---|---|---|---|---|---|---|
| `Storage` | `crates/ragent-storage/src/storage.rs` | 5702 | `$XDG_CONFIG_HOME/ragent/ragent.db` | 18 (+1 dup `settings`) | 16 | `Mutex<Connection>` writer + `Option<Mutex<Connection>>` reader |
| `ActivityLog` | `crates/ragent-storage/src/activity_log.rs` | 1768 | `$XDG_CONFIG_HOME/ragent/activity_log.db` | 1 | 2 (1 unique) | `Mutex<Connection>` |
| `IndexStore` | `crates/ragent-codeindex/src/store.rs` | 1815 | `<index_dir>/index.db` | 8 | 19 | bare `Connection`, wrapped by `CodeIndex.store: Mutex<_>` |
| `SourceVault` | `crates/ragent-research/src/source_vault.rs` | 656 | `<run>/vault.db` (one per run) | 1 | 2 | `Arc<Mutex<Connection>>` |
| `ContentCache` | `crates/ragent-tools-extended/src/masterfetch/cache.rs` | 710 | `<cwd>/.ragent/masterfetch_cache.db` | 1 | 2 | `Arc<Mutex<Connection>>` |

### 1.3 Schema ownership

Three migration idioms coexist. Only `Storage` has a version-gated fast path.

| Store | Entry point | Versioning |
|---|---|---|
| `Storage` | `Storage::migrate` (`storage.rs:807`) | `SCHEMA_VERSION = 2` row in `settings`; skips 34 DDL statements + 7 `pragma_table_info` probes when current |
| `ActivityLog` | `ActivityLog::migrate` (`activity_log.rs:246`) | none; pure `CREATE ... IF NOT EXISTS` |
| `IndexStore` | `IndexStore::init_schema` (`store.rs:155`) | `SCHEMA_VERSION = 4` row in `schema_version` (written, never read back) |
| `SourceVault` | `SourceVault::migrate` (`source_vault.rs:405`) | none; `ALTER TABLE` with ignored error |
| `ContentCache` | `ContentCache::init` (`cache.rs:258`) | none; `SELECT col LIMIT 0` probes then `ALTER` |

`IndexStore::init_schema` writes a version row but never compares it, so it
re-runs all 27 DDL statements on every open. See [T-09](#t-09--remove-the-unread-schema-version-write).

### 1.4 Pragmas - current state

| Pragma | `Storage` | `ActivityLog` | `IndexStore` | `SourceVault` | `ContentCache` |
|---|---|---|---|---|---|
| `journal_mode = WAL` | yes | yes | yes | yes | yes (falls back to `memory`) |
| `synchronous = NORMAL` | yes | yes | yes | no (FULL) | yes |
| `busy_timeout` | 30 s | 5 s | **no** | 5 s | **no** |
| `wal_autocheckpoint` | 500 pages | no | **no** | no | no |
| `temp_store = MEMORY` | no | no | yes | no | no |
| `foreign_keys = ON` | **no** | n/a | yes | n/a | n/a |
| `cache_size` | default | default | default | default | default |
| `mmap_size` | default | default | default | default | default |

---

## 2. Findings and evidence

Measurements were taken on this repository's own data:

- `.ragent/codeindex/index.db`: 3798 files, 45 323 symbols, 309 033 refs, 764 916 edges.
- `~/.config/ragent/ragent.db`: 3734 sessions, 18 219 messages, 2841 todos, 4687 run costs.
- `~/.config/ragent/activity_log.db`: 259 runs, 65 783 events.

### P-01 - `messages.parts` dominates the database, and nothing prunes it

`dbstat` on the live `ragent.db`:

```
messages             1607.0 MiB
messages_fts_content  283.0 MiB
messages_fts_data      96.0 MiB
todos                   1.0 MiB
idx_messages_session    1.0 MiB
background_tasks        1.0 MiB
```

2.06 GB total, of which **78% is one table's one column**. 18 219 messages hold
1577 MiB of `parts` JSON (mean 90 KB, max 9.8 MB). 35% of the sampled bytes are
`MessagePart::Reasoning` text.

The write path is `create_message` -> one `INSERT`, no size cap, no compaction of
stored history. `get_messages` (`storage.rs:1484`, SQL at `:1486-1488`) reads the column for **every**
message in the session with no `LIMIT`, and it is called on session resume, by
`conversation_search`, and once per result by `session_search`.

Why it matters beyond disk: every `get_messages` is a 1.5 GB-column read into
`serde_json::from_str`, and the two FTS shadow tables add another 379 MiB of
copies of (a subset of) the same content.

### P-02 - `messages_fts` is a full-scan liability, not an index

`messages_fts` stores the message text a second time. Its only queries both
filter on `session_id`:

```sql
-- storage.rs:4359-4366 search_conversation
WHERE messages_fts MATCH ?1 AND f.session_id = ?2 ORDER BY f.rank LIMIT ?3
-- storage.rs:4438-4520 search_session_messages
WHERE messages_fts MATCH ?1 AND f.session_id = ?{n} ... ORDER BY f.rank
```

`session_id` is declared `UNINDEXED` (`storage.rs:1054-1060`), so the plan is
`SCAN f VIRTUAL TABLE INDEX 32:M4` - FTS evaluates the match for **every** row,
then the filter discards. Measured on the live db:

| Query | Time |
|---|---|
| `MATCH 'the' LIMIT 10` (no session filter) | 0.5 ms |
| `MATCH 'the' AND session_id = 'x' LIMIT 10` | **50.0 ms** |
| full `search_conversation` for `'the'` | **74.7 ms** |
| full `search_conversation` for `'memory'` | 41.2 ms |

The session filter makes the query 100x slower than not filtering, and it pays
the full cost even when the answer is zero rows.

Two dead-weight components:

1. **The content shadow table.** `messages_fts_content` (283 MiB) exists only so
   `rank`/`highlight()` can re-read the document. No call site uses either;
   `search_conversation` selects `f.content` (the stored column, not the
   shadow). The shadow is pure overhead.
2. **The per-row `INSERT`.** `create_message` writes a second full copy of the
   message text inside the same transaction.

`warm_message_search_index` already documents (correctly) that a
`message_id` join cannot use an index on this table, and rebuilt the reconcile
as a rowid anti-join. That fix removed the *warm-up* scan; the *query* path
still scans.

### P-03 - `memories_fts` is external-content with no triggers (correctness)

```sql
-- storage.rs:1010
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
USING fts5(content, content=memories, content_rowid=rowid);
```

An external-content FTS5 table is **not maintained** by SQLite on `UPDATE` -
that is what `content=` means. The store must issue the FTS5 `'delete'` +
re-`INSERT`, or keep triggers.

`update_memory_content` says the opposite:

```rust
// storage.rs:3527
"UPDATE memories SET content = ?1, updated_at = ?2 WHERE id = ?3" ...
// For content-synced FTS5 tables, the index is automatically updated
// when the underlying content table is modified. No manual FTS update needed.
```

The comment is wrong. After an edit, `search_memories` (`storage.rs:3130-3140`)
still matches the old text and no longer matches the new text. `create_memory`
and the four delete paths do the FTS bookkeeping by hand, so the omission is
inconsistent, not deliberate.

`delete_memories_by_filter` (`storage.rs:3322-3335`) issues one
`DELETE FROM memories_fts WHERE rowid = (SELECT rowid FROM memories WHERE id = ?)`
**per id** inside the transaction; FTS5 deletes are the most expensive operation
on the vtab and they are batched poorly.

### P-04 - `derive_edges_for_file` reloads every symbol, per file, under the lock

```rust
// graph/edges.rs:328
pub fn derive_edges_for_file(store: &IndexStore, file_id: i64) -> Result<()> {
    let file_symbols = store.get_file_symbols(file_id)?;
    ...
    let all_symbols = store.query_symbols(&SymbolFilter::default())?;  // every symbol
    let nr = NameResolution::from_symbols(&all_symbols);
```

`SymbolFilter::default()` has no name/kind/file/language filter, so the SQL is
`SELECT ... FROM symbols ORDER BY name LIMIT 1_000_001` - a covering-index scan of
all 45 323 rows plus the deserialisation of every symbol.

Call path: file watcher -> `index_files(&paths)` -> `index_file(..)` per path ->
this function per path, each time while `CodeIndex.store`'s mutex is held. The
watcher's default `batch_size` is 50.

Measured on the live index:

```
derive_edges_for_file all-symbols load: 226 615 rows over 5 files in 493 ms
                                       => 98.5 ms/file
projected for a 50-file watcher batch:   4.9 s
```

During those 4.9 s every `codeindex_*` tool call, and every TUI status poll that
touches the store, blocks. `graph/mod.rs:197` (`godnodes`), `traverse.rs:147`
(`explain`) and `communities.rs:151` have the same whole-table load, but they
are one-shot user commands, not per-file loops.

### P-05 - `sessions` has no index for its list query

```sql
-- storage.rs:59 SQL_LIST_SESSIONS_FMT_VERSION
SELECT ... FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC
```

No index on `sessions` beyond the implicit `id` primary key. Plan:

```
SCAN sessions
USE TEMP B-TREE FOR ORDER BY
```

5.6 ms for 3734 rows today. It grows linearly and it runs on every session
picker open and every `GET /sessions`. A partial index removes the sort
entirely (`SCAN sessions USING INDEX idx_sessions_active_updated`, still
walking all rows because `archived_at IS NULL` matches almost everything, but
no temp b-tree; measured 9.5 ms on a copy, dominated by copying 3734 rows of
wide text columns). The sort is the part that scales badly.

### P-06 - `todos` sorts in a temp b-tree

```
list_todos no status:   SEARCH todos USING INDEX idx_todos_session (session_id=?)
                        USE TEMP B-TREE FOR ORDER BY
list_todos with status: SEARCH todos USING INDEX idx_todos_session (session_id=? AND status=?)
                        USE TEMP B-TREE FOR ORDER BY
```

`ORDER BY created_at` cannot be served because `created_at` is not in
`(session_id, status)`. Small today (max 44 rows per session), but it is the
hot Task-tool path and the fix is one index.

### P-07 - the activity log has no retention

`activity_log.db` is 90 MiB: 259 runs, 65 783 events, mean 254 events per run.
`ActivityLog::expire_runs_older_than` (`activity_log.rs:1372`) exists, is
tested, and has **no production caller** - only
`crates/ragent-storage/tests/test_activity_retention.rs`. Nothing ever deletes a
run.

Also: `expire_runs_older_than` runs `SELECT run_id ... GROUP BY run_id HAVING
MAX(timestamp) < ?` (a full index scan of 65 783 rows), then calls
`expire_run(..)` **once per stale run**, each taking the mutex separately and
performing its own transaction. If it is ever wired up it will be an N-transaction
purge.

### P-08 - the code-index WAL is never checkpointed

```
-rw-r--r-- 467791872 .ragent/codeindex/index.db
-rw-r--r-- 204150152 .ragent/codeindex/index.db-wal
```

`IndexStore::init_schema` sets `journal_mode`, `synchronous`, `temp_store` and
`foreign_keys` - but not `busy_timeout` and not `wal_autocheckpoint`.
`Storage` sets `wal_autocheckpoint = 500` pages (`storage.rs:721`) precisely
because it hit this; the code index did not get the same treatment. A 204 MB
WAL means every connection open replays/validates a large WAL and the file is
never truncated. `Storage::checkpoint_wal` exists and is also never called in
production (only `tests/test_settings_and_file_storage.rs:112`).

### P-09 - `idx_edges_kind` is write-only

`graph_edges` has `idx_edges_source`, `idx_edges_target`, `idx_edges_kind` and
the `UNIQUE(source_sym, target_sym, kind)` autoindex. No query filters on
`kind` alone; `query_edges_for_symbol` uses a `MULTI-INDEX OR` over
source/target, and `persist_edges` upserts 764 916 rows. Every one of those
upserts maintains an index that is read zero times.

### P-10 - `Storage` declares 9 foreign keys and enforces none

`PRAGMA foreign_keys` defaults to OFF in SQLite and is per-connection.
`storage.rs` never sets it; `store.rs:161` does. The nine `FOREIGN KEY`
declarations in `ragent.db` (`messages`, `snapshots`, `run_cost_summaries`,
`background_tasks`, `todos`, `memory_tags`, `kg_relationships`,
`messages_embedding`) therefore do nothing, and `ON DELETE CASCADE` never
fires. There is no `DELETE FROM sessions` today, so nothing is currently broken
- but the schema advertises an invariant that is not enforced, and the FTS
shadow rows depend on manual cleanup anyway (see P-02).

### P-11 - uncached prepares and an eviction per insert

`SourceVault` uses `conn.prepare(..)` for every query (0 `prepare_cached`);
`ContentCache` likewise (0 `prepare_cached` in the file). `ContentCache::insert`
calls `evict_to_cap` after **every** write, and `evict_to_cap` sums
`size_bytes` over the whole table and then loads `(url, extraction_type,
css_selector, pages, size_bytes)` for **every row** ordered by `created_at`, on
every insert, holding the mutex.

### P-12 - dead schema

`snapshots` (`storage.rs:887`) and `mcp_servers` (`storage.rs:880`) have DDL and
no producer or consumer anywhere in the tree. `snapshots` is additionally
superseded by the file-based `ragent-storage/src/snapshot.rs`, which contains no
SQL.

---

## 3. What is already correct (do not "fix" these)

Recording these so a later pass does not regress them:

- **Prepared-statement caching**: 26/29 `store.rs` statements, all but two
  `storage.rs` statements use `prepare_cached`. The two `conn.prepare` sites in
  `storage.rs` are in the one-shot warm-up path.
- **Transaction wrapping on multi-statement writes**: `create_message`,
  `update_message`, `delete_messages`, `create_memory`, `delete_memory`,
  `delete_memories_by_filter`, `clear_memories_for_project`, the whole
  `migrate` batch, and `apply_diff`/`upsert_symbols`/`upsert_edges_batch` all
  commit once.
- **`upsert_symbols` autocommit detection** (`store.rs:726`): standalone calls
  get their own transaction, batch calls join the caller's. Correct.
- **`INSERT ... RETURNING id`** in `upsert_symbols_inner`: removes one round-trip
  per symbol.
- **FTS warm-up as a rowid anti-join** (`storage.rs:4644`): the previous
  `f.message_id = m.id` plan was an O(nxm) nested scan; `test_warm_index_scan.rs`
  locks the new plan in with `EXPLAIN QUERY PLAN`.
- **Activity-log append is `BEGIN IMMEDIATE`** so `MAX(seq)+1` and the insert are
  one atomic step, with typed `DuplicateSeq`/`DuplicateEventId`/`OutOfOrder`
  errors.
- **`Storage`'s dual connection** (writer mutex + read-only reader) means reads
  do not queue behind writes in WAL mode.
- **Batch writes in `full_reindex`** (chunk of 20, one SQLite transaction and one
  tantivy commit per chunk, parse outside the lock).
- **`storage.rs` migration fast path** via the `schema_version` setting.
- **`MULTI-INDEX OR` on `graph_edges`**: `WHERE source_sym=? OR target_sym=?`
  uses both indexes; `delete_edges_for_symbols` does too.
- **All `query_symbols` filter shapes use an index**: prefix name ->
  `idx_symbols_name_nocase` (range seek), kind -> `idx_symbols_kind`,
  language -> `idx_files_language` -> `idx_symbols_file`. The only slow shape is
  the unfiltered default (P-04).

---

## 4. Remediation plan

Ordered by value. Each task is independently shippable and independently
verifiable. Sizes are implementation time on this codebase assuming the existing
tests run.

### Critical

---

#### T-01 - Resolve the `messages_fts` redundancy

**Size:** half a day. **Depends on:** nothing. **Blocks:** T-02.

The fix is a schema change: keep the FTS index, drop the duplicate text, and
move the key columns where an index can reach them.

```sql
-- schema v3
DROP TABLE messages_fts;
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    content='',              -- contentless: no shadow copy of the text
    contentless_delete=1,    -- requires SQLite >= 3.43; bundled 3.51 is fine
    tokenize='porter unicode61'
);
```

`message_id`/`session_id`/`role` move **out** of the vtab and into a small
ordinary table that the rowid joins to:

```sql
CREATE TABLE message_fts_keys (
    rowid      INTEGER PRIMARY KEY,   -- the FTS rowid
    message_id TEXT    NOT NULL UNIQUE,
    session_id TEXT    NOT NULL,
    role       TEXT    NOT NULL
);
CREATE INDEX idx_fts_keys_session ON message_fts_keys(session_id);
```

Then `search_conversation` becomes a session-first plan:

```sql
SELECT k.message_id, k.session_id, k.role, m.created_at, s.title, s.directory, -bm25(f) AS rank
FROM message_fts_keys k
JOIN messages_fts f ON f.rowid = k.rowid
JOIN messages   m ON m.id    = k.message_id
LEFT JOIN sessions s ON s.id = k.session_id
WHERE f.content MATCH ?1 AND k.session_id = ?2
ORDER BY rank DESC
LIMIT ?3
```

`messages_fts_content` is 283 MiB of the 2.06 GB db, so the contentless form
removes ~14% of the database and one full copy of every message text on every
`create_message`. It also removes the shadow table that makes the warm-up
reconcile necessary at all.

There is no "just add an index" variant to consider: FTS5 has no secondary
indexes over `UNINDEXED` columns, so making the session filter cheap requires
moving `session_id` out of the virtual table - which is what the schema above
does.

**Verification**

1. New test `tests/test_fts_session_scope.rs`:
   `EXPLAIN QUERY PLAN` for `search_conversation` must not contain
   `SCAN f VIRTUAL TABLE`; it must contain a seek on `idx_fts_keys_session`.
2. Timing assertion (informational, print-only): `MATCH 'the'` scoped to a
   missing session must be within 2x of the unscoped `LIMIT 10` query
   (measured baseline 50.0 ms vs 0.5 ms before, i.e. 100x).
3. `dbstat` after migration on a copy of the live db: `messages_fts` +
   shadow tables must be < 120 MiB (from 379 MiB).
4. Existing `test_warm_index_scan.rs` plan assertions must be rewritten for
   the new shape - the rowid anti-join becomes a straight `NOT IN` over
   `message_fts_keys`, which is now indexed.

**Behaviour change to state in the changelog:** FTS matching itself is
unchanged - `search_session_messages` (cross-session) keeps working by joining
through the key table and applying its filters there. What changes is that the
`message_id`, `session_id` and `role` columns are no longer stored *inside* the
virtual table; they are in `message_fts_keys`. Any code that reads
`messages_fts.message_id` directly must read `message_fts_keys.message_id`
instead (there is one such site today, `replace_fts_row` at
`storage.rs:4839`).

---

#### T-02 - Bound and prune `messages.parts`

**Size:** one day. **Depends on:** T-01 (do it first so you only migrate the
FTS once).

Three separate caps, applied at write time, in this order of importance:

1. **Stop storing reasoning text in `parts`.** 35% of the sampled bytes are
   `MessagePart::Reasoning`. Reasoning is a UI concern; it does not need to
   survive a restart. Either drop it from the persisted `parts` (keep it in the
   in-memory message) or store it truncated to N KB with an explicit marker.
   This is the single largest win and needs no new table.
2. **Cap the tool-call `output` field** stored in `parts` with the same
   truncation the UI already applies on render. A 9.8 MB message exists because
   one tool output was persisted at full length.
3. **Add a retention sweep.** `Storage::prune_session_messages(session_id,
   keep)` deleting everything older than the newest N messages, and
   `Storage::prune_messages_older_than(duration)`. Wire the former into the
   existing compaction path (`ragent-agent/src/session/` compaction already
   decides what the model sees; this makes storage agree with it).

**Verification**

1. `tests/test_message_size_cap.rs`: create a message with 1 MB of reasoning and
   5 MB of tool output; assert the stored `LENGTH(parts)` is below the
   configured cap and that `get_messages` returns a message with the truncation
   marker.
2. `tests/test_message_retention.rs`: seed 100 messages, prune to 20, assert
   count, assert `messages_fts` has no orphan rows (T-01 shape), assert session
   still loads.
3. Regression gate: `dbstat` on a 30-day-old session snapshot must show
   `messages` below 10% of its pre-change size. Record the before/after in the
   task doc.

**Risk:** truncating stored tool output loses replay fidelity for old sessions.
Mitigate by keeping the full output in the sub-agent log files that already
exist (`log/subagents/<task-id>.md`) and storing only a reference.

---

#### T-03 - Fix `memories_fts` synchronisation (correctness bug)

**Size:** 2 hours. **Depends on:** nothing.

Add triggers to the schema (idempotent, so an existing db picks them up on the
next `migrate`) - the same shape the SQLite documentation prescribes for
external-content FTS5:

```sql
CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
  INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;
CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content)
  VALUES('delete', old.rowid, old.content);
END;
CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content)
  VALUES('delete', old.rowid, old.content);
  INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;
```

Then **delete** the manual `memories_fts` statements in `create_memory`
(`:2949`), `delete_memory` (`:3226`), `delete_memories_by_filter` (`:3328`),
`clear_memories_for_project` (`:3359`, `:3368`) and fix the wrong comment in
`update_memory_content` (`:3527`).

Bump `SCHEMA_VERSION` to 3 and, in the migration, run the one-time rebuild
because existing rows may already be stale:

```sql
INSERT INTO memories_fts(memories_fts) VALUES('rebuild');
```

**Verification**

1. New test `tests/test_memory_fts_sync.rs`:
   - create a memory with content A, `update_memory_content` to B,
     `search_memories("A")` returns nothing, `search_memories("B")` returns it.
     **This test fails today** - confirm it red before the fix.
   - delete the memory, both searches return nothing.
   - `clear_memories_for_project`, no orphans in `memories_fts`.
2. Assert the trigger set exists after `Storage::open` on a fresh db and on a
   pre-existing v2 db.

---

#### T-04 - Stop `derive_edges_for_file` from reloading every symbol

**Size:** half a day. **Depends on:** nothing.

Three complementary changes, in order:

1. **Cache the name-resolution table for the duration of a batch.**
   `index_files(&paths)` already loops; build `NameResolution` **once** before
   the loop and pass `&NameResolution` into a new
   `derive_edges_for_file_with(file_id, &nr)`. `derive_edges_for_file` keeps its
   signature and builds the table itself for single-file callers.
   Expected: 98.5 ms/file -> ~1 ms/file inside a batch (the remaining cost is
   `get_file_symbols` + `get_file_refs` + `get_file_imports`, all index seeks).
2. **Make the whole-table load lazy.** With the batch cache in place the
   default `SymbolFilter` load happens once per batch instead of once per file,
   which is the actual fix. Additionally reduce what that one load costs: it
   currently deserialises `signature`, `doc_comment` and `body_hash` for all
   45 323 symbols when name resolution only needs `id`, `name`, `file_id`,
   `kind` and `visibility`. Add a projection for the resolver rather than a new
   cap - the `MAX_GRAPH_LOAD_ROWS` ceiling (1 000 001) exists to bound memory,
   not to make the load cheap, and lowering it would silently drop symbols from
   name resolution.
3. **Hoist the lock.** Do the two reads (`get_file_symbols`, `get_file_refs`)
   under the guard, then derive outside it, then persist. The existing
   `load_graph_inputs` + `derive_edges_from_inputs` + `persist_edges` split
   (FR-026) already demonstrates the pattern for the full build; apply it to the
   incremental path.

**Verification**

1. New test `tests/test_graph_incremental_cost.rs`, ignored by default and run
   with `--ignored`: build a 5000-symbol index, then `index_files` a 50-file
   batch and assert wall time is under a recorded ceiling. Print the measured
   value so the number is in the log, not just the pass/fail.
2. `EXPLAIN QUERY PLAN` assertion: the per-file path must not contain
   `SCAN symbols` (covering-index full scan) after change 1.
3. Existing `test_graph_*.rs` suites must stay green - the output must be
   byte-identical. Add a test that derives edges for a single file with
   `derive_edges_for_file` and with the batch variant and compares the two
   `Vec<GraphEdge>` for equality.

---

### High

---

#### T-05 - Index `sessions` for its list query

**Size:** 1 hour. **Depends on:** nothing (schema v3 slot, shares it with T-03).

```sql
CREATE INDEX IF NOT EXISTS idx_sessions_active_updated
    ON sessions(updated_at DESC)
    WHERE archived_at IS NULL;
```

Partial index: it only contains live sessions, so it stays small even when the
archive grows. Add to the `migrate` batch and bump `SCHEMA_VERSION`.

**Verification**

1. New test `tests/test_sessions_list_plan.rs`: `EXPLAIN QUERY PLAN` for
   `SQL_LIST_SESSIONS_FMT_VERSION` must contain
   `USING INDEX idx_sessions_active_updated` and must **not** contain
   `TEMP B-TREE`.
2. Assert `list_sessions` still excludes archived sessions and still orders by
   `updated_at DESC` with an archived-session fixture in the middle.

---

#### T-06 - Index `todos` for its list ordering

**Size:** 1 hour. **Depends on:** nothing (schema v3 slot).

```sql
CREATE INDEX IF NOT EXISTS idx_todos_session_created
    ON todos(session_id, created_at);
```

Keep `idx_todos_session(session_id, status)` for the filtered count paths; the
two are complementary. If the size matters, replace the old one with
`(session_id, status, created_at)` and drop both temp sorts at once - verify
with both plans below.

**Verification**

1. `tests/test_tasks_list_plan.rs`: both `list_tasks` variants
   (with and without a status filter) must show no
   `USE TEMP B-TREE FOR ORDER BY`.
2. Ordering assertion with two tasks created in the same second (tie-break
   must be stable - add `, id` to the `ORDER BY` if it is not).

---

#### T-07 - Wire up activity-log retention

**Size:** half a day. **Depends on:** nothing.

1. Add a config knob (`activity_log.retention_days`, default 30; `0` = keep
   forever) following the existing `ragent_config::activity_log` module.
2. Call `expire_runs_older_than` from the existing background writer task start
   (`SessionProcessor::start_activity_writer`) once per process start, and from
   a daily tick.
3. Rewrite the purge as **one** transaction: select the stale run ids, then a
   single `DELETE FROM activity_events WHERE run_id IN (...)` batched at 500 ids,
   recording the lifecycle events in the same transaction. Do not call
   `expire_run(..)` once per run.
4. Truncate the WAL after a purge (`PRAGMA wal_checkpoint(TRUNCATE)`), the way
   `Storage::checkpoint_wal` intends to.

**Verification**

1. `tests/test_activity_retention_wired.rs`: seed two runs with different
   `MAX(timestamp)`, run the sweep with `retention_days = 1`, assert exactly one
   run is gone, assert a lifecycle event was recorded for it, assert the other
   run is untouched (this is the test that exists today, extended to the
   batched implementation).
2. Assert the sweep performs **one** `DELETE` per 500 ids batch, via a
   statement-counting hook or by timing a 10 000-run fixture.
3. `activity_log.db` size after a synthetic 300-run/70 000-event load with
   `retention_days = 1` must return to near-empty.

---

#### T-08 - Checkpoint the code-index WAL and set the missing pragmas

**Size:** 1 hour. **Depends on:** nothing.

In `IndexStore::init_schema` (`store.rs:155`), alongside the existing pragmas:

```sql
PRAGMA busy_timeout = 5000;
PRAGMA wal_autocheckpoint = 1000;   -- pages; matches Storage's intent
```

and call `PRAGMA wal_checkpoint(TRUNCATE)` on graceful shutdown next to
whatever closes the `CodeIndex` (mirror `Storage::checkpoint_wal`, which itself
needs a production caller - see the note under T-07.4).

**Verification**

1. `tests/test_codeindex_pragmas.rs`: open a file-backed store, assert
   `journal_mode = wal`, `busy_timeout = 5000`,
   `wal_autocheckpoint = 1000`.
2. Write a large batch, call the new checkpoint, assert `index.db-wal` is 0
   bytes.

---

### Medium

---

#### T-09 - Remove the unread schema-version write in the code index

**Size:** 30 minutes. **Depends on:** nothing.

`IndexStore::init_schema` writes `SCHEMA_VERSION = 4` and never reads it, so
all 27 DDL statements run on every open. Either implement the fast path the way
`Storage::migrate` does (read first, skip the batch when current, bump the
version when the DDL changes) or delete the row. Do not leave it half-done.

**Verification**

`tests/test_codeindex_schema_fastpath.rs`: the second `open` of the same path
must issue no `CREATE TABLE` statement (assert via a counting authorizer).

---

#### T-10 - Drop `idx_edges_kind`

**Size:** 30 minutes + one graph rebuild. **Depends on:** nothing.

```sql
DROP INDEX IF EXISTS idx_edges_kind;
```

Verify first with `EXPLAIN QUERY PLAN` that no query regresses (see the
"measured plans" list in Appendix B - nothing uses it). Expected win: 764 916
index rows removed, and one fewer index maintained on every edge upsert during
every graph build.

**Verification**

1. `EXPLAIN QUERY PLAN` snapshot test over the five graph queries
   (`query_edges_for_symbol`, `query_all_edges`, `godnodes`,
   `delete_edges_for_symbols`, `query_edges_for_file`) asserting no plan
   degraded from `SEARCH` to `SCAN`.
2. Time a full `build_graph` on a copy of the live index before and after;
   record both numbers in the task doc.

---

#### T-11 - Batch the memory delete path

**Size:** 2 hours. **Depends on:** T-03 (the manual FTS deletes disappear, so do
this after).

`delete_memories_by_filter` currently runs one
`DELETE FROM memories_fts WHERE rowid = (SELECT ...)` per id. After T-03 the
triggers handle the FTS side, so this reduces to one
`DELETE FROM memories WHERE id IN (...)` - batched at 500 parameters to stay
under SQLite's variable limit.

**Verification**

`tests/test_memory_delete_batch.rs`: delete 1200 memories with one filter call,
assert the count, assert no `memories_fts` orphans, assert statement count is
3 (not 1200) via an authorizer.

---

#### T-12 - Cache the remaining prepared statements and skip the redundant eviction

**Size:** 3 hours. **Depends on:** nothing.

1. `SourceVault`: swap the nine `conn.prepare(..)` for `prepare_cached`; the
   dynamic token query is bounded (one `?n` per search token) so the cache
   stays small.
2. `ContentCache`: swap the three `conn.prepare(..)` for `prepare_cached`.
3. `ContentCache::insert` -> `evict_to_cap`: replace the full-table sum +
   full-table row load with

   ```sql
   -- one row, served by idx_fetch_cache_created
   SELECT COALESCE(SUM(size_bytes), 0) FROM fetch_cache;
   -- then, only if over the cap, evict in one statement per pass:
   DELETE FROM fetch_cache WHERE rowid IN (
       SELECT rowid FROM fetch_cache ORDER BY created_at ASC LIMIT ?1
   );
   ```

   looped until the sum is under the cap, instead of loading every row's
   four-column key into memory.

**Verification**

1. `tests/test_source_vault_prepared.rs` / `tests/test_mf_cache_evict.rs`:
   functional equivalence with the current behaviour (same rows evicted, same
   order), plus an authorizer assertion that no statement is prepared more than
   once across N calls.
2. Eviction on a 10 000-entry cache must complete without loading 10 000 rows
   (assert peak `SELECT` row count via the authorizer callback).

---

### Low

---

#### T-13 - Decide the `foreign_keys` question in `Storage`

**Size:** 2 hours. **Depends on:** T-01 and T-03 (both change what cleanup is
manual).

Two coherent choices; pick one deliberately.

**A. Turn it on** (`conn.pragma_update(None, "foreign_keys", "ON")` in
`Storage::open` and in the read-only connection is unnecessary - it is a
per-connection setting that only affects writes). This makes the nine declared
`FOREIGN KEY`s real and `ON DELETE CASCADE` fire. Requires an audit of every
delete path that currently relies on rows surviving, and a migration step for
databases that already contain orphans (run `PRAGMA foreign_key_check` at
startup and log).

**B. Delete the `FOREIGN KEY` clauses** and document that referential integrity
is application-enforced.

Do not leave the current state, where the schema claims a guarantee the engine
does not enforce.

**Verification** (choice A)

`tests/test_foreign_keys_enforced.rs`: inserting a `message` with an unknown
`session_id` must fail; deleting a session must cascade. `PRAGMA
foreign_key_check` on a fresh migration of the live db must return zero rows.

---

#### T-14 - Drop the dead `snapshots` and `mcp_servers` tables

**Size:** 30 minutes. **Depends on:** nothing.

Both have zero CRUD sites. `snapshots` is superseded by the file-based
`ragent-storage/src/snapshot.rs`. Removing them shrinks the schema batch and
removes two indexes from the migration. Keep the DDL in a comment or in this
document's history if a future feature wants them back.

**Verification**

`PRAGMA table_list` on a fresh db must not contain them; a migration from a v2
db must drop them without touching any populated table.

---

## 5. Suggested sequencing

```
T-03 (memories_fts triggers) -- go first, live correctness bug
T-01 (messages_fts)  --> T-02 (parts caps + retention)
T-01                 --> T-11 (memory delete batch, also after T-03)
T-05 + T-06 + T-09 + T-14   -- one schema-version bump, one migration test
T-04 (graph incremental)    -- independent, highest CPU win
T-07 (activity retention)   -- independent
T-08 (codeindex pragmas)    -- independent
T-10 (drop idx_edges_kind)  -- needs a graph rebuild to see the win
T-12 (prepare_cached)       -- independent, small
T-13 (foreign_keys)         -- decide after T-01/T-03
```

T-03 is a live correctness bug and should go first regardless of ordering. T-01
and T-02 are what turn the 2.06 GB database into something proportionate to
18 000 messages.

## Appendix A - measurement harness

Every number in this document came from one of these. Re-run them before and
after each task.

```bash
# 1. Where the bytes are (main store)
python3 - <<'PY'
import sqlite3, os
p = os.path.expanduser('~/.config/ragent/ragent.db')
c = sqlite3.connect(f'file:{p}?mode=ro', uri=True)
for name, mib in c.execute(
    'SELECT name, SUM(pgsize)/1048576.0 mib FROM dbstat GROUP BY name ORDER BY mib DESC LIMIT 15'
):
    print(f'{name:40s} {mib:8.1f} MiB')
print('freelist MiB', c.execute('PRAGMA freelist_count').fetchone()[0]
      * c.execute('PRAGMA page_size').fetchone()[0] / 1048576)
PY

# 2. Code-index volumes
python3 - <<'PY'
import sqlite3
c = sqlite3.connect('file:.ragent/codeindex/index.db?mode=ro', uri=True)
for t in ['indexed_files','symbols','symbol_refs','imports','file_deps','graph_edges','communities']:
    print(t, c.execute(f'select count(*) from {t}').fetchone()[0])
PY

# 3. Plan of any statement
python3 -c "
import sqlite3
c = sqlite3.connect('file:.ragent/codeindex/index.db?mode=ro', uri=True)
for r in c.execute('EXPLAIN QUERY PLAN ' + '''<statement>'''):
    print(r[3])
"
```

The existing `crates/ragent-storage/tests/test_warm_index_scan.rs` is the model
for turning a measured plan into a regression test - copy that pattern for the
new plan assertions in T-01, T-05, T-06, T-10.

## Appendix B - measured query plans (baseline, this repository)

Recorded so the "no regression" checks have something to compare against.

### `ragent.db`

| Query | Plan | Time |
|---|---|---|
| `list_sessions` | `SCAN sessions` + `USE TEMP B-TREE FOR ORDER BY` | 5.6 ms / 3734 rows |
| `list_todos` (no status) | `SEARCH todos USING INDEX idx_todos_session (session_id=?)` + `TEMP B-TREE FOR ORDER BY` | - |
| `list_todos` (status) | `SEARCH todos USING INDEX idx_todos_session (session_id=? AND status=?)` + `TEMP B-TREE FOR ORDER BY` | - |
| `list_background_tasks` (status) | `SEARCH ... idx_bg_tasks_session (session_id=? AND status=?)` | no temp b-tree |
| `list_background_tasks` (no status) | `SEARCH ... idx_bg_tasks_session (session_id=?)` + `TEMP B-TREE FOR ORDER BY` | - |
| `conversation_stats` | `SEARCH messages USING INDEX idx_messages_session (session_id=?)` x4 | - |
| `get_messages` | `SEARCH messages USING INDEX idx_messages_session (session_id=?)` | full `parts` column read |
| `search_conversation` | `SCAN f VIRTUAL TABLE INDEX 32:M4` + two PK seeks | 41-75 ms |
| unscoped `MATCH ... LIMIT 10` | `SCAN f VIRTUAL TABLE` | 0.5 ms |

### `index.db`

| Query | Plan | Time |
|---|---|---|
| `query_symbols` (name prefix) | `SEARCH symbols USING COVERING INDEX idx_symbols_name_nocase` | - |
| `query_symbols` (kind) | `SEARCH s USING INDEX idx_symbols_kind` + `TEMP B-TREE` | - |
| `query_symbols` (language) | `SEARCH f USING idx_files_language` -> `SEARCH s USING idx_symbols_file` | - |
| `query_symbols` (default, every symbol) | `SCAN symbols USING COVERING INDEX idx_symbols_name` | **108 ms / 45 323 rows** |
| `get_symbol_by_exact_name` | `SEARCH symbols USING INDEX idx_symbols_name (name=?)` | - |
| `find_references_limited` | `SEARCH r USING INDEX idx_refs_symbol` + PK seek + `TEMP B-TREE FOR ORDER BY` | 1.1-7.3 ms |
| `query_all_edges_typed` | `SCAN graph_edges USING COVERING INDEX sqlite_autoindex_graph_edges_1` | **573 ms / 764 916 rows** |
| `query_edges_for_symbol` | `MULTI-INDEX OR` -> `idx_edges_source` + `idx_edges_target` | - |
| `delete_edges_for_symbols` | `MULTI-INDEX OR` -> autoindex + `idx_edges_target` | - |
| `query_all_refs` | `SCAN r` + PK seek + `TEMP B-TREE FOR ORDER BY` | **466 ms / 309 033 rows** |
| `query_all_communities` | `SCAN communities` | 15 ms |

`query_all_edges_typed` (573 ms) is called by `godnodes`, `explain`, `path`,
`communities`, and the JSON/report export - all one-shot user commands, all
under the store mutex. Not listed as a finding because the alternative
(SQL-side degree aggregation) changes the algorithms; note it here so a future
pass knows where the remaining seconds go.

## Appendix C - file and line index

| Concern | Location |
|---|---|
| Main schema + migrations | `crates/ragent-storage/src/storage.rs:807-1197` |
| FTS session query | `crates/ragent-storage/src/storage.rs:4344-4540` |
| FTS warm-up / reconcile | `crates/ragent-storage/src/storage.rs:4644-4790` |
| Memory FTS writes | `storage.rs:2949`, `:3226`, `:3328`, `:3359`, `:3368`, `:3527` |
| Activity-log schema | `crates/ragent-storage/src/activity_log.rs:246-263` |
| Activity-log append | `activity_log.rs:369-470`, `:486-620` |
| Activity-log retention | `activity_log.rs:1372-1395` |
| Code-index schema | `crates/ragent-codeindex/src/store.rs:155-280` |
| `query_symbols` | `store.rs:847-940` |
| `derive_edges_for_file` | `crates/ragent-codeindex/src/graph/edges.rs:328-370` |
| Graph full-build split | `crates/ragent-codeindex/src/graph/edges.rs:404-660` |
| Vault schema + search | `crates/ragent-research/src/source_vault.rs:335-435` |
| Cache schema + eviction | `crates/ragent-tools-extended/src/masterfetch/cache.rs:258-320`, `:660-705` |
