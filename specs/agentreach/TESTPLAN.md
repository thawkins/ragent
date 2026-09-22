---
status: draft
---
# Agent-Reach Integration — Manual Test Plan

This manual test plan validates the `agentreach` specification (`SPEC.md`).
Each test case is designed to be executed by a human tester with access to the
ragent TUI, CLI, HTTP server, and a working build of the `ragent-reach` crate.

Preconditions common to all test cases:
- A debug build of ragent with the `reach` Cargo feature enabled (`cargo build`).
- An in-memory or temporary-file storage backend available for credential
  isolation (tests must never touch the real user `ragent.db`).
- A configured LLM provider (any) for tests that require the agent loop.

## Test Cases

### TC-001 — Reach tools registered and visible by default

**Preconditions:**
- ragent is built with the `reach` feature on (default).
- `tool_visibility.reach` is not set in `ragent.json` (defaults to `true`).

**Steps:**
1. Launch the ragent TUI: `ragent`.
2. Type `/tools` or open the tool-list view.
3. Search for `reach_` in the tool list.

**Test data:** None.

**Expected results:**
- The tool list includes `reach_doctor`, `reach_credentials`, `reach_rss`,
  `reach_twitter_read`, `reach_twitter_search`, `reach_reddit_read`,
  `reach_reddit_search`, `reach_bilibili_search`, `reach_bilibili_detail`,
  `reach_xiaohongshu_read`, `reach_linkedin_read`, `reach_facebook_read`,
  `reach_instagram_read`, `reach_web_read`, `reach_web_search`,
  `reach_github`, `reach_youtube`.
- Each tool reports `permission_category` of `"reach"`.
- No tool is hidden.

---

### TC-002 — Reach tools hidden when `tool_visibility.reach` is false

**Preconditions:**
- `ragent.json` contains `"tool_visibility": { "reach": false }`.

**Steps:**
1. Launch the ragent TUI: `ragent`.
2. Type `/tools` or open the tool-list view.
3. Search for `reach_` in the tool list.

**Test data:** `ragent.json` with `tool_visibility.reach = false`.

**Expected results:**
- No `reach_*` tool appears in the tool list or is offered to the LLM.
- All non-reach tools remain visible and functional.
- No panic or error is logged at startup.

---

### TC-003 — Feature-gated build compiles without the `reach` feature

**Preconditions:**
- A clean checkout of the ragent source.

**Steps:**
1. Run: `cargo build --no-default-features -p ragent-tools-extended` (or the
   workspace equivalent that disables the `reach` feature).
2. Run: `ragent --no-tui run "list available tools"` and inspect the output.

**Test data:** None.

**Expected results:**
- The build compiles with zero errors and zero warnings.
- No `reach_*` tools appear in the agent's tool list.
- All other tools (core, extended, vcs) remain registered and visible.

---

### TC-004 — `reach_doctor` returns per-channel health report

**Preconditions:**
- No platform credentials configured.
- ragent TUI is running.

**Steps:**
1. Invoke `reach_doctor` by typing: `/tool reach_doctor` (or asking the agent
   to call it).

**Test data:** None.

**Expected results:**
- A JSON report is returned with one entry per registered channel.
- Each entry has fields: `channel`, `status`, `active_backend`, `fix_hint`.
- Channels with a no-auth backend (e.g., `reddit_rss`, `bili_api`,
  `jina_reader`) report `status: "healthy (no-auth)"` or `healthy`.
- Channels requiring credentials (e.g., `twitter`, `reddit_oauth`) report
  `status: "unavailable"` with a `fix_hint` naming the missing credential.
- No raw credential value appears anywhere in the output.

---

### TC-005 — Credential masking in all tool output

**Preconditions:**
- A reach credential is stored (e.g., `reach::twitter::bearer_token` with a
  test value).

**Steps:**
1. Invoke `reach_credentials list`.
2. Invoke `reach_doctor`.
3. Invoke any `reach_*` channel tool that returns a response referencing the
   configured credential.

**Test data:** `reach::twitter::bearer_token` = `sk-test-1234567890abcdef`.

**Expected results:**
- `reach_credentials list` shows the key name only, never the value.
- `reach_doctor` shows `configured` or a masked last-4 form (e.g.
  `<masked:cdef>`) for the credential, never the full token.
- No tool output contains the substring `sk-test-1234567890abcdef`.
- No tool output contains more than the last 4 characters of any secret.

---

### TC-006 — `reach_credentials set` persists without echoing the secret

**Preconditions:**
- ragent TUI is running.
- In-memory storage is configured.

**Steps:**
1. Invoke: `reach_credentials set twitter bearer_token sk-test-secret-value`.
2. Invoke: `reach_credentials list`.
3. Invoke: `reach_credentials test twitter`.

**Test data:** Platform = `twitter`, key = `bearer_token`, value =
`sk-test-secret-value`.

**Expected results:**
- Step 1 confirms storage with a message like `"credential stored"` and does
  NOT echo `sk-test-secret-value` in the confirmation.
- Step 2 lists `reach::twitter::bearer_token` but does not show the value.
- Step 3 tests the credential (probe result) without revealing the plaintext.
- The value stored in the `provider_auth` table is encrypted (starts with
  `v2:`), not plaintext.

---

### TC-007 — `reach_credentials delete` removes credential and clears cache

**Preconditions:**
- `reach::twitter::bearer_token` is stored.
- ragent TUI is running.

**Steps:**
1. Invoke: `reach_credentials set twitter bearer_token sk-original`.
2. Invoke: `reach_credentials delete twitter`.
3. Invoke: `reach_credentials list`.
4. Invoke: `reach_doctor` and inspect the `twitter` channel entry.

**Test data:** Platform = `twitter`.

**Expected results:**
- Step 2 confirms deletion.
- Step 3 no longer lists `reach::twitter::bearer_token`.
- Step 4 reports `twitter` as `unavailable` with a `fix_hint` about the
  missing credential.
- A subsequent `get` returns `None`.

---

### TC-008 — Backend fallback on HTTP 401/403

**Preconditions:**
- `reach::twitter::bearer_token` is NOT set (so `twitter_api` returns 401).
- `twitter_guest` backend is available.

**Steps:**
1. Invoke `reach_twitter_read` with a valid tweet ID.

**Test data:** Tweet ID (e.g., a well-known public tweet).

**Expected results:**
- The `twitter_api` backend is attempted first, fails with 401.
- The routing layer automatically falls back to `twitter_guest`.
- The tool returns the tweet content from the guest backend, not an error.
- The output does not indicate a total failure.

---

### TC-009 — All backends fail returns actionable error

**Preconditions:**
- No Twitter credentials are configured.
- The `twitter_guest` backend is unavailable or blocked (simulate by
  disconnecting from the network or pointing at an invalid endpoint).

**Steps:**
1. Invoke `reach_twitter_read` with a tweet ID.

**Test data:** Tweet ID.

**Expected results:**
- The tool returns a `ToolOutput` error.
- The error names the channel (`twitter`).
- The error lists the backends tried (`twitter_api`, `twitter_guest`).
- The error states which credential or configuration is missing.
- The error does not contain any raw credential or internal stack trace.

---

### TC-010 — `reach_rss` parses RSS 2.0, Atom 1.0, and RDF feeds

**Preconditions:**
- Network access to a known RSS feed, an Atom feed, and an RDF/RSS 1.0 feed.

**Steps:**
1. Invoke `reach_rss` with an RSS 2.0 feed URL.
2. Invoke `reach_rss` with an Atom 1.0 feed URL.
3. Invoke `reach_rss` with an RDF/RSS 1.0 feed URL.

**Test data:**
- RSS 2.0: e.g. `https://example.com/rss.xml`
- Atom 1.0: e.g. `https://example.com/atom.xml`
- RDF/RSS 1.0: e.g. `https://example.com/rdf.xml`

**Expected results:**
- Each invocation returns a JSON array of feed items.
- Each item has `title`, `link`, `published`, and `summary` fields.
- No parsing error is returned for any of the three feed formats.
- The response does not include raw XML.

---

### TC-011 — Reddit read falls back from OAuth to RSS

**Preconditions:**
- Reddit `client_id` and `client_secret` are NOT configured.
- Network access is available.

**Steps:**
1. Invoke `reach_reddit_read` with a public post URL.

**Test data:** A public Reddit post URL (e.g.
`https://www.reddit.com/r/rust/comments/...`).

**Expected results:**
- `reddit_oauth` is attempted first, fails (no credentials).
- The routing layer falls back to `reddit_rss` (`.json` suffix).
- The tool returns the post title, body, and comments from the RSS/JSON
  endpoint.
- The output does not contain an authentication error.

---

### TC-012 — Bilibili search works with no credentials

**Preconditions:**
- No Bilibili credentials are configured.
- Network access to `api.bilibili.com` is available.

**Steps:**
1. Invoke `reach_bilibili_search` with a search query.

**Test data:** Query = `"rust programming"`.

**Expected results:**
- The tool returns a JSON array of video results.
- Each result includes title, bvid, uploader, and view count.
- `reach_doctor` reports `bilibili` as `healthy (no-auth)`.

---

### TC-013 — `reach_doctor` preflight timeout

**Preconditions:**
- One channel backend points to an unresponsive endpoint (simulate by
  blocking the port or using a non-routable IP).

**Steps:**
1. Invoke `reach_doctor`.

**Test data:** None.

**Expected results:**
- The unresponsive channel's probe completes within 10 seconds.
- `reach_doctor` does not hang indefinitely.
- The unresponsive channel reports `status: "unavailable"` or `"degraded"`.
- Other channels are still probed and reported.

---

### TC-014 — Credential store uses `StorageBackend` trait only (no raw SQL)

**Preconditions:**
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Search all `.rs` files under `crates/ragent-reach/src/` for the strings
   `CREATE TABLE`, `ALTER TABLE`, `CREATE INDEX`, `rusqlite`, `SELECT `,
   `INSERT `, `DELETE FROM`, `UPDATE `.
2. Inspect the `ReachCredentialStore` implementation for storage calls.

**Test data:** None.

**Expected results:**
- No SQL DDL strings (`CREATE TABLE`, `ALTER TABLE`, `CREATE INDEX`, `CREATE
  VIEW`, `CREATE TRIGGER`, `ADD COLUMN`, `DROP COLUMN`, `migration`,
  `migrate`) appear in any `.rs` file under `crates/ragent-reach/src/`.
- No direct `rusqlite` calls appear in reach code.
- All credential reads/writes/deletes go through `get_provider_auth`,
  `set_provider_auth`, `delete_provider_auth`, `get_setting`, `set_setting`,
  or `delete_setting` on the `StorageBackend` trait.
- The reach crate introduces zero new database schema objects.

---

### TC-015 — No double encryption of credentials

**Preconditions:**
- In-memory storage is configured.
- A known test plaintext is available.

**Steps:**
1. Invoke `reach_credentials set reddit client_secret my-test-secret`.
2. Inspect the raw `api_key` column of the `provider_auth` table in the
   in-memory database (via a debug query or test hook).

**Test data:** `client_secret` = `my-test-secret`.

**Expected results:**
- The stored value starts with `v2:`.
- The stored value does NOT start with `v2:v2:` (which would indicate
  double-encryption).
- A subsequent `get` returns `my-test-secret` exactly.
- The plaintext was passed directly to `set_provider_auth`, not pre-encrypted
  by reach code.

---

### TC-016 — Legacy v1 credential auto-migrates to v2 on first read

**Preconditions:**
- A reach credential is stored in the deprecated v1 `obfuscate_key` format
  (simulate by inserting a v1-obfuscated value directly into the
  `provider_auth` table via a test helper).

**Steps:**
1. Invoke `reach_credentials test <platform>` (or any operation that reads
   the credential).
2. Inspect the stored value in the `provider_auth` table after the read.

**Test data:** A v1-obfuscated credential inserted via test helper.

**Expected results:**
- Step 1 returns the correct decrypted plaintext, not an error.
- Step 2 shows the stored value now starts with `v2:` (auto-migrated).
- No reach-specific migration code was invoked — the migration was performed
  by the storage layer's `get_provider_auth` auto-migration path.

---

### TC-017 — Decryption failure reports `unavailable` with machine-binding hint

**Preconditions:**
- A `v2:`-encrypted credential blob is stored under a key encrypted by a
  different `MACHINE_KEY` (simulate by injecting a `v2:` blob encrypted under
  a different username/home, or by mocking `MACHINE_KEY`).

**Steps:**
1. Invoke `reach_doctor`.
2. Inspect the `fix_hint` for the affected channel.

**Test data:** A mismatched `v2:` credential blob.

**Expected results:**
- The affected channel reports `status: "unavailable"`.
- The `fix_hint` contains the phrases "machine-local", "username", and
  "home directory".
- The `fix_hint` states that credentials set on one machine or under one user
  account cannot be decrypted on another.
- The `fix_hint` instructs the user to re-set the credential via
  `reach_credentials set`.
- The empty plaintext is NOT surfaced as a usable credential to any backend.

---

### TC-018 — Empty credential value rejected on `set`

**Preconditions:**
- In-memory storage is configured.

**Steps:**
1. Invoke `reach_credentials set twitter bearer_token ""` (empty string).
2. Invoke `reach_credentials set twitter bearer_token "   "` (whitespace only).
3. Invoke `reach_credentials set twitter bearer_token "real-secret-value"`.

**Test data:** Empty string, whitespace string, real value.

**Expected results:**
- Step 1 returns an error like `"value is empty; non-empty value required"`.
- Step 2 returns the same error.
- Step 3 succeeds and stores the credential.
- No empty or whitespace-only value is ever written to the `provider_auth`
  table.

---

### TC-019 — No plaintext credential export or `dump` action

**Preconditions:**
- At least one reach credential is stored.
- ragent TUI or CLI is running.

**Steps:**
1. Invoke `reach_credentials` with action `list`.
2. Attempt to invoke `reach_credentials` with action `dump`, `export`,
   `export-all`, or `get-all-credentials`.
3. Invoke `reach_doctor` and inspect all output.

**Test data:** Any stored reach credential.

**Expected results:**
- Step 1 returns key names only, never decrypted values.
- Step 2: no `dump`, `export`, `export-all`, or `get-all-credentials` action
  exists. The tool returns an "unknown action" error for each.
- Step 3: `reach_doctor` output contains no decrypted credential plaintext.
- A source audit confirms no `.rs` file under `crates/ragent-reach/src/`
  contains the strings `dump`, `export_all`, or `get_all_credentials`.

---

### TC-020 — `reach::` namespace isolation enforced

**Preconditions:**
- The `ReachCredentialKey` constructor is available for testing.

**Steps:**
1. Attempt to construct a key `twitter::bearer_token` (no `reach::` prefix).
2. Attempt to construct a key `reach::anthropic::key` (LLM-provider name
   collision).
3. Attempt to construct a key `reach::twitter::a::b` (too many `::`
   separators).
4. Attempt to construct a key `reach::twitter::bearer_token` (valid).

**Test data:** The four keys above.

**Expected results:**
- Step 1 is rejected with an error about the missing `reach::` prefix.
- Step 2 is rejected with an error about provider-name collision.
- Step 3 is rejected with an error about malformed segments.
- Step 4 is accepted and produces a valid `ReachCredentialKey`.
- No reach code reads, writes, or deletes keys outside the `reach::`
  namespace.

---

### TC-021 — Storage `None` returns actionable error, no panic

**Preconditions:**
- A `ToolContext` with `storage: None` is constructed (simulate by building a
  reach registry without a storage handle).

**Steps:**
1. Invoke any `reach_*` tool that requires credentials (e.g.
   `reach_credentials list` or `reach_twitter_read`).

**Test data:** None.

**Expected results:**
- The tool returns an `anyhow::Error` with an actionable message such as
  `"no storage backend configured; cannot access reach credentials"`.
- No `unwrap()` or `panic!()` occurs.
- The process does not crash.
- The error does not contain a raw stack trace.

---

### TC-022 — Reach uses shared `ToolContext.storage`, not its own `Storage::open`

**Preconditions:**
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Search all `.rs` files under `crates/ragent-reach/src/` for the strings
   `Storage::open`, `Storage::open_in_memory`, `open(`.
2. Inspect the `ReachStorageProvider` and `ReachCredentialStore` for how they
   obtain the `Storage` handle.

**Test data:** None.

**Expected results:**
- No production code path under `crates/ragent-reach/src/` calls
  `Storage::open` or `Storage::open_in_memory`.
- The `Storage` handle is obtained from `ToolContext.storage` (the session's
  shared `Arc`).
- Test code under `crates/ragent-reach/tests/` may use
  `Storage::open_in_memory()` but no test resolves the real user `ragent.db`
  path.

---

### TC-023 — Short-lived derived tokens never persisted

**Preconditions:**
- A Reddit OAuth flow is simulated or mocked.
- `reach::reddit::client_id` and `reach::reddit::client_secret` are stored.

**Steps:**
1. Invoke `reach_reddit_read` or `reach_reddit_search` (triggers OAuth token
   minting).
2. Inspect the `provider_auth` table after the call.

**Test data:** Reddit credentials and a valid post/search query.

**Expected results:**
- The tool returns results successfully (the ephemeral access token was
  minted in memory and used).
- The `provider_auth` table contains only `reach::reddit::client_id` and
  `reach::reddit::client_secret`.
- No ephemeral access token row appears in `provider_auth` or `settings`.
- On token expiry, a subsequent call re-derives the token from the persisted
  long-lived material.

---

### TC-024 — Storage errors are sanitized to reach-level errors

**Preconditions:**
- An in-memory storage is configured but the `provider_auth` table is dropped
  or corrupted (simulate via a test hook).

**Steps:**
1. Invoke `reach_credentials list` or any credential operation.

**Test data:** None.

**Expected results:**
- The tool returns an error containing the reach key name (e.g.
  `reach::twitter::bearer_token`).
- The error contains an actionable hint.
- The error does NOT contain the raw `rusqlite` error string, row identifiers,
  or SQL fragments.
- The error is consistent with the FR-006 masking rule.

---

### TC-025 — Credential index self-heals from stale and corrupt states

**Preconditions:**
- The `reach::__credential_index__` settings key is used for enumeration
  (fallback path from FR-029).

**Steps:**
1. Store a credential, then manually delete its `provider_auth` row while
   leaving the index entry (simulate a stale index).
2. Invoke `reach_credentials list`.
3. Write garbage bytes into the `reach::__credential_index__` settings value.
4. Invoke `reach_credentials list`.
5. Store a new credential and invoke `reach_credentials list` again.

**Test data:** Stale index entry, corrupt JSON index.

**Expected results:**
- Step 2: the stale entry is pruned from the index and omitted from the
  result. The result does not include the orphaned key.
- Step 4: a `tracing::warn!` is emitted. The list returns empty (treated as
  if the index were empty).
- Step 5: a valid index is restored. The list returns the newly stored
  credential.
- No crash or panic occurs in any scenario.

---

### TC-026 — `reach_doctor` probes are read-only

**Preconditions:**
- An instrumented in-memory storage that counts mutations (writes/deletes)
  is configured.
- At least one credential is stored.

**Steps:**
1. Record the mutation count.
2. Invoke `reach_doctor`.
3. Record the mutation count again.

**Test data:** None.

**Expected results:**
- The mutation count after `reach_doctor` equals the count before.
- No `set_provider_auth`, `delete_provider_auth`, or `set_setting` call was
  made by any probe.
- The transparent v1-to-v2 migration inside `get_provider_auth` (if any)
  is internal to the storage layer and does not count as a reach-side write.

---

### TC-027 — `ReachCredentialStore` is the sole credential facade

**Preconditions:**
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Search all `.rs` files under `crates/ragent-reach/src/channels/` and
   `crates/ragent-reach/src/backends/` for the strings `provider_auth`,
   `set_setting`, `get_setting`, `set_provider_auth`, `get_provider_auth`,
   `delete_provider_auth`.
2. Inspect the `Storage` / `StorageBackend` handle accessibility outside
   the `credentials` module.

**Test data:** None.

**Expected results:**
- No `.rs` file under `channels/` or `backends/` contains `provider_auth`
  or `set_setting`.
- All credential access goes through `ReachCredentialStore` methods.
- The `Storage` / `StorageBackend` handle is private/inaccessible outside
  the `credentials` module (no public accessor).

---

### TC-028 — Secret-free audit events on credential I/O

**Preconditions:**
- Tracing is enabled with `RUST_LOG=ragent_reach::credentials=debug`.
- A known test secret is stored.

**Steps:**
1. Invoke `reach_credentials set twitter bearer_token sk-known-test-secret`.
2. Invoke `reach_credentials list`.
3. Invoke `reach_credentials delete twitter`.
4. Inspect the tracing log output for `ragent_reach::credentials` events.

**Test data:** `bearer_token` = `sk-known-test-secret`.

**Expected results:**
- Each operation (`set`, `list`, `delete`) emits a `tracing::debug!` event
  with `target = "ragent_reach::credentials"`.
- Each event records the operation type and the `ReachCredentialKey` name.
- No event includes the credential value `sk-known-test-secret`, masked or
  otherwise.
- The events provide an audit trail of credential access without leaking
  secrets.

---

### TC-029 — Startup secret seeding covers reach credentials (no reach-specific seeding)

**Preconditions:**
- A reach credential is stored in the shared `provider_auth` table.
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Start a ragent session (triggers `seed_secret_registry` on the shared
   `Storage`).
2. Produce a log line that would contain the credential plaintext if not
   redacted.
3. Search all `.rs` files under `crates/ragent-reach/src/` for the string
   `seed_secret_registry`.

**Test data:** Any stored reach credential.

**Expected results:**
- Step 2: the credential plaintext is redacted by `ragent_storage::sanitize`
  (it was seeded into the redaction registry at startup by the existing
  `seed_secret_registry` call).
- Step 3: no `.rs` file under `crates/ragent-reach/src/` contains
  `seed_secret_registry`. Reach does not implement its own startup seeding.

---

### TC-030 — Zero SQLite schema footprint in reach crate

**Preconditions:**
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Search all `.rs` files under `crates/ragent-reach/src/` for the SQL strings:
   `CREATE TABLE`, `ALTER TABLE`, `CREATE INDEX`, `CREATE VIEW`,
   `CREATE TRIGGER`, `ADD COLUMN`, `DROP COLUMN`, `migration`, `migrate`.
2. Inspect the database schema before and after a reach credential is stored.

**Test data:** None.

**Expected results:**
- Step 1: none of the listed SQL strings appear in any `.rs` file under
  `crates/ragent-reach/src/`.
- Step 2: the set of tables, columns, indexes, views, and triggers in the
  shared `ragent.db` is identical before and after the reach credential
  operation. Reach uses only the existing `provider_auth` and `settings`
  tables.

---

### TC-031 — Async non-blocking: reach tools do not block the executor

**Preconditions:**
- ragent is running with the agent loop active.
- A slow storage backend or a simulated delay in SQLite is configured.

**Steps:**
1. Invoke a `reach_*` tool that performs credential I/O (e.g.
   `reach_credentials list`).
2. While the tool is executing, observe the TUI for responsiveness (e.g.,
   can the user still type, scroll, or interact?).

**Test data:** None.

**Expected results:**
- The TUI remains responsive during the credential I/O operation.
- The agent-loop executor thread is not blocked.
- The credential operation completes via `spawn_blocking` off the executor.

---

### TC-032 — Non-secret platform config in `settings`, secrets in `provider_auth`

**Preconditions:**
- In-memory storage is configured.

**Steps:**
1. Store a Reddit `client_id` (non-secret config) via the `settings` table
   path.
2. Store a Reddit `client_secret` (secret) via the `provider_auth` table path.
3. Inspect the raw values in both tables.

**Test data:** `client_id` = `my-client-id`, `client_secret` = `my-secret`.

**Expected results:**
- The `client_id` is stored as plaintext in the `settings` table under a
  `reach::reddit::config::client_id` key.
- The `client_secret` is stored encrypted (starts with `v2:`) in the
  `provider_auth` table under a `reach::reddit::client_secret` key.
- The `client_secret` is NOT stored in plaintext anywhere.
- Both values are retrievable by the reach tool.

---

### TC-033 — Test isolation from the real user database

**Preconditions:**
- The `ragent-reach` test suite source is available for inspection.

**Steps:**
1. Inspect every test file under `crates/ragent-reach/tests/`.
2. Verify each test constructs its own storage via `Storage::open_in_memory()`
   or a unique file under `target/temp/`.
3. Search for any reference to the real user `ragent.db` path or the default
   storage path.

**Test data:** None.

**Expected results:**
- No test resolves the default user database path.
- No test opens, reads, or mutates the user's real `ragent.db`.
- Every test uses an isolated in-memory or temporary-file storage.
- A CI-safe guard test asserts the test harness never passes the real
  `ragent.db` path to `Storage::open`.

---

### TC-034 — No new cryptographic code in `ragent-reach`

**Preconditions:**
- The `ragent-reach` source code is available for inspection.

**Steps:**
1. Search all `.rs` files under `crates/ragent-reach/src/` for the strings
   `aes`, `gcm`, `blake3`, `chacha`, `argon2`, `pbkdf`, `scrypt`, `ring::`,
   `openssl`, `Encrypt`, `Cipher`.
2. Inspect the `Cargo.toml` of `ragent-reach` for crypto-related dependencies.

**Test data:** None.

**Expected results:**
- No cryptographic primitive implementation appears in reach source.
- `Cargo.toml` does not list any crypto crate as a direct dependency.
- All encryption is delegated to `ragent-storage` via `set_provider_auth` /
  `get_provider_auth`.
- The at-rest cipher is the v2 blake3-derived keystream (`encrypt_key`)
  implemented in `ragent-storage`, not AES-GCM (NFR-007 clarifies NFR-003).

---

### TC-035 — `reach_web_read` and `reach_web_search` delegate to existing tools

**Preconditions:**
- Network access is available.
- ragent TUI is running.

**Steps:**
1. Invoke `reach_web_read` with a public URL.
2. Invoke `reach_web_search` with a search query.

**Test data:**
- URL = `https://example.com`
- Query = `"rust async"`.

**Expected results:**
- `reach_web_read` returns the extracted page content (same as `mf_fetch`
  would produce).
- `reach_web_search` returns search results (same as `mf_search` would
  produce).
- No new HTTP fetching logic is invoked — the existing `mf_fetch` /
  `mf_search` tools are reused internally.

---

### TC-036 — LinkedIn read via Jina Reader delegate

**Preconditions:**
- Network access to `r.jina.ai` is available.
- No LinkedIn credentials are configured.

**Steps:**
1. Invoke `reach_linkedin_read` with a public LinkedIn profile URL.

**Test data:** A public LinkedIn profile URL.

**Expected results:**
- The tool returns the extracted profile content via the Jina Reader.
- No authentication is required (public pages only).
- `reach_doctor` reports `linkedin` as `healthy (no-auth)` or `healthy`.

---

### TC-037 — `reach_doctor` fix_hint for missing credential is actionable

**Preconditions:**
- No Twitter credentials are configured.

**Steps:**
1. Invoke `reach_doctor`.
2. Inspect the `twitter` channel entry's `fix_hint`.

**Test data:** None.

**Expected results:**
- The `fix_hint` names the missing credential (e.g.
  `"reach::twitter::bearer_token not configured"`).
- The `fix_hint` suggests the action to fix it (e.g.
  `"run reach_credentials set twitter bearer_token <value>"`).
- The `fix_hint` does not contain a raw stack trace or internal error code.

---

### TC-038 — CLI-based backend routing through `bash` validation

**Preconditions:**
- A channel backend that shells out to an external CLI (e.g. `yt-dlp`) is
  configured.
- The `bash` tool's 7-layer security is active.

**Steps:**
1. Invoke the channel tool that uses the CLI backend.
2. Inspect the execution path (logs or debug output) for the command
   invocation.

**Test data:** A valid input for the CLI-based channel.

**Expected results:**
- The external CLI is invoked through the `bash` tool's validated command
  path.
- No direct `std::process::Command` call with user-supplied input is made.
- If the command is on the banned list or matches a denied pattern, it is
  rejected by the bash security layer.

---

### TC-039 — Zero-warnings build and clippy clean

**Preconditions:**
- A clean checkout of the ragent source.

**Steps:**
1. Run: `cargo build -p ragent-reach`.
2. Run: `cargo clippy -p ragent-reach`.
3. Run: `cargo clippy -p ragent-tools-extended`.

**Test data:** None.

**Expected results:**
- `cargo build` completes with zero warnings.
- `cargo clippy` on both crates completes with zero warnings.
- The build matches the zero-warnings policy of all other crates.

---

### TC-040 — In-memory credential cache serves repeated reads without storage I/O

**Preconditions:**
- A reach credential is stored.
- The `ragent-reach` source code is available for inspection or debug
  tracing is enabled.

**Steps:**
1. Invoke a `reach_*` tool that reads the credential (first read — cache
   miss).
2. Invoke the same tool again (second read — cache hit).
3. Inspect the storage I/O count or tracing logs.

**Test data:** Any stored reach credential.

**Expected results:**
- Step 1 reads from storage (cache miss) and populates the in-memory cache.
- Step 2 reads from the in-memory cache (no SQLite lookup or decryption).
- The storage I/O count for step 2 is zero.
- After a `set` or `delete`, the cache is invalidated and the next read goes
  back to storage.

---

### TC-041 — Concurrent credential access is thread-safe

**Preconditions:**
- An in-memory storage with multiple reach credentials stored.
- The ability to spawn multiple concurrent tool invocations (e.g. via the
  HTTP server or multiple TUI sessions).

**Steps:**
1. Spawn 20 concurrent `reach_*` tool invocations that each perform mixed
   `get`, `set`, and `delete` operations on overlapping credential keys.
2. Wait for all to complete.
3. Verify the final state of each credential key.

**Test data:** Multiple credential keys with overlapping access patterns.

**Expected results:**
- All 20 concurrent operations complete without deadlock or panic.
- No data race corrupts a credential value.
- The final `get` for each key returns the last-written value (or `None` if
  the last op was `delete`).
- The in-memory cache `RwLock` does not block the executor thread for
  longer than the in-memory map mutation.

---

### TC-042 — Storage mutex critical sections are bounded

**Preconditions:**
- An in-memory storage is configured.
- Tracing or instrumentation that measures mutex hold time is available.

**Steps:**
1. Perform a compound credential operation (e.g. `set` which writes a row
   AND updates the credential index).
2. Measure the duration the `Storage` connection mutex is held.

**Test data:** None.

**Expected results:**
- The mutex is held only for the duration of each SQL statement, not across
  network I/O, decryption of unrelated keys, or `await` points.
- Compound operations use consecutive short critical sections, not one
  long-held lock.
- A reach mutation does not stall unrelated storage users (memory,
  sessions, Gmail token store).