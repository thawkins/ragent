# JCODEPLAN Milestone 7 — Completion Report

**Milestone:** M7 — External Integrations (Gmail + Messaging Channels)
**Status:** ✅ Complete (T-060, T-061, T-062, T-063)

## Objective

Implement the Milestone 7 external integrations defined in
`docs/JCODEPLAN.md`: a `gmail` tool and a `send_channel_message` tool in
`ragent-tools-extended`, with config schema, mocked-backend tests, and
registry wiring.

## Delivered

### T-060 — `gmail` tool

**File:** `crates/ragent-tools-extended/src/gmail.rs` (865 lines)

- `GmailTool` with actions: `search`, `read`, `draft`, `send`, `auth`,
  `status`, `logout`, implemented against the Gmail REST API v1.
- `SqliteTokenStore` stores OAuth2 tokens encrypted via
  `ragent_storage::Storage::set_provider_auth("gmail", json)` (machine-local
  v2 encryption scheme). Tokens never appear in `ragent.json`.
- Injectable `TokenStore` trait (`GmailTool::with_store`) enables in-memory
  test doubles.
- Automatic refresh-token exchange with a single retry on HTTP 401.
- Client-credential precedence: auth-time args → stored tokens →
  `gmail.client_id` / `gmail.client_secret` config (with `env:` indirection
  via `channels::resolve_secret`) → `GMAIL_CLIENT_ID` /
  `GMAIL_CLIENT_SECRET` environment variables.
- `gmail.base_url` config override retargets the API and token endpoint
  (`{base}/oauth2/v4/token`) so tests can use `http://` mock servers.
- `build_raw_message` constructs an RFC 2822 message and encodes it
  base64url for the `messages.send` payload.

### T-061 — `send_channel_message` tool

**File:** `crates/ragent-tools-extended/src/channels.rs`

- `SendChannelMessageTool` supporting Telegram (bot API `sendMessage`) and
  Discord (incoming webhook).
- Actions: `send` (targets `telegram`, `discord`, or `all`), `status`.
- `resolve_secret` applies `env:` indirection so credentials can be kept out
  of configuration files.
- Graceful degradation: honest errors with a `next_action` hint when no
  channel is configured.

### Config schema

**Files:** `crates/ragent-config/src/config.rs`, `crates/ragent-config/src/lib.rs`

- New `ChannelsConfig`, `TelegramChannelConfig`, `DiscordChannelConfig`, and
  `GmailConfig` types on `Config`, re-exported from `ragent-config`.
- Empty blocks are skipped during serialization; overlay merge preserves
  base channels.
- Both tools use `permission_category() = "network:send"`.

### T-062 — Mocked-backend tests

- `crates/ragent-tools-extended/tests/test_gmail.rs` — 19 tests: identity,
  schema (7 actions), config parse, encrypted `SqliteTokenStore` round-trip
  asserting no plaintext leaks into the SQLite row, auth/status/logout
  cycle, mocked search/read/draft/send, refresh-token exchange, and an
  RFC 2822 wire-format check.
- `crates/ragent-tools-extended/tests/test_channels.rs` — 20 tests:
  identity, schema, config parse/merge, env indirection, graceful
  degradation, and mocked Telegram/Discord fanout via an axum mock server
  (axum 0.8 path syntax `/bot{token}/sendMessage`).

### T-063 — Registration

- Both tools registered in `create_extended_registry()`
  (`crates/ragent-tools-extended/src/lib.rs`, "JCODEPLAN M7 — external
  integrations" block).
- Agent-crate registration is automatic:
  `register_extracted_extended_tools`
  (`crates/ragent-agent/src/tool/mod.rs:997`) adapts everything in
  `create_extended_registry()` via `ExtractedExtendedToolAdapter`; no manual
  edit was required.

## Verification

- `cargo check -p ragent-config -p ragent-tools-extended`: clean, 0 warnings.
- `cargo test -p ragent-tools-extended --test test_gmail --test test_channels`:
  39/39 passed.
- `cargo test -p ragent-tools-extended -p ragent-config -p ragent-agent`:
  all suites pass, 0 failures.
- `cargo clippy -p ragent-agent -p ragent-tools-extended -p ragent-config
  --all-targets`: no warnings attributable to M7 work (3 pre-existing
  warnings remain in `test_mf_links.rs` and `test_bg_service.rs`).

## Documentation updates

- `SPEC.md` — new §19A "Gmail & Messaging Channels" covering both tools,
  credential precedence, and config examples.
- `CHANGELOG.md` — Unreleased entry describing the M7 additions.
- `docs/JCODEPLAN.md` — T-060 through T-063 marked complete (✅).

## Acceptance evidence

- `gmail action="search" query="from:ci@example.com"` returns messages —
  exercised against a mocked Gmail API in
  `test_gmail.rs::test_search_and_read_via_mock`.
- `send_channel_message message="deployed"` sends to configured channels —
  exercised against mocked Telegram and Discord endpoints in
  `test_channels.rs::test_send_all_channels_fanout`.
