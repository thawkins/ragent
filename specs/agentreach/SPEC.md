---
status: draft
audit:
  - { time: 1787528989, from: "none", to: "draft", actor: "system" }
---
# Agent-Reach Capability Integration — Specification

## Background

[Agent-Reach](https://github.com/Panniantong/Agent-Reach) is a Python "capability
layer" that gives AI agents internet access to platforms that are difficult to
reach: Twitter/X, Reddit, YouTube, Bilibili, XiaoHongShu, RSS, Facebook,
Instagram, LinkedIn, plus general web reading and search. It is **not** a set of
readers — it is a **selection, installation, diagnosis, and routing** layer over
upstream open-source CLIs (twitter-cli, rdt-cli, bili-cli, yt-dlp, feedparser,
OpenCLI, etc.). Its distinctive value is the ordered "preferred + fallback"
backend list per channel and the `agent-reach doctor` health check.

ragent already provides strong general-purpose web tools:

- `webfetch` / `mf_fetch` — arbitrary web-page and PDF extraction.
- `websearch` / `mf_search` — keyless multi-engine web search.
- `http_request` — full HTTP method/headers/body control.
- `browser` — Chrome DevTools Protocol automation (for login-gated sites).
- GitHub tools (`github_*`) — authenticated repository access.
- `mf_fetch` YouTube transcript extraction (via `masterfetch::youtube`).

The **gap** is first-class access to social/content platforms (Twitter, Reddit,
Bilibili, XiaoHongShu, RSS, Facebook, Instagram, LinkedIn) and the
**credential + diagnostics + routing** layer that makes them usable without the
user hand-configuring each upstream CLI.

## Goal

Add a new `reach` tool family to ragent that exposes Agent-Reach-style platform
channels as native LLM tools, reusing ragent's existing `Tool` trait,
`ToolRegistry`, `ToolContext`, permission system, `tool_visibility` config, and
HTTP/CLI execution infrastructure. Add only what ragent lacks: channel
backends, credential storage, routing, and a diagnostics tool.

## Scope

### In scope

- A new `ragent-reach` crate under `crates/` providing channel backends.
- New `reach_*` LLM tools registered via the extended-tools adapter.
- A `tool_visibility.reach` config switch (default on).
- Encrypted local credential storage for per-platform tokens/cookies.
- Ordered backend routing (preferred + fallback) per channel.
- A `reach_doctor` diagnostics tool.
- Reuse of existing `mf_fetch`/`http_request`/`browser` where a channel just
  needs HTTP or browser automation.

### Out of scope

- Bundling or vendoring the Python `agent-reach` package (we reimplement the
  routing layer in Rust; upstream CLIs are invoked only when no native Rust
  backend is available).
- A TUI installer wizard (Agent-Reach's `agent-reach install`). Configuration is
  done via `ragent.json` and the `reach_credentials` tool.
- Reverse-engineering or scraping that violates platform ToS. Each backend
  uses either official APIs, the user's own authenticated session, or
  public-page reads via existing ragent web tools.

## Design Principles

1. **Reuse ragent infrastructure.** New tools implement the existing
   `ragent_tools_extended::Tool` trait and are registered in
   `create_extended_registry()`. No new tool trait, registry, or permission
   model.
2. **Capability layer, not a reader.** Each channel picks the best available
   backend at runtime and falls back gracefully. The LLM sees one stable tool
   per action (`reach_twitter_search`), not the underlying backend.
3. **Credentials are local and encrypted.** Reuse `ragent-storage`'s encrypted
   credential store. Never log secrets. File permissions 0600.
4. **Native Rust first.** Prefer HTTP-based backends implemented in Rust
   (using `reqwest` already in the workspace). Shell out to upstream CLIs only
   when no HTTP path exists (e.g., yt-dlp for media, feedparser-equivalent in
   Rust). The `bash` tool's 7-layer security governs any shelling-out.
5. **Diagnostics built in.** `reach_doctor` reports each channel's current
   backend, health, and configuration status — mirroring Agent-Reach's doctor.
6. **No silent failure.** When all backends for a channel are unavailable, the
   tool returns an actionable error describing what to configure.

## Architecture

```
crates/ragent-reach/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Tool trait impls, create_reach_registry()
│   ├── config.rs           # ReachConfig (deserialised from ragent.json)
│   ├── credentials.rs      # Encrypted credential store (delegates to ragent-storage)
│   ├── routing.rs          # Ordered backend list + probe + select
│   ├── doctor.rs           # reach_doctor tool
│   ├── credentials_tool.rs # reach_credentials tool
│   ├── channels/
│   │   ├── mod.rs          # Channel trait + registry
│   │   ├── web.rs          # web read (delegates to mf_fetch — existing)
│   │   ├── search.rs       # web search (delegates to mf_search — existing)
│   │   ├── github.rs       # GitHub (delegates to existing github_* tools)
│   │   ├── youtube.rs      # YouTube transcript (delegates to masterfetch::youtube)
│   │   ├── rss.rs          # RSS/Atom parsing (NEW — feed-rs crate)
│   │   ├── twitter.rs      # Twitter/X read + search (NEW backends)
│   │   ├── reddit.rs       # Reddit read + search (NEW backends)
│   │   ├── bilibili.rs     # Bilibili search + detail (NEW backends)
│   │   ├── xiaohongshu.rs  # XiaoHongShu read (NEW backend)
│   │   ├── linkedin.rs     # LinkedIn public read (delegates to mf_fetch + Jina)
│   │   ├── facebook.rs     # Facebook (browser-session backend)
│   │   └── instagram.rs    # Instagram (browser-session backend)
│   └── backends/
│       ├── mod.rs
│       ├── http_json.rs    # Generic HTTP+JSON backend helper
│       ├── jina_reader.rs  # r.jina.ai reader (existing mf_fetch can delegate)
│       ├── twitter_api.rs  # Twitter API v2 (when token configured)
│       ├── twitter_guest.rs # Guest/anonymous read endpoint
│       ├── reddit_oauth.rs # Reddit OAuth2 (app-only token)
│       ├── reddit_rss.rs   # Reddit RSS JSON endpoint (no auth, .json suffix)
│       ├── bili_api.rs     # Bilibili public search/detail API
│       └── browser_session.rs # Reuses ragent browser tool for login-gated
```

The crate exposes its tools through `create_reach_registry()` which is called
from `ragent-tools-extended::create_extended_registry()` (one additional
`register_extracted_reach_tools` function in `ragent-agent/src/tool/mod.rs`),
identical to how core/extended/vcs tools are already wired.

## Tool Inventory

| Tool name | Action | Backends | New? |
|-----------|--------|----------|------|
| `reach_web_read` | Read any web page | mf_fetch (existing) | Wrapper |
| `reach_web_search` | Web search | mf_search (existing) | Wrapper |
| `reach_github` | Read public/private repo | github_* (existing) | Wrapper |
| `reach_youtube` | YouTube transcript | masterfetch::youtube (existing) | Wrapper |
| `reach_rss` | Parse RSS/Atom feed | feed-rs (new) | New |
| `reach_twitter_read` | Read a tweet | twitter_api → twitter_guest | New |
| `reach_twitter_search` | Search tweets | twitter_api → twitter_guest | New |
| `reach_reddit_read` | Read post + comments | reddit_oauth → reddit_rss | New |
| `reach_reddit_search` | Search subreddit | reddit_oauth → reddit_rss | New |
| `reach_bilibili_search` | Search Bilibili videos | bili_api | New |
| `reach_bilibili_detail` | Video detail + subtitles | bili_api | New |
| `reach_xiaohongshu_read` | Read a XiaoHongShu note | browser_session | New |
| `reach_linkedin_read` | Read public profile/page | jina_reader | New wrapper |
| `reach_facebook_read` | Read public page/feed | browser_session | New |
| `reach_instagram_read` | Read public profile/posts | browser_session | New |
| `reach_doctor` | Diagnose all channels | — | New |
| `reach_credentials` | Set/list/test credentials | encrypted store | New |

## Requirements

### FR-001 — Ubiquitous

The system **shall** register every `reach_*` tool in the existing
`ragent-tools-extended` `ToolRegistry` via a `create_reach_registry()` function
called from `create_extended_registry()`, so that the agent loop, permission
checker, and tool-visibility system treat them identically to all other tools.

> Ubiquitous: always active when the `reach` visibility switch is on.

### FR-002 — Ubiquitous

The system **shall** expose a `tool_visibility.reach` boolean config switch in
`ragent-config::ToolVisibilityConfig`, defaulting to `true`, governed by the
same `set_hidden()` mechanism as `office`, `github`, and `finance`.

### FR-003 — Event-driven

**When** the agent invokes any `reach_*` channel tool, the system **shall**
select a backend by probing the ordered backend list for that channel in
preferred-to-fallback order and use the first backend whose preflight check
succeeds.

> Event-driven: triggered by a tool invocation.

### FR-004 — State-driven

**If** all configured backends for a channel fail their preflight check, the
system **shall** return a `ToolOutput` error that names the channel, lists the
tried backends, and states which credential or configuration is missing.

> State-driven: depends on the availability state of backends.

### FR-005 — Optional

**Where** a channel has a zero-configuration backend (e.g., Reddit RSS,
Bilibili public API, Jina Reader), the system **may** serve the request
without any credentials, and `reach_doctor` **shall** report the channel as
`healthy (no-auth)`.

> Optional: applies when a no-auth backend exists.

### FR-006 — Unwanted

The system **shall not** transmit, log, or include in tool output any
credential, cookie, or token value. Credential fields in tool output and
`reach_doctor` output **shall** be masked to their last four characters or
shown as `configured` / `not configured`.

> Unwanted: the credential-leak behaviour is prohibited.

### FR-007 — Ubiquitous

The system **shall** store all platform credentials in the existing
`ragent-storage` encrypted credential store, keyed by platform name, with
file permissions `0600` on the underlying database.

### FR-008 — State-driven

**If** the `reach` visibility switch is `false`, the agent loop **shall**
hide all `reach_*` tool definitions from the LLM, identical to the behaviour
for `office`, `github`, and `teams` tools.

### FR-009 — Event-driven

**When** the user runs `reach_credentials` with action `set`, the system
**shall** persist the supplied credential under the named platform key and
confirm storage without echoing the secret value.

### FR-010 — Event-driven

**When** the user runs `reach_doctor`, the system **shall** iterate every
registered channel, run each backend's preflight probe, and return a JSON
report with per-channel `status` (`healthy`, `degraded`, `unavailable`),
`active_backend`, and `fix_hint`.

### FR-011 — Ubiquitous

Every `reach_*` tool **shall** declare a `permission_category` of `"reach"`
so the existing permission system groups them under one family.

### FR-012 — State-driven

**If** a channel backend requires shelling out to an upstream CLI (e.g.,
`yt-dlp` for media download), the system **shall** execute it through the
existing `bash` tool's command-validation layer; direct `std::process::Command`
use with user-supplied input is prohibited.

### FR-013 — Optional

The system **may** reuse `mf_fetch` as the backend for `reach_web_read` and
`reach_linkedin_read` rather than reimplementing HTTP fetch, to avoid
duplicating the existing extraction pipeline.

### FR-014 — Ubiquitous

The `ragent-reach` crate **shall** compile with zero warnings under
`cargo build` and pass `cargo clippy` with the workspace's standard lints,
matching the zero-warnings policy of all other crates.

### FR-015 — Event-driven

**When** a channel's primary backend returns an HTTP 401/403 or a known
rate-limit response, the routing layer **shall** automatically try the next
backend in the ordered list before returning an error.

### FR-016 — Unwanted

The system **shall not** bundle, vendor, or require the Python
`agent-reach` pip package. The integration is a native Rust reimplementation
of the routing and diagnostics concepts; upstream CLIs are optional external
dependencies invoked only when no native backend exists.

### FR-017 — Ubiquitous

The `reach_rss` tool **shall** parse RSS 2.0, Atom 1.0, and RDF/RSS 1.0 feeds
in pure Rust using the `feed-rs` crate (or equivalent), returning title, link,
published date, and content summary per item.

### FR-018 — State-driven

**If** the `ragent-reach` crate is not built (feature-gated off via a
`reach` Cargo feature on `ragent-tools-extended`), the agent binary **shall**
still compile and all non-reach tools **shall** remain registered and visible.

### FR-019 — Ubiquitous

The system **shall** perform all reach credential reads, writes, and deletes
through the existing `StorageBackend` trait's `get_provider_auth`,
`set_provider_auth`, and `delete_provider_auth` methods. Direct `rusqlite`
calls, raw SQL queries, or parallel encryption implementations in the
`ragent-reach` crate are prohibited.

> Ubiquitous: applies to every credential I/O path in the reach crate.

### FR-020 — State-driven

**If** `ToolContext.storage` is `None` (no storage backend configured), the
`ReachCredentialStore` **shall** return an actionable `anyhow::Error` from
all credential operations (`get`, `set`, `delete`, `list_names`) rather than
panicking, calling `unwrap()`, or silently succeeding.

> State-driven: depends on whether storage is configured.

### FR-021 — Ubiquitous

The system **shall** encrypt all reach credentials at rest using
`ragent_storage::storage::encrypt_key` (the v2 blake3-derived keystream
scheme) before persisting them. The stored value in the database **shall**
carry the `v2:` prefix. Reliance on the deprecated v1 `obfuscate_key` path
is prohibited for reach credential writes.

> Ubiquitous: applies to every credential write path.

### FR-022 — Event-driven

**When** a reach credential is stored via `ReachCredentialStore::set`, the
system **shall** register the plaintext value with
`ragent_storage::sanitize::register_secret` so the existing redaction layer
masks it in logs and telemetry. **When** a credential is deleted, the system
**shall** call `ragent_storage::sanitize::unregister_secret` to remove it
from the redaction registry.

> Event-driven: triggered by credential set/delete operations.

### FR-023 — Unwanted

The system **shall not** use the plaintext `settings` table
(`set_setting` / `get_setting`) for reach credential storage. Credentials
**shall** go only through the `provider_auth` table, which applies
encryption. Storing credential secrets as plaintext settings is prohibited.

> Unwanted: the plaintext-credential-storage behaviour is prohibited.

### FR-024 — State-driven

**If** a reach credential was previously stored using the deprecated v1
`obfuscate_key` format (via `set_provider_auth`), the system **shall** rely on
the existing `get_provider_auth` auto-migration path to transparently upgrade
it to v2 `encrypt_key` format on first read. No reach-specific migration code
**shall** be written.

> State-driven: depends on the format of an existing stored credential.

### FR-025 — Unwanted

The system **shall not** pre-encrypt reach credential values with
`ragent_storage::storage::encrypt_key` before passing them to
`set_provider_auth`. The `set_provider_auth` method applies v2 encryption
internally (`obfuscate_key` delegates to `encrypt_key`), so a second
`encrypt_key` call on the same plaintext would produce a double-encrypted
value that `get_provider_auth` cannot decrypt. Reach credential writes
**shall** pass the plaintext secret directly to `set_provider_auth`, and the
resulting stored value **shall** carry the `v2:` prefix exactly once.

> Unwanted: the double-encryption behaviour is prohibited because it makes
> credentials permanently unreadable.

### FR-026 — Unwanted

The `ragent-reach` crate **shall not** call
`ragent_storage::sanitize::register_secret` or `unregister_secret` for
credentials that are stored or deleted via `set_provider_auth` /
`delete_provider_auth`. Those storage methods already register the plaintext
with (and unregister it from) the in-memory redaction registry. A second
`register_secret` call is a redundant no-op; calling `unregister_secret` with
a value that the storage layer has already removed from the registry is a
useless call and may mask a mismatch. Reach **shall** rely on the storage
layer to manage the sanitize registry for `provider_auth` entries.

> Unwanted: the duplicate sanitize-registration behaviour is prohibited.

### FR-027 — Ubiquitous

The `ragent-reach` crate **shall** obtain its `Storage` handle from
`ToolContext.storage` (the session's shared `Arc<Storage>` instance) and
**shall not** open a second `Storage::open` connection to the shared
`ragent.db` database. This guarantees that the process-global sanitize
secret registry stays consistent across reach and non-reach credential I/O,
and avoids WAL lock contention between two concurrent connections to the
same database file. The Gmail `SqliteTokenStore::shared()` pattern (which
opens its own connection) **shall not** be reused for reach.

> Ubiquitous: applies to every reach credential access path.

### FR-028 — Optional

**Where** a platform requires non-secret configuration that is not itself a
credential (e.g., a Reddit `client_id`, a Bilibili locale, or an instance
base URL), the system **may** persist it in the plaintext `settings` table
via `set_setting` under a `reach::` namespaced key. Only secret material
(tokens, cookies, client secrets, bearer strings, refresh tokens) **shall**
go through the encrypted `provider_auth` table. This narrows FR-023, which
prohibits the `settings` table for credentials: non-secret configuration is
not a credential and is exempt.

> Optional: applies only when non-secret per-platform configuration exists.

### FR-029 — Event-driven

**When** the `reach_credentials` tool is invoked with the `list` action, or
`reach_doctor` needs to enumerate configured platforms, the system **shall**
return the set of stored reach credential names by querying the storage
layer's `provider_auth` table through a storage-trait enumeration method.
Direct `rusqlite` calls or raw `SELECT provider_id FROM provider_auth` SQL
inside the `ragent-reach` crate are prohibited (FR-019). **If** no
storage-trait enumeration method exists, the system **shall** maintain a
reach-owned namespaced index (a JSON array of `ReachCredentialKey` strings
stored under a single `settings` key such as `reach::__credential_index__`)
updated atomically on every `set` / `delete`, and **shall** derive the list
from that index. The index stores key *names* only; it is not a credential
and is not prohibited by FR-023.

> Event-driven: triggered by `list` / `reach_doctor`.

### FR-030 — State-driven

**If** `get_provider_auth` returns a non-`None` value whose decrypted
plaintext is the empty string (indicating a `v2:` payload that failed to
decode — typically because the `MACHINE_KEY` was derived from a different
user/home on another machine, or the database row is corrupt), the reach
credential store **shall** treat the credential as unavailable and
`reach_doctor` **shall** report the channel's status as `unavailable` with a
`fix_hint` of `"credential was encrypted on a different machine or user, or
the database row is corrupt; re-set the credential on this machine"`. The
system **shall not** surface the empty string as a usable credential to any
backend.

> State-driven: depends on the decryption-result state.

### NFR-005

Reach credential reads, writes, and deletes that touch the `Storage`
`Mutex<Connection>` **shall** be dispatched via
`tokio::task::spawn_blocking` when called from an async tool handler, so the
SQLite lock acquisition never blocks the agent-loop executor thread. This
matches the workspace's existing pattern of offloading blocking storage I/O
onto the blocking thread pool.

### FR-031 — Event-driven

**When** `register_extracted_reach_tools` constructs the reach tool registry,
the adapter **shall** propagate the session's shared `ToolContext.storage`
handle into every reach tool it registers, so that no production reach tool
runs with `storage: None` while the session has a configured storage backend.
Registration **shall not** fabricate a new `Storage::open` connection to fill
a `None` storage slot (that path remains prohibited by FR-027); a `None`
storage at registration time **shall** surface through the FR-020 actionable
error at tool-invocation time instead.

> Event-driven: triggered by tool-registry construction at session start.

### FR-032 — Ubiquitous

The key prefix `reach::` **shall** be reserved exclusively for reach
credentials and reach configuration within the `provider_auth` and `settings`
tables. Reach code **shall not** read, write, or delete keys outside the
`reach::` namespace, and **shall not** reuse LLM-provider key names (e.g.,
`anthropic`, `openai`, `ollama`) as reach platform keys. The
`ReachCredentialKey` constructor **shall** reject any key that does not begin
with `reach::`.

> Ubiquitous: applies to every reach storage access, keeping reach isolated
> from the other tenants of the shared credential store.

### FR-033 — Ubiquitous

All automated tests covering reach credential storage **shall** run against
`Storage::open_in_memory()` or a temporary SQLite file under `target/temp/`,
and **shall not** open, read, or mutate the user's real `ragent.db` database
or any pre-existing row in its `provider_auth` table. This guarantees that
test runs can never leak fixtures into, or destroy credentials in, the live
encrypted store.

> Ubiquitous: applies to every test in `crates/ragent-reach/tests/` and to
> any reach credential test elsewhere in the workspace.

### FR-034 — Ubiquitous

Short-lived derived tokens — Reddit OAuth app-only access tokens, Twitter
guest tokens, and similar runtime-minted artifacts — **shall** be held only
in process memory with a time-to-live and **shall not** be written to
`provider_auth` or the `settings` table. Only long-lived secret material
(bearer tokens, client secrets, refresh tokens, cookies) **shall** be
persisted via `set_provider_auth`. On token expiry, backends **shall**
re-derive the token from the persisted long-lived material.

> Ubiquitous: applies to every backend that mints ephemeral tokens; keeps the
> encrypted store limited to durable secrets and avoids stale persisted
> tokens shadowing fresh ones.

### FR-035 — Unwanted

The system **shall not** surface raw `rusqlite` or storage-layer error
strings (which may contain row identifiers or payload fragments) in reach
tool output. Reach credential operations **shall** map storage errors to
reach-level errors containing only the platform/key name and an actionable
hint, consistent with the FR-006 masking rule.

> Unwanted: the raw-storage-error-leak behaviour is prohibited.

### FR-036 — Event-driven

**When** `ReachCredentialStore::set` or `delete` maintains the namespaced
credential index defined in FR-029, the `provider_auth` row mutation
**shall** be committed before the index update, so a crash can never leave an
index entry pointing at a credential that was never written. **If**
`list_names` encounters an index entry with no corresponding `provider_auth`
row, it **shall** prune the stale entry from the index and omit it from the
result rather than failing. **If** the stored index is unparseable JSON, the
store **shall** log a `tracing::warn!`, treat the index as empty, and rewrite
it on the next mutation.

> Event-driven: triggered by credential set/delete/list operations.

### FR-037 — Ubiquitous

`reach_doctor` preflight probes **shall** perform only read operations against
the credential store. A probe **shall not** write, re-encrypt, rotate, or
delete any credential as a side effect of diagnosis. The transparent v1-to-v2
format migration performed inside `get_provider_auth` (FR-024) **shall**
remain the sole exception, because it is applied by the storage layer itself,
not by the probe.

> Ubiquitous: applies to every channel/backend preflight probe.

---

(The following block belongs under the existing `### FR-038 — Ubiquitous

The system **shall** route every reach credential read, write, delete, and
enumeration operation through the `ReachCredentialStore` facade established in
T-032. No channel backend, routing module, or tool implementation in
`ragent-reach` **shall** call `get_provider_auth`, `set_provider_auth`,
`delete_provider_auth`, `get_setting`, or `set_setting` directly. The
`ReachCredentialStore` is the single boundary between reach code and the
shared `ragent-storage` encrypted credential store, ensuring consistent
caching (T-036), masking (FR-006), error sanitisation (FR-035), and
namespace enforcement (FR-032).

> Ubiquitous: applies to every credential access path in the `ragent-reach`
> crate, from channel backends through to the `reach_credentials` tool.

### FR-039 — Event-driven

**When** `ReachCredentialStore::set` is called with a credential value, the
system **shall** reject the write and return an actionable error if the value
is an empty string or consists solely of whitespace, because an empty
credential cannot be distinguished from the FR-030 decryption-failure sentinel.
Non-empty values (including values the platform later rejects at preflight)
**shall** be accepted and persisted. The validation **shall** occur before any
`set_provider_auth` call so no empty value is ever written to the
`provider_auth` table.

> Event-driven: triggered by a credential `set` operation.

### FR-040 — Event-driven

**When** any reach credential operation (`get`, `set`, `delete`, `list_names`)
is executed, the system **shall** emit a structured `tracing::debug!` event
recording the operation type and the `ReachCredentialKey` name. The event
**shall not** include the credential value, masked or otherwise. This provides
a security audit trail of credential access without violating the FR-006
no-leak rule. The event **shall** use the `target = "ragent_reach::credentials"`
span so it can be filtered independently of platform backend traffic.

> Event-driven: triggered by every credential I/O operation.

### NFR-007

Reach credential encryption **shall** reuse whatever at-rest cipher
`ragent-storage` currently implements — the v2 blake3-derived keystream scheme
(`encrypt_key`) specified in FR-021 — and **shall not** introduce any
additional cryptographic primitive (AES-GCM or otherwise) into the
`ragent-reach` crate. This clarifies NFR-003, which references "AES-GCM" as the
storage cipher; the actual v2 scheme is a blake3 keystream XOR, and reach
delegates entirely to the storage layer's `set_provider_auth` /
`get_provider_auth` so the cipher choice is a storage-layer concern, not a
reach concern.

### NFR-008

The in-memory credential cache introduced in T-036 **shall** be guarded by a
`tokio::sync::RwLock` (or equivalent lock-free concurrent map) so concurrent
`get` calls from multiple async backend tasks cannot produce a data race.
Cache invalidation on `set` / `delete` (T-037) **shall** acquire a write lock
and **shall** complete without blocking the agent-loop executor thread for
longer than the in-memory map mutation, independent of any SQLite I/O which
remains governed by NFR-005.
## Non-Functional Requirements` heading.)

### NFR-006

Each reach credential operation **shall** hold the `Storage` connection mutex
only for the duration of its SQL work and **shall not** hold it across
network I/O, decryption of unrelated keys, or any `.await` point other than
the `spawn_blocking` join required by NFR-005. Compound operations (e.g., a
credential write plus its FR-029 index update) **shall** use consecutive
short critical sections rather than one long-held lock, so a reach mutation
cannot stall unrelated storage users (memory, sessions, Gmail token store).

---

### FR-041 — Ubiquitous

The system **shall** rely on the existing
`ragent_storage::Storage::seed_secret_registry()` startup call to populate the
in-memory redaction registry with reach credentials. Because reach credentials
are stored in the same `provider_auth` table as all other provider credentials
(FR-007, FR-019), the existing startup seeding pass — which iterates every
`provider_auth` row, decrypts it, and feeds the plaintext into
`crate::sanitize::seed_secrets` — **shall** cover `reach::`-namespaced keys
with no reach-specific seeding code. The `ragent-reach` crate **shall not**
implement, duplicate, or wrap `seed_secret_registry`; it **shall** assume the
session's startup sequence has already called it on the shared `Storage`
handle (FR-027).

> Ubiquitous: applies to every session startup that has reach credentials
> stored in the shared encrypted credential store.

### FR-042 — Unwanted

The `ragent-reach` crate **shall not** create, alter, or extend any SQLite
table, column, index, or migration in the shared `ragent.db` database. Reach
credentials **shall** be stored exclusively in the existing `provider_auth`
table (schema: `provider_id TEXT PRIMARY KEY, api_key TEXT, updated_at TEXT`)
and non-secret configuration exclusively in the existing `settings` table
(schema: `key TEXT PRIMARY KEY, value TEXT, updated_at TEXT`). The
`ragent-reach` crate **shall not** define or execute any `CREATE TABLE`,
`ALTER TABLE`, `CREATE INDEX`, or migration SQL. Any schema evolution of the
encrypted credential store is a `ragent-storage` concern, not a reach
concern.

> Unwanted: the reach-owned schema-mutation behaviour is prohibited because
> it would bypass the storage layer's migration authority and risk corrupting
> the shared database that holds all provider credentials, sessions, and
> memory.

### FR-043 — Event-driven

**When** `reach_doctor` reports a channel as `unavailable` due to an
empty-plaintext decryption failure (FR-030), the `fix_hint` string **shall**
explicitly state that the credential store uses a machine-local encryption key
derived from the current user's username and home directory, and that
credentials set on one machine or under one user account cannot be decrypted
on another. This surfaces the inherent machine-binding property of the
existing `ragent-storage` v2 `encrypt_key` scheme (which derives its key via
`blake3::derive_key("ragent credential encryption v2", "{username}:{home}")`)
so the user understands the failure cause rather than re-entering the same
credential and getting the same result.

> Event-driven: triggered by the FR-030 decryption-failure diagnostic path
> in `reach_doctor`.

### FR-044 — Unwanted

The system **shall not** export, serialise, or otherwise emit decrypted
reach credential plaintext as part of any configuration export, session
export, debug dump, or `reach_doctor` output. Reach credentials are
machine-bound (the v2 `encrypt_key` scheme ties ciphertext to the current
username + home directory) and their plaintext exists only transiently in
process memory for backend use. Any existing or future ragent export
mechanism that iterates the `provider_auth` table **shall** treat reach
credential rows identically to LLM-provider credential rows — emitting at
most the `provider_id` (the `reach::` key name) and never the decrypted
`api_key` value. The `reach_credentials` tool **shall not** expose a `dump`
or `export-all` action.

> Unwanted: the plaintext-credential-export behaviour is prohibited because
> it would bypass the encrypted-at-rest invariant (FR-021) and the
> machine-binding property of the existing credential store.

## Non-Functional Requirements

### NFR-001

The `reach_*` tools **shall** be async and non-blocking, using `tokio` and
`reqwest` consistent with the rest of the workspace. No blocking I/O on the
agent loop's executor thread.

### NFR-002

The `reach_doctor` preflight probes **shall** each time out within 10 seconds
so a single unresponsive backend cannot stall the diagnostic.

### NFR-003

Credential storage **shall** reuse `ragent-storage`'s existing AES-GCM
encryption; no new cryptographic code in `ragent-reach`.

### NFR-004

The `ragent-reach` crate **shall** add no new workspace dependencies beyond
`feed-rs` (RSS parsing) and reuse `reqwest`, `serde`, `serde_json`, `anyhow`,
`tokio`, `tracing`, and `ragent-types` already present.

### NFR-009

The `ragent-reach` crate **shall** introduce zero new SQLite schema objects
— no tables, no columns, no indexes, no views, no triggers, and no migration
steps — in the shared `ragent.db` database. This operationalises FR-042 at
the non-functional level: the encrypted credential store's schema is owned
entirely by `ragent-storage`, and reach's storage footprint is limited to
rows in the existing `provider_auth` and `settings` tables. A post-build
audit **shall** confirm that no `CREATE TABLE`, `ALTER TABLE`, `CREATE INDEX`,
or `migration` SQL string appears in `crates/ragent-reach/src/**/*.rs`.

## Open Questions

1. **Twitter access path.** The Twitter/X API v2 requires a paid Basic tier.
   Agent-Reach uses `twitter-cli` with user-exported cookies. Which backend
   should be primary — API v2 (when token present) or cookie-based guest read?
   *Default decision: API v2 primary when configured, cookie-guest fallback.*
2. **Reddit OAuth.** Reddit now requires OAuth2 app-only tokens even for
   read access. Should we ship a default shared client_id (rate-limited) or
   require the user to register their own Reddit app? *Default: support both;
   `reach_doctor` reports which is active.*
3. **Bilibili subtitles.** `bili-cli` provides no-login search/detail but
   subtitles need OpenCLI. Do we implement subtitle extraction natively or
   defer to a future task? *Default: defer subtitles; ship search + detail
   only in v1.*

## Dependencies

- `ragent-storage` (encrypted credential store) — existing
- `ragent-config` (new `ReachConfig`, `tool_visibility.reach`) — extension
- `ragent-tools-extended` (registration host) — extension
- `ragent-agent` (tool adapter wiring) — extension
- `feed-rs` crate — new workspace dependency (RSS parsing)
- `reqwest` — existing

## Risks

| Risk | Mitigation |
|------|------------|
| Platform APIs change/break | Ordered fallback backends; `reach_doctor` surfaces breakage |
| Credential leak | FR-006 masking; encrypted store; no echo on set |
| Rate limiting | FR-015 fallback; exponential backoff per backend |
| ToS violations | Only official APIs, user sessions, or public pages; documented limits |
| Crate bloat | Feature-gate `reach` behind a Cargo feature (FR-018) |