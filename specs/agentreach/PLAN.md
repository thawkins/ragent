# Agent-Reach Integration — Implementation Plan

This plan implements the `agentreach` specification (`SPEC.md`). Tasks are
ordered so each task's dependencies are completed first. Effort: S = ≤2h,
M = ≤1 day, L = multi-day. All new tasks start `Pending`.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `ragent-reach` crate skeleton | FR-014, FR-018 | S | Critical | Pending | — |
| T-002 | Add `ReachConfig` to `ragent-config` | FR-002, FR-008 | S | Critical | Pending | — |
| T-003 | Add `tool_visibility.reach` switch | FR-002, FR-008 | S | Critical | Pending | T-002 |
| T-004 | Implement `Channel` trait + routing module | FR-003, FR-004, FR-015 | M | Critical | Pending | T-001 |
| T-005 | Wire encrypted credential store delegate | FR-007, NFR-003 | M | Critical | Pending | T-001 |
| T-006 | Implement `reach_credentials` tool | FR-009 | M | Critical | Pending | T-005 |
| T-007 | Implement `reach_doctor` tool | FR-010, NFR-002 | M | Critical | Pending | T-004, T-005 |
| T-008 | Add `register_extracted_reach_tools` in `ragent-agent` | FR-001, FR-011 | S | Critical | Pending | T-007 |
| T-009 | Add `reach` Cargo feature gate on `ragent-tools-extended` | FR-018 | S | High | Pending | T-001 |
| T-010 | Implement `reach_web_read` channel (delegate to mf_fetch) | FR-013 | S | High | Pending | T-004 |
| T-011 | Implement `reach_web_search` channel (delegate to mf_search) | FR-013 | S | High | Pending | T-004 |
| T-012 | Implement `reach_github` channel (delegate to github_* tools) | FR-013 | S | Medium | Pending | T-004 |
| T-013 | Implement `reach_youtube` channel (delegate to masterfetch::youtube) | FR-013 | S | High | Pending | T-004 |
| T-014 | Implement `reach_rss` tool with `feed-rs` | FR-017, NFR-004 | M | High | Pending | T-001, T-004 |
| T-015 | Implement `reach_reddit_read` (reddit_oauth + reddit_rss backends) | FR-003, FR-005, FR-015 | L | High | Pending | T-005, T-004 |
| T-016 | Implement `reach_reddit_search` (reddit_oauth + reddit_rss) | FR-003, FR-005 | M | High | Pending | T-015 |
| T-017 | Implement `reach_twitter_read` (api -> guest backends) | FR-003, FR-015 | L | High | Pending | T-005, T-004 |
| T-018 | Implement `reach_twitter_search` (api -> guest) | FR-003, FR-015 | M | High | Pending | T-017 |
| T-019 | Implement `reach_bilibili_search` (bili_api backend) | FR-005 | M | Medium | Pending | T-004 |
| T-020 | Implement `reach_bilibili_detail` (bili_api backend) | FR-005 | M | Medium | Pending | T-019 |
| T-021 | Implement `reach_xiaohongshu_read` (browser_session backend) | FR-005 | L | Low | Pending | T-004 |
| T-022 | Implement `reach_linkedin_read` (jina_reader delegate) | FR-013 | S | Medium | Pending | T-010 |
| T-023 | Implement `reach_facebook_read` (browser_session backend) | FR-005 | L | Low | Pending | T-021 |
| T-024 | Implement `reach_instagram_read` (browser_session backend) | FR-005 | L | Low | Pending | T-021 |
| T-025 | Implement backend preflight probes + 401/403 fallback | FR-015, NFR-002 | M | High | Pending | T-004 |
| T-026 | Mask all credential output (last-4 or `configured`) | FR-006 | S | Critical | Pending | T-006 |
| T-027 | Enforce `permission_category = "reach"` on all tools | FR-011 | S | Critical | Pending | T-008 |
| T-028 | Route CLI-based backends through `bash` validation layer | FR-012 | M | Medium | Pending | T-004 |
| T-029 | Verify zero-warnings build + clippy clean | FR-014, NFR-001 | S | High | Pending | T-024 |
| T-030 | Update `docs/howtos/tools.md` with Reach category | FR-001 | S | Medium | Pending | T-029 |
| T-031 | Update `SPEC.md`, `README.md`, `QUICKSTART.md`, `CHANGELOG.md` | FR-001 | S | Medium | Pending | T-030 |
| T-032 | Implement `ReachCredentialStore` trait + storage adapter | FR-007, NFR-003 | M | Critical | Pending | T-001 |
| T-033 | Define `ReachCredentialKey` naming scheme (`reach::platform::key`) | FR-007 | S | Critical | Pending | T-005 |
| T-034 | Auto-migrate legacy reach credential keys to namespaced keys | FR-007 | M | Medium | Pending | T-033 |
| T-035 | Implement `ReachStorageProvider` adapter bridging to `ToolContext.storage` | FR-007, NFR-003 | M | Critical | Pending | T-032 |
| T-036 | Cache decrypted reach credentials in-memory with automatic re-read on miss | FR-007, FR-009 | M | High | Pending | T-032 |
| T-037 | Clear reach credential cache on `reach_credentials delete` and `set` | FR-009 | S | High | Pending | T-036 |
| T-038 | Unit-test `ReachCredentialStore` set/get/delete/list round-trip | FR-007, FR-009 | S | High | Pending | T-032 |
| T-039 | Audit reach credential reads to use only `ReachCredentialStore` | FR-006, FR-007, FR-038 | S | High | Pending | T-032, T-037 |
| T-040 | Verify reach credential I/O uses only `StorageBackend` trait methods | FR-019 | S | Critical | Pending | T-032, T-039 |
| T-041 | Return actionable error when `ToolContext.storage` is `None` | FR-020 | S | High | Pending | T-035 |
| T-042 | Store reach credentials via `encrypt_key` (v2) on write | FR-021, NFR-003, NFR-007 | M | Critical | Pending | T-032 |
| T-043 | Register/unregister reach secrets with `ragent_storage::sanitize` | FR-022 | S | High | Pending | T-006, T-026 |
| T-044 | Prohibit plaintext `set_setting`/`get_setting` for reach credentials | FR-023 | S | Critical | Pending | T-032, T-039 |
| T-045 | Verify legacy v1 reach credentials rely on existing auto-migration | FR-024 | S | Medium | Pending | T-032 |
| T-046 | Verify reach passes plaintext to `set_provider_auth` (no double-encrypt) | FR-025 | S | Critical | Pending | T-032, T-042 |
| T-047 | Remove direct `register_secret`/`unregister_secret` calls from reach credential path | FR-026 | S | High | Pending | T-043 |
| T-048 | Use `ToolContext.storage` handle, not `Storage::open`, in reach | FR-027 | S | Critical | Pending | T-035 |
| T-049 | Permit non-secret platform config in `settings` table; keep secrets in `provider_auth` | FR-028 | S | Medium | Pending | T-033, T-044 |
| T-050 | Implement reach credential enumeration via storage layer or namespaced settings index | FR-029 | M | High | Pending | T-032 |
| T-051 | Detect empty-plaintext decryption failure and report `unavailable` in `reach_doctor` | FR-030 | S | High | Pending | T-007, T-032 |
| T-052 | Wrap reach credential SQLite I/O in `spawn_blocking` | NFR-005 | S | High | Pending | T-032 |
| T-053 | Propagate session `ToolContext.storage` into all reach tools at registration | FR-031 | M | Critical | Pending | T-008, T-035 |
| T-054 | Enforce and test `reach::` namespace isolation in `ReachCredentialKey` | FR-032 | S | High | Pending | T-033 |
| T-055 | Isolate all reach credential tests from the real user database | FR-033 | S | High | Pending | T-038 |
| T-056 | Keep derived short-lived tokens in memory only with TTL; never persist | FR-034 | M | High | Pending | T-036, T-015 |
| T-057 | Map storage errors to sanitized reach-level errors | FR-035 | S | High | Pending | T-032, T-026 |
| T-058 | Order row writes before index updates; self-heal stale/corrupt index | FR-036, FR-029 | M | High | Pending | T-050 |
| T-059 | Verify `reach_doctor` probes are read-only against the credential store | FR-037 | S | Medium | Pending | T-007, T-032 |
| T-060 | Bound storage-mutex critical sections in reach credential operations | NFR-006 | S | Medium | Pending | T-052 |
| T-061 | Enforce `ReachCredentialStore` as sole credential facade | FR-038 | M | High | Pending | T-032, T-039 |
| T-062 | Validate non-empty credential value on `set` | FR-039 | S | High | Pending | T-032 |
| T-063 | Emit secret-free structured audit events on credential I/O | FR-040 | S | Medium | Pending | T-032, T-057 |
| T-064 | Correct NFR-003 cipher description to v2 blake3 keystream | NFR-007 | S | Low | Pending | T-042 |
| T-065 | Verify in-memory credential cache is thread-safe under concurrent access | NFR-008 | M | High | Pending | T-036, T-037 |
| T-066 | Rely on existing `seed_secret_registry` for reach credential startup seeding | FR-041 | S | High | Pending | T-032, T-048 |
| T-067 | Audit reach crate for zero SQLite schema mutations | FR-042, NFR-009 | S | Critical | Pending | T-032 |
| T-068 | Surface machine-binding property in `reach_doctor` decryption-failure fix_hint | FR-043 | S | Medium | Pending | T-051 |
| T-069 | Prohibit plaintext reach credential export and `dump`/`export-all` actions | FR-044 | S | High | Pending | T-006, T-026 |

## Task Detail

### T-001 — Create `ragent-reach` crate skeleton
Create `crates/ragent-reach/` with `Cargo.toml` (member of workspace), `src/lib.rs`
exporting `create_reach_registry() -> ToolRegistry`, and the module tree from
the spec's architecture diagram. Empty stubs for each channel/backend file.

### T-002 — Add `ReachConfig` to `ragent-config`
Add a `ReachConfig` struct to `ragent-config::config` with per-platform
optional credential references (env-var indirection like `env:TWITTER_TOKEN`)
and a `channels` sub-map for enabling/disabling individual channels. Wire into
the top-level `Config` deserialisation.

### T-003 — Add `tool_visibility.reach` switch
Add `reach: bool` field to `ToolVisibilityConfig` and `ToolVisibilitySpecified`
(default `true`). Add to `iter_switches()`. Update the hidden-tools computation
in `ragent-agent/src/dry_run.rs` to map the `reach` family to `reach_*` names.

### T-004 — Implement `Channel` trait + routing module
Define `trait Channel { fn name(); fn backends(); fn execute(). }` and a
`Router` that probes backends in order. Preflight = lightweight HTTP OPTIONS
or auth check. On 401/403/rate-limit, advance to next backend (FR-015).

### T-005 — Wire encrypted credential store delegate
Add `reach::credentials::CredentialStore` that delegates get/set/list to
`ragent-storage`'s encrypted store under a `reach::<platform>` namespace.
Ensure 0600 file perms come from storage layer (verify, do not re-implement).
This task creates the public `ReachCredentialStore` facade; T-032 adds the
trait + adapter that it delegates to.

### T-006 — Implement `reach_credentials` tool
Tool with actions: `set <platform> <key> <value>`, `list`, `test <platform>`,
`delete <platform>`. `set` does not echo the value back (FR-009). Uses
permission category `"reach"`. Clears the in-memory credential cache on
`set`/`delete` (T-037).

### T-007 — Implement `reach_doctor` tool
Iterates all registered channels, runs each backend's preflight with a 10s
timeout (NFR-002), returns JSON: `[{channel, status, active_backend,
fix_hint}]`. Masks credentials (FR-006).

### T-008 — Add `register_extracted_reach_tools` in `ragent-agent`
In `ragent-agent/src/tool/mod.rs`, add `fn register_extracted_reach_tools()`
following the exact pattern of `register_extracted_extended_tools()`, and call
it from `create_default_registry()`. Add `ExtractedReachToolAdapter` mirroring
`ExtractedExtendedToolAdapter`.

### T-009 — Add `reach` Cargo feature gate
Gate the `ragent-reach` dependency behind a `reach` feature on
`ragent-tools-extended` (default on). When off, `create_reach_registry()`
returns an empty registry (FR-018).

### T-010 — `reach_web_read` channel (delegate to `mf_fetch`)
Thin `Channel` impl that calls the existing `mf_fetch` tool internally. Passes
through the URL and returns extracted content. No new HTTP logic.

### T-011 — `reach_web_search` channel (delegate to `mf_search`)
Thin `Channel` impl that calls the existing `mf_search` tool internally. Passes
through the query and returns search results.

### T-012 — `reach_github` channel (delegate to `github_*` tools)
Thin `Channel` impl that calls the existing `github_*` read-only tool subset
internally (e.g., `github_get_issue`, `github_list_prs`).

### T-013 — `reach_youtube` channel (delegate to `masterfetch::youtube`)
Thin `Channel` impl that calls
`masterfetch::youtube::extract_transcript_from_watch_page` internally. Returns
the transcript text.

### T-014 — `reach_rss` with `feed-rs`
Add `feed-rs` to workspace `Cargo.toml`. Parse feed URL via `reqwest`, pass
bytes to `feed_rs::parser`, return items as JSON array
(`title`, `link`, `published`, `summary`).

### T-015 — `reach_reddit_read` (reddit_oauth + reddit_rss backends)
Backend list `[reddit_oauth, reddit_rss]`. `reddit_oauth` obtains an app-only
token via client_credentials grant. `reddit_rss` appends `.json` to the post
URL (no auth). Routing falls back from oauth to rss on 401/403 (FR-015).

### T-016 — `reach_reddit_search` (reddit_oauth + reddit_rss)
Same backend list as T-015 but targets the search endpoint. `reddit_oauth`
searches via the authenticated API; `reddit_rss` uses the `.json` search
suffix.

### T-017 — `reach_twitter_read` (api -> guest backends)
Backend list `[twitter_api, twitter_guest]`. `twitter_api` uses API v2
`GET /2/tweets/:id` with bearer token. `twitter_guest` uses the guest-token
flow (syndication endpoint). Falls back from api to guest on 401/403 (FR-015).

### T-018 — `reach_twitter_search` (api -> guest)
`twitter_api` uses `GET /2/tweets/search/recent`. `twitter_guest` uses the
guest search endpoint. Same fallback logic as T-017.

### T-019 — `reach_bilibili_search` (bili_api backend)
`bili_api` uses `https://api.bilibili.com/x/web-interface/search/type`. No auth
required (FR-005). Returns video title, bvid, uploader, view count.

### T-020 — `reach_bilibili_detail` (bili_api backend)
`bili_api` uses `https://api.bilibili.com/x/web-interface/view?bvid=`. No auth
required (FR-005). Returns title, description, duration, tags. Subtitles
deferred per Open Question 3.

### T-021 — `reach_xiaohongshu_read` (browser_session backend)
Uses the existing `browser` tool to open a note URL in a Chrome session where
the user is already logged in, reads the rendered DOM via CDP `snapshot`.

### T-022 — `reach_linkedin_read` (jina_reader delegate)
Calls `https://r.jina.ai/<linkedin_url>` via the existing `mf_fetch` HTTP
path. No auth (public pages only).

### T-023 — `reach_facebook_read` (browser_session backend)
Same browser-session pattern as T-021. Lower priority (Open Question
acknowledges login-gated complexity).

### T-024 — `reach_instagram_read` (browser_session backend)
Same browser-session pattern as T-021. Lower priority (Open Question
acknowledges login-gated complexity).

### T-025 — Preflight + 401/403 fallback
Implement the `Router::execute_with_fallback` method: run primary backend; if
it returns `401`, `403`, or a known rate-limit status, try next backend. Each
preflight probe has a 10s timeout (NFR-002).

### T-026 — Credential masking
Audit every `ToolOutput` produced by `reach_*` tools. Replace any token/cookie
substring with `<masked:...last4>`. Add a helper `mask_secret(&str) -> String`
in `credentials.rs` and use it in `reach_doctor` and `reach_credentials list`.

### T-027 — Permission category enforcement
Verify every tool returned by `create_reach_registry()` reports
`permission_category() == "reach"`. Update `is_hardwired_auto_approved_tool`
if reach tools should be auto-approved (decision: not auto-approved — they
perform network I/O; respect user permission rules like `web`/`http_request`).

### T-028 — CLI backend routing through `bash`
For any backend that shells out (e.g., `yt-dlp`), invoke via the `bash` tool's
validated command path, never raw `std::process::Command` with user input
(FR-012). Document which channels use this path.

### T-029 — Build + clippy verification
Run `cargo build`, `cargo clippy -p ragent-reach`, `cargo clippy -p
ragent-tools-extended`. Fix all warnings. Verify feature-off build
(`--no-default-features` path for `reach`) still compiles (FR-018).

### T-030 — Update `docs/howtos/tools.md`
Add a "Reach" row to the Category Index and a `## 25. Reach` section listing
all `reach_*` tools with schema examples, matching the format of existing
categories.

### T-031 — Update root docs
Update `SPEC.md` tool count, `README.md` feature list (add "Platform reach"
bullet), `QUICKSTART.md` with a `reach_doctor` example, and `CHANGELOG.md`
entry under the next version heading.

### T-032 — Implement `ReachCredentialStore` trait + storage adapter
Define `reach::credentials::ReachCredentialStore` — a concrete struct wrapping
a `dyn ragent_tools_vcs::storage::StorageBackend` (or the equivalent
`ToolContext.storage` trait). Implement `set`, `get`, `delete`, `list_names`
methods that forward to `set_provider_auth` / `get_provider_auth` /
`delete_provider_auth` / `set_setting` / `get_setting` / `delete_setting`
under namespaced keys. This is the real implementation behind the T-005
facade; it must call `encrypt_key` / `decrypt_key` via the storage layer (no
new crypto in `ragent-reach`, NFR-003, NFR-007). All credentials are stored in
the existing `ragent-storage` encrypted credential store keyed by platform
name (FR-007).

### T-033 — Define `ReachCredentialKey` naming scheme
Establish the canonical key format `reach::<platform>::<key>` (e.g.
`reach::twitter::bearer_token`, `reach::reddit::client_id`). Document it in
`credentials.rs` and add a constructor `ReachCredentialKey::new(platform, key)`
that validates the components contain no `::` separators. All T-005 callsites
use this type instead of raw strings. Implements the "keyed by platform name"
requirement of FR-007.

### T-034 — Auto-migrate legacy reach credential keys to namespaced keys
On first read of any reach credential, if the key is found without the
`reach::` prefix (legacy format from an earlier dev build), re-store it under
the namespaced key and delete the old entry. Log a one-time `info!` message.
Guard with a `migrated` flag in the settings table so it only runs once.
Ensures consistency with the FR-007 platform-keyed storage scheme.

### T-035 — Implement `ReachStorageProvider` adapter bridging to `ToolContext.storage`
Add `reach::credentials::ReachStorageProvider` that adapts the
`Option<Arc<dyn StorageBackend>>` from `ToolContext` into the
`ReachCredentialStore`. Handle the `None` case (no storage configured) by
returning an actionable error from all credential operations. This lets reach
tools obtain credentials without a direct `ragent-storage` dependency at the
call site. All storage goes through the existing encrypted credential store
(FR-007) with no new cryptographic code (NFR-003, NFR-007).

### T-036 — Cache decrypted reach credentials in-memory with automatic re-read on miss
Add an in-memory `HashMap<ReachCredentialKey, String>` behind a
`tokio::sync::RwLock` (NFR-008) in `ReachCredentialStore`. On `get`, check the
cache first; on miss, read from storage, decrypt, populate the cache, and
return. This avoids repeated SQLite lookups + decryption for the same
credential within a session. Underlying storage remains the encrypted
credential store (FR-007) and `set` confirms storage without echoing the
secret (FR-009).

### T-037 — Clear reach credential cache on `reach_credentials delete` and `set`
When `reach_credentials` performs a `set` or `delete` action, invalidate the
specific key (or all keys for the platform on `delete <platform>`) in the
in-memory cache so the next read picks up the new value. This prevents stale
credentials from being served after a mid-session reconfiguration. `set`
persists the credential and confirms without echoing (FR-009). Cache
invalidation acquires a write lock on the `RwLock` (NFR-008).

### T-038 — Unit-test `ReachCredentialStore` set/get/delete/list round-trip
Write manual unit tests (in `crates/ragent-reach/tests/`) that create an
in-memory `Storage::open_in_memory()`, wrap it in `ReachCredentialStore`,
and verify: `set` then `get` returns the same value; `list_names` returns
the key; `delete` removes it; `get` after `delete` returns `None`. Verify
that the stored value in the DB is encrypted (not plaintext). Confirms
FR-007 (encrypted credential store) and FR-009 (persist credential).

### T-039 — Audit reach credential reads to use only `ReachCredentialStore`
Grep/audit all `reach_*` tool implementations and backend files for any direct
`get_provider_auth` / `get_setting` calls that bypass the
`ReachCredentialStore`. Replace them with `ReachCredentialStore::get`. This
ensures all reads go through the cache + masking layer (FR-006) and that no
backend accidentally invents its own key-naming scheme outside the FR-007
platform-keyed encrypted store. This task also enforces FR-038: the
`ReachCredentialStore` is the sole credential facade.

### T-040 — Verify reach credential I/O uses only `StorageBackend` trait methods
Audit the `ReachCredentialStore` implementation to confirm that all
credential reads, writes, and deletes go through the `StorageBackend`
trait's `get_provider_auth`/`set_provider_auth`/`delete_provider_auth`
methods. Verify there are no direct SQLite queries, raw `rusqlite` calls,
or parallel encryption implementations in the reach crate. This ensures
FR-019 compliance — all credential I/O is mediated by the existing storage
trait abstraction.

### T-041 — Return actionable error when `ToolContext.storage` is `None`
In the `ReachStorageProvider` adapter (T-035), when `ToolContext.storage` is
`None`, all credential operations (`get`, `set`, `delete`, `list_names`)
must return an `anyhow::Error` with an actionable message such as
`"no storage backend configured; cannot access reach credentials"`.
Verify that no `unwrap()` or `panic!()` is used on the `None` path. This
implements FR-020 — graceful degradation when storage is unavailable.

### T-042 — Store reach credentials via `encrypt_key` (v2) on write
Ensure the `ReachCredentialStore::set` path passes the plaintext credential
to `set_provider_auth`, which internally applies the v2 blake3-derived
keystream (`encrypt_key`) encryption (NFR-007). Verify via a round-trip test
that the stored DB value starts with `v2:` prefix. This ensures FR-021
compliance — credentials are encrypted at rest with the current v2 scheme,
not the deprecated v1 obfuscation. See T-046 for the no-double-encrypt
verification.

### T-043 — Register/unregister reach secrets with `ragent_storage::sanitize`
When `ReachCredentialStore::set` stores a credential, call
`ragent_storage::sanitize::register_secret(&plaintext_value)` so the
value is added to the in-memory redaction registry. When `delete` removes
a credential, call `ragent_storage::sanitize::unregister_secret(&value)`
to remove it from the registry. This ensures that any accidental logging
of the plaintext value is automatically redacted by the existing sanitize
layer. Implements FR-022.

### T-044 — Prohibit plaintext `set_setting`/`get_setting` for reach credentials
Audit the `ReachCredentialStore` implementation and all reach backend code
to confirm that the plaintext `settings` table (`set_setting`/`get_setting`)
is never used for credential storage. Credentials must only go through the
`provider_auth` table (via `set_provider_auth`/`get_provider_auth`), which
applies encryption. Add a code comment or lint guard in `credentials.rs`
documenting this prohibition. This implements FR-023.

### T-045 — Verify legacy v1 reach credentials rely on existing auto-migration
Verify that if a reach credential was stored using the deprecated v1
`obfuscate_key` format (via `set_provider_auth`), the existing
`get_provider_auth` auto-migration path transparently upgrades it to v2
on first read. Write a manual test that stores a credential with
`obfuscate_key`, reads it via `ReachCredentialStore::get`, and verifies
the returned plaintext is correct. Then verify the DB value has been
updated to the `v2:` prefixed format. This confirms FR-024.

### T-046 — Verify reach passes plaintext to `set_provider_auth` (no double-encrypt)
Audit the `ReachCredentialStore::set` implementation. Confirm it calls
`set_provider_auth(key, &plaintext_value)` and does NOT call
`encrypt_key(&plaintext_value)` first. `set_provider_auth` internally invokes
`obfuscate_key`, which delegates to `encrypt_key`, producing a single `v2:`
prefix on the stored value. Add a round-trip test: store a known plaintext
via `ReachCredentialStore::set`, read the raw `api_key` column from the
`provider_auth` table, assert it starts with `v2:` and does NOT start with
`v2:v2:` (which would indicate double-encryption), then read it back via
`ReachCredentialStore::get` and assert the plaintext matches. If T-042
(pre-encrypt with `encrypt_key`) was implemented, revert that pre-encryption
step. This corrects FR-025.

### T-047 — Remove direct `register_secret`/`unregister_secret` calls from reach credential path
Audit `ReachCredentialStore::set` and `delete` for calls to
`ragent_storage::sanitize::register_secret` / `unregister_secret`.
`set_provider_auth` already calls `register_secret` (storage.rs:1250) and
`delete_provider_auth` already calls `unregister_secret` (storage.rs:1273).
Remove any reach-side duplicates so the secret is registered exactly once.
Add a comment in `credentials.rs` documenting that the storage layer owns
sanitize registration for `provider_auth` entries. If T-043 added these
calls, remove them from the `set`/`delete` paths (T-043's audit of masking
output remains valid). This implements FR-026.

### T-048 — Use `ToolContext.storage` handle, not `Storage::open`, in reach
In `ReachStorageProvider` (T-035), confirm the `Storage` handle comes from
`ToolContext.storage: Option<Arc<crate::storage::Storage>>` (ragent-agent) or
`Option<Arc<dyn StorageBackend>>` (ragent-tools-extended/vcs), downcast/adapted
to the reach storage trait. Ensure no reach code path calls
`Storage::open(path)` or `Storage::open_in_memory()` outside of tests.
Contrast with `ragent_tools_extended::gmail::SqliteTokenStore::shared()`,
which opens its own connection — that pattern is explicitly rejected for
reach. Add a code comment in `credentials.rs` citing FR-027. This ensures a
single connection, consistent sanitize state, and no WAL lock contention.

### T-049 — Permit non-secret platform config in `settings` table; keep secrets in `provider_auth`
Define a reach-internal classification of per-platform data into "secret"
(client_secret, bearer_token, refresh_token, cookie, password) and
"non-secret config" (client_id, instance_url, locale, user_agent). Secret
fields MUST be stored via `set_provider_auth` under a
`reach::<platform>::<field>` key (T-033). Non-secret config fields MAY be
stored via `set_setting` under a `reach::<platform>::config::<field>` key.
Update the FR-023 prohibition comment (T-044) to carve out this exception: the
`settings` table is prohibited for *secrets*, not for non-secret config. Add
a unit test storing a Reddit `client_id` via `set_setting` and a
`client_secret` via `set_provider_auth`, then verify the secret is encrypted
at rest and the config is plaintext. This implements FR-028.

### T-050 — Implement reach credential enumeration via storage layer or namespaced settings index
Implement `ReachCredentialStore::list_names() -> Vec<ReachCredentialKey>`.
Preferred path: add a `list_provider_auth_keys() -> Result<Vec<String>>`
method to the `StorageBackend` trait (ragent-tools-vcs and
ragent-tools-extended), backed by `ragent_storage::Storage` issuing
`SELECT provider_id FROM provider_auth` internally, then filter to
`reach::` prefixed keys. Fallback path (if extending the trait is too
invasive): maintain a reach-owned JSON array index under the settings key
`reach::__credential_index__`, updated atomically inside
`ReachCredentialStore::set` / `delete`. `list_names` reads and parses the
index. The index contains key names only — it is not a credential and does
not violate FR-023. Wire `list_names` into the `reach_credentials list`
action and `reach_doctor`. Add a unit test: set three reach credentials,
`list_names` returns exactly those three keys, delete one, `list_names`
returns the remaining two. This implements FR-029.

### T-051 — Detect empty-plaintext decryption failure and report `unavailable` in `reach_doctor`
In `ReachCredentialStore::get`, if `get_provider_auth` returns `Some(s)`
where `decrypt_key(&s)` would return an empty string (the v2 payload failed
to decode), convert the result to `None` and record a
`DecryptionFailure` marker. In `reach_doctor`, for any channel whose required
credential returns `DecryptionFailure`, emit a report entry with
`status: "unavailable"`, `active_backend: "none"`, and `fix_hint:
"credential was encrypted on a different machine or user, or the database
row is corrupt; re-set the credential on this machine"`. Add a test: store a
credential on machine A, copy the `provider_auth` row to an in-memory DB
whose `MACHINE_KEY` differs (mock by storing a `v2:` blob encrypted under a
different key), call `reach_doctor`, assert the channel reports
`unavailable` with the `fix_hint`. This implements FR-030 and documents the
machine-local key-binding property of `encrypt_key`.

### T-052 — Wrap reach credential SQLite I/O in `spawn_blocking`
Audit every `ReachCredentialStore` method (`get`, `set`, `delete`,
`list_names`) and every reach backend that calls `get_provider_auth` /
`set_provider_auth` / `delete_provider_auth` from an `async fn`. Wrap each
blocking storage call in `tokio::task::spawn_blocking(move || { ... })` and
`.await?` the `JoinHandle`. This prevents the `Storage` `Mutex<Connection>`
lock from stalling the agent-loop executor. Use the existing workspace
pattern (search for `spawn_blocking` in `ragent-storage` / `ragent-agent`
for reference). Add a test that asserts credential reads complete within a
reasonable time even when the storage mutex is held briefly by another
task. This implements NFR-005.

### T-053 — Propagate session `ToolContext.storage` into all reach tools at registration
In `register_extracted_reach_tools` (T-008), thread the session's
`ToolContext.storage` handle (the shared `Arc` — FR-027) through
`create_reach_registry()` so every reach tool receives the same storage
instance as the rest of the session. Do NOT open a fallback
`Storage::open(path)` when the slot is `None`; instead let the FR-020
actionable error surface at invocation time. Add a registration-time test
that builds a registry with a storage-equipped `ToolContext` and asserts
every registered reach tool observes `storage.is_some()`, plus a second test
asserting a `None`-storage registry still constructs successfully (degrading
per FR-020 rather than panicking — FR-031).

### T-054 ��� Enforce and test `reach::` namespace isolation in `ReachCredentialKey`
Move the `reach::` prefix check into `ReachCredentialKey::new` so it becomes
impossible to construct a reach credential key outside the namespace: reject
keys not starting with `reach::`, reject platform segments that collide with
known LLM-provider ids (`anthropic`, `openai`, `gemini`, `ollama`, etc.), and
reject empty segments or embedded `::` beyond the two separators (extends
T-033). Audit `ReachCredentialStore` to confirm every storage call uses a
validated `ReachCredentialKey` and never a raw string. Unit tests: valid key
accepted; `twitter::bearer` without prefix rejected; `reach::anthropic::x`
rejected as a provider-name collision; `reach::twitter::a::b` rejected
(FR-032).

### T-055 — Isolate all reach credential tests from the real user database
Audit every test in `crates/ragent-reach/tests/` (and any reach credential
test elsewhere) to confirm it constructs its own storage via
`Storage::open_in_memory()` or a unique file under `target/temp/`, and that
no test resolves the default user database path. Add a CI-safe guard test
that asserts the test harness never passes the real `ragent.db` path to
`Storage::open`. Remove any test fixture that seeds or asserts against
pre-existing real `provider_auth` rows (FR-033).

### T-056 — Keep derived short-lived tokens in memory only with TTL; never persist
Introduce a small `EphemeralTokenCache` in `ragent-reach` (e.g.,
`RwLock<HashMap<&'static str /*backend*/, (String, Instant)>>`) used by the
`reddit_oauth` (T-015) and `twitter_guest` (T-017) backends for
runtime-minted access/guest tokens. Entries carry a TTL derived from the
provider's `expires_in` (default 50 minutes for Reddit app-only tokens) and
are re-derived from the persisted long-lived material on expiry. Assert by
inspection and test that no code path passes an ephemeral token to
`set_provider_auth` or `set_setting`: add a regression test that runs a
mocked Reddit OAuth flow and verifies the `provider_auth` table afterwards
contains only the `client_id`/`client_secret` keys, not the access token
(FR-034).

### T-057 — Map storage errors to sanitized reach-level errors
Add a `map_storage_err(platform, key, err) -> anyhow::Error` helper in
`credentials.rs` that converts `ragent_storage`/rusqlite errors into messages
of the form `"reach credential <reach::platform::key> could not be <op>:
<actionable hint>"`, deliberately dropping the inner error's Display text
(which may echo row identifiers or payload fragments). Use it at every
credential I/O call site in `ReachCredentialStore`. Unit test: force a
storage failure (e.g., drop the table in the in-memory DB), invoke `get`,
and assert the returned error contains the key name but not the raw rusqlite
message (FR-035, complementing the FR-006 masking helper from T-026).

### T-058 — Order row writes before index updates; self-heal stale/corrupt index
In `ReachCredentialStore` (built on T-050's index): on `set`, commit the
`provider_auth` row first, then read-modify-write the
`reach::__credential_index__` settings value; on `delete`, remove the row
first, then the index entry. In `list_names`, verify each index entry against
`get_provider_auth`: entries whose row is missing are pruned from the index
(written back once) and omitted from the result; unparseable index JSON is
logged via `tracing::warn!`, treated as empty, and rewritten on the next
mutation. Unit tests: (1) index contains `K` but row absent → `list_names`
returns `[]` and index rewritten without `K`; (2) write garbage bytes into
the index → `list_names` returns `[]`, a warn is emitted, subsequent `set`
restores a valid index; (3) kill-ordering test simulating a crash between row
write and index update leaves the store listable and consistent after one
`set` re-run (FR-036).

### T-059 — Verify `reach_doctor` probes are read-only against the credential store
Audit every channel/backend preflight probe invoked by `reach_doctor` (T-007)
and assert it performs, at most, `get_provider_auth` / `get_setting` reads —
no `set_provider_auth`, `delete_provider_auth`, or `set_setting` calls.
Enforce mechanically where feasible: hand probes a read-only credential view
(a `&dyn ReachCredentialReader` with only `get`/`list_names`) instead of the
full store. Regression test: run `reach_doctor` against an instrumented
in-memory storage that counts mutations, and assert the mutation count is
zero after the run (the FR-024 in-`get_provider_auth` migration is internal
to the storage layer and exempt) (FR-037).

### T-060 — Bound storage-mutex critical sections in reach credential operations
Review every `spawn_blocking` closure added by T-052: each closure must
perform its SQL statement(s) and return without network calls, nested
`spawn_blocking`, or awaits; credential decryption outside the storage layer
happens after the closure returns. Compound operations (`set` + FR-029 index
update) must use two sequential `spawn_blocking` calls, never a single
closure that holds the connection across both mutations. Add a doc comment
in `credentials.rs` stating the NFR-006 invariant, and a stress test that
runs 50 concurrent mixed credential ops against an in-memory store and
asserts all complete (no deadlock) within a generous wall-clock budget.

### T-061 — Enforce `ReachCredentialStore` as sole credential facade
Audit every channel backend (`channels/*.rs`), the routing module
(`routing.rs`), and the `reach_credentials` / `reach_doctor` tools for any
direct call to `get_provider_auth`, `set_provider_auth`,
`delete_provider_auth`, `get_setting`, or `set_setting` that bypasses
`ReachCredentialStore`. Replace each with the corresponding
`ReachCredentialStore` method. Add a compile-time guard: make the
`Storage` / `StorageBackend` handle inaccessible outside the
`credentials` module (e.g., private field, no public accessor) so backends
physically cannot reach the storage layer except through the store. Add a
grep-based CI test that asserts no `.rs` file under `channels/` or `backends/`
contains `provider_auth` or `set_setting`. Implements FR-038.

### T-062 — Validate non-empty credential value on `set`
In `ReachCredentialStore::set`, before calling `set_provider_auth`, check that
the supplied value `trim()` is non-empty. If empty, return
`anyhow::Error` such as `"reach credential <key> rejected: value is empty;
non-empty value required (an empty stored value collides with the
decryption-failure sentinel, see FR-030)"`. Do not call `set_provider_auth`.
Add a unit test: `set` with `""` and `"   "` both return `Err`; `set` with a
real value succeeds and `get` returns it. This prevents FR-030's empty-plaintext
sentinel from being produced by a write rather than a decryption failure.
Implements FR-039.

### T-063 — Emit secret-free structured audit events on credential I/O
Add a `tracing::debug!` call at the entry of each `ReachCredentialStore` method
(`get`, `set`, `delete`, `list_names`) with `target =
"ragent_reach::credentials"` and fields `op = "get"|"set"|"delete"|"list"` and
`key = <ReachCredentialKey as Display>`. Never include the `value` field. Run
the existing workspace lint that rejects `value` as a tracing field name if
present, or add a `#[deny]`-style code comment. Add a test that captures
`tracing` events into a test subscriber, performs a `set` + `get` + `delete`
cycle, and asserts each event carries the key name but no value substring of
the known test secret. Complements FR-006 masking and FR-035 error
sanitisation by providing an access trail. Implements FR-040.

### T-064 — Correct NFR-003 cipher description to v2 blake3 keystream
Update the NFR-003 prose in `SPEC.md` (and any code comment in
`credentials.rs` that echoes "AES-GCM") to state that the at-rest cipher is the
v2 blake3-derived keystream (`encrypt_key`) already implemented in
`ragent-storage`, not AES-GCM. Add a cross-reference to FR-021. No code change
required — this is a documentation correctness fix ensuring future
contributors do not attempt to add an AES-GCM path. Verify T-042 and T-046
(which test the `v2:` prefix round-trip) still pass unchanged. Implements
NFR-007.

### T-065 — Verify in-memory credential cache is thread-safe under concurrent access
Review the `RwLock<HashMap<ReachCredentialKey, String>>` cache from T-036.
Confirm that `get` acquires a read lock only for the HashMap lookup (no SQLite
or network I/O inside the read guard), and that `set` / `delete` (T-037) acquire
a write lock for the cache mutation only, with any `spawn_blocking` storage I/O
happening outside the lock. Add a stress test: spawn 20 async tasks that each
perform 50 mixed `get` / `set` / `delete` operations on overlapping keys
against an in-memory `Storage`, then assert all complete without deadlock or
panic and that the final `get` for each key returns the last-written value
(or `None` if last op was `delete`). This formalises the thread-safety of the
T-036 cache and the T-037 invalidation path. Implements NFR-008.

### T-066 — Rely on existing `seed_secret_registry` for reach credential startup seeding
Audit the `ragent-reach` crate and the `register_extracted_reach_tools`
adapter (T-008) to confirm that neither calls, wraps, or duplicates
`ragent_storage::Storage::seed_secret_registry`. Reach credentials are stored
in the same `provider_auth` table as all other provider credentials, so the
existing startup seeding pass — which iterates every `provider_auth` row,
decrypts it, and feeds the plaintext into `crate::sanitize::seed_secrets` —
already covers `reach::`-namespaced keys. Add a test that stores a reach
credential via `ReachCredentialStore::set`, calls `seed_secret_registry` on
the shared `Storage`, and asserts the credential plaintext is registered with
`ragent_storage::sanitize` (i.e., `redact_secrets` masks it in a sample log
line). Add a grep-based assertion that no `.rs` file under
`crates/ragent-reach/src/` contains the string `seed_secret_registry`. This
confirms reach credentials are covered by the existing ragent encrypted
credential store's startup seeding with zero reach-specific seeding code.
Implements FR-041.

### T-067 — Audit reach crate for zero SQLite schema mutations
Grep all `.rs` files under `crates/ragent-reach/src/` for the SQL strings
`CREATE TABLE`, `ALTER TABLE`, `CREATE INDEX`, `CREATE VIEW`, `CREATE TRIGGER`,
`ADD COLUMN`, `DROP COLUMN`, `migration`, and `migrate`. Assert that none
appear. Confirm that the `ReachCredentialStore` (T-032) and all reach backends
interact with the database exclusively through the `StorageBackend` trait
methods (`get_provider_auth`, `set_provider_auth`, `delete_provider_auth`,
`get_setting`, `set_setting`, `delete_setting`) which operate on the existing
`provider_auth` and `settings` tables. Add a CI test that fails the build if
any schema-mutation SQL string is introduced into the reach crate. This
ensures reach has zero schema footprint in the shared encrypted credential
store, leaving schema ownership entirely with `ragent-storage`. Implements
FR-042 and NFR-009.

### T-068 — Surface machine-binding property in `reach_doctor` decryption-failure fix_hint
Extend the FR-030 decryption-failure path in `reach_doctor` (T-051) so the
`fix_hint` string explicitly states: (a) the credential store uses a
machine-local encryption key derived from the current user's username and home
directory, (b) credentials set on one machine or under one user account cannot
be decrypted on another, and (c) the user must re-set the credential on the
current machine via `reach_credentials set`. Add a test that forces a
decryption failure (store a `v2:` blob encrypted under a different
`MACHINE_KEY` by mocking or injecting a mismatched key), runs `reach_doctor`,
and asserts the `fix_hint` contains the phrases "machine-local", "username",
and "home directory". This surfaces the inherent machine-binding property of
the existing `ragent-storage` v2 `encrypt_key` scheme so the user understands
the failure cause. Implements FR-043.

### T-069 — Prohibit plaintext reach credential export and `dump`/`export-all` actions
Audit the `reach_credentials` tool (T-006) and `reach_doctor` tool (T-007) to
confirm that neither exposes a `dump`, `export`, `export-all`, or
`get-all-credentials` action. Confirm that the `list` action returns only key
names (via `list_names`, FR-029), never decrypted values. Confirm that any
existing or future ragent config/session export mechanism that iterates the
`provider_auth` table treats reach credential rows identically to
LLM-provider credential rows — emitting at most the `provider_id` (the
`reach::` key name) and never the decrypted `api_key` value. Add a test that
calls `reach_credentials` with every exposed action and asserts none returns
a decrypted credential value; add a grep assertion that no `.rs` file under
`crates/ragent-reach/src/` contains the strings `dump`, `export_all`, or
`get_all_credentials`. This ensures the existing ragent encrypted credential
store's at-rest encryption and machine-binding properties are never bypassed
by a reach export path. Implements FR-044.