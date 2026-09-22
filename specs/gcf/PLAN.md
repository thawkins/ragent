---
status: draft
---
# Implementation Plan: GCF Protocol Integration

Each task maps to one or more requirements in [`SPEC.md`](SPEC.md).

## Summary

| Item | Value |
|------|-------|
| Spec ID | `gcf` |
| Config section | `gcf.enabled` (default off, omitted when default) |
| Slash command | `/gcf on|off|show|help` |
| Encode choke point | `tool_result_content_for_llm` (`crates/ragent-agent/src/session/history.rs`) |
| Dependency | `gcf` crate (crates.io, MIT, zero deps) or `vendor/gcf/` fallback |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Verify `gcf` crate API and add workspace dependency | FR-004, FR-007 | S | Critical | completed | — |
| T-002 | Add `GcfConfig` to `ragent-config` with default-off | FR-001 | S | Critical | completed | T-001 |
| T-003 | Config serde round-trip and malformed-value tests | FR-001, FR-008 | S | High | completed | T-002 |
| T-004 | Implement GCF encoding hook in `tool_result_content_for_llm` | FR-004, FR-005, FR-007 | M | Critical | completed | T-001, T-002 |
| T-005 | Encoder hook tests (threshold, margin, fallback, round-trip) | FR-004, FR-007 | M | High | completed | T-004 |
| T-006 | Add conditional GCF primer to system prompt | FR-006 | S | High | completed | T-002 |
| T-007 | Primer presence/absence tests | FR-006 | S | Medium | completed | T-006 |
| T-008 | Register `/gcf` in SLASH_COMMANDS + suggestions | FR-003 | S | High | completed | — |
| T-009 | Implement `/gcf` dispatch arm (on/off/show/help/reject) | FR-002, FR-003 | M | Critical | completed | T-002, T-008 |
| T-010 | `/gcf` TUI dispatch tests (persist, help, unknown subcommand) | FR-002, FR-003 | M | High | completed | T-009 |
| T-011 | Verify non-LLM consumers see raw results under encoding | FR-005 | S | High | completed | T-004 |
| T-012 | Manual TESTPLAN execution pass (AC-1..AC-8) | All FR | S | Medium | blocked | T-010, T-011 |
| T-013 | Update docs (QUICKSTART/SPEC config schema, CHANGELOG) | FR-001, FR-003 | S | Medium | completed | T-009 |
## Task details

### T-001 — Verify `gcf` crate API and add workspace dependency

- Check crates.io for the published `gcf` crate (gcf-rust); confirm API shape
  for the generic profile: JSON value in, GCF text out, and a decode entry
  point for round-trip use.
- Add `gcf = "<x.y.z>"` to `[workspace.dependencies]` in root `Cargo.toml`
  and reference it from `crates/ragent-agent/Cargo.toml`.
- If the published crate is unsuitable, vendor the minimal generic-profile
  encoder/decoder under `vendor/gcf/` (pattern: `vendor/html2text`).
- Confirm `cargo deny` / `cargo audit` accept the dependency (MIT, zero
  transitive deps expected to pass cleanly).
- Verify: `cargo check -p ragent-agent` compiles with the dep wired.

### T-002 — Add `GcfConfig` to `ragent-config` with default-off

- New module `crates/ragent-config/src/gcf.rs`:
  `pub struct GcfConfig { pub enabled: bool }` with `Default` (`false`),
  `is_default()` helper; `mod gcf;` + re-export in `lib.rs`.
- `Config` field near the opt-in sections (`piegap`/`trigger` block,
  `config.rs` ~line 225): `#[serde(default, skip_serializing_if =
  "GcfConfig::is_default")] pub gcf: GcfConfig`.
- Verify: `cargo check -p ragent-config`; a config file without `gcf`
  deserialises to `enabled == false`.

### T-003 — Config serde round-trip and malformed-value tests

- New `crates/ragent-config/tests/test_gcf_config.rs` following
  `test_code_index_config.rs` / `test_tool_visibility.rs` patterns:
  - absent section -> `enabled == false` (default-off, FR-001);
  - `gcf.enabled: true` -> true, `false` -> false;
  - serialise-with-default -> `gcf` key omitted;
  - malformed values (`"gcf": {"enabled": "yes"}`, `"gcf": true`) -> load
    error with actionable diagnostics (FR-008);
  - persistence through `save_to_source()` keeps `gcf.enabled` (FR-009).
- Verify: `cargo test -p ragent-config --test test_gcf_config`.

### T-004 — Implement GCF encoding hook in `tool_result_content_for_llm`

- Extend `crates/ragent-agent/src/session/history.rs::tool_result_content_for_llm`
  (the single choke point covering live dispatch + replay):
  - gate on effective GCF state (from config, threaded via the existing
    config access path used by the session);
  - parse observation as JSON; apply minimum-size threshold (200 chars);
  - encode with the `gcf` generic profile; wrap in a labelled block;
  - size-margin rule: keep raw JSON when GCF output is not smaller;
  - any encode failure falls back to the raw observation;
  - error observations (`Error: ...`) are never encoded.
- Ensure the function stays pure (input -> output) so both call sites
  (processor.rs live path, history.rs replay path) get identical behaviour
  without touching processor.rs dispatch code.
- Verify: `cargo check -p ragent-agent`; manual spot check that
  processor/history call sites need no changes.

### T-005 — Encoder hook tests (threshold, margin, fallback, round-trip)

- New `crates/ragent-agent/tests/test_gcf_encode_hook.rs`:
  - non-JSON observation passes through unchanged;
  - small JSON (< threshold) passes through unchanged;
  - large JSON object/array is emitted as a GCF block;
  - GCF block decodes to the original JSON (lossless, FR-007);
  - margin rule keeps raw when encoding would not save space;
  - encode-failure path falls back to raw;
  - disabled state returns input unchanged (FR-001/FR-005 interaction).
- Verify: `cargo test -p ragent-agent --test test_gcf_encode_hook`.

### T-006 — Add conditional GCF primer to system prompt

- In `crates/ragent-agent/src/agent/mod.rs` system-prompt assembly, add a
  `## GCF Encoding Primer` section included only when the effective GCF state
  is enabled (same conditional-section pattern as existing toggles).
- Content: block marker, generic-profile grammar summary (headers, pipe
  rows, scalar grammar, nested `>` path columns), lossless statement, read
  as structured data instruction.
- Verify: `cargo check -p ragent-agent`.

### T-007 — Primer presence/absence tests

- New tests in `crates/ragent-agent/tests/test_builtin_agents.rs` pattern
  (or a sibling `test_gcf_primer.rs`): prompt contains the primer when
  enabled; no primer string when disabled or default.
- Verify: targeted cargo test run.

### T-008 — Register `/gcf` in SLASH_COMMANDS + suggestions

- `crates/ragent-tui/src/app/state.rs`: add `SlashCommandDef { trigger:
  "gcf", description: "Toggle GCF encoding of tool results: /gcf
  on|off|show|help" }` to `SLASH_COMMANDS` (alphabetical position).
- `crates/ragent-tui/src/app/slash.rs`: add `"gcf" => vec!["on", "off",
  "show", "help"]` arm in `get_command_suggestions`.
- Verify: `cargo check -p ragent-tui`.

### T-009 — Implement `/gcf` dispatch arm (on/off/show/help/reject)

- In `execute_slash_command_inner` match block (`crates/ragent-tui/src/app/slash.rs`
  ~line 2067), add a `"gcf"` arm mirroring the `/codeindex on|off` template
  (slash.rs ~8813):
  - `on`/`off`: set in-memory effective state, `Config::load()` -> set
    `gcf.enabled` -> `save_to_source()` -> `invalidate_config_cache()`;
    on save failure keep live state + "(unsaved)" status;
  - `show`: render current state + source (persisted / in-session / default-off);
  - `help`/`--help`/`-h`/no-args: markdown help notice (purpose + subcommands),
    `is_help_args` helper;
  - unknown: usage notice, no state change.
- Confirmation notice bubbles via `append_assistant_text`, status line via
  `self.status`.
- Verify: `cargo check -p ragent-tui`.

### T-010 — `/gcf` TUI dispatch tests (persist, help, unknown subcommand)

- New `crates/ragent-tui/tests/test_gcf_command.rs`:
  - `/gcf on` persists `gcf.enabled: true` and live state is on;
  - `/gcf off` persists `false`;
  - `/gcf show` reports state without mutation;
  - help variants (bare, `help`, `--help`, `-h`) display help, mutate
    nothing;
  - unknown subcommand rejected, state unchanged, config untouched.
- Follow `test_codeindex_toggle_persistence.rs` structure for temp-config
  setup/teardown.
- Verify: `cargo test -p ragent-tui --test test_gcf_command`.

### T-011 — Verify non-LLM consumers see raw results under encoding

- Trace and assert (via targeted reads/tests) that with encoding active the
  TUI ToolResult event, activity log, memory `on_tool_result`, PostToolUse
  hooks, and compaction inputs receive raw JSON (FR-005). Where a consumer
  shares the encoded view, fix routing before completion.
- Verify: targeted test run; note results in the task log.

### T-012 — Manual TESTPLAN execution pass (AC-1..AC-8)

- Execute `specs/gcf/TESTPLAN.md` cases TC-001..TC-008 in a debug build;
  record results.
- Verify: all manual cases pass.

### T-013 — Update docs (QUICKSTART/SPEC config schema, CHANGELOG)

- Add `gcf` section to the config schema example (SPEC.md config chapter),
  mention `/gcf` in QUICKSTART slash-command docs and CHANGELOG entry.
- Verify: docs spell-check / consistent version references.

---

## Risks

- **R-1**: Published `gcf` crate API may differ from README claims (OQ-1).
  Mitigation: T-001 verifies first; vendor fallback path defined.
- **R-2**: Encoding at the choke point could regress replay behaviour if the
  persisted content were stored encoded. Mitigation: hook is view-time only
  (encode at read); persisted parts keep raw JSON.
- **R-3**: LLM misreads GCF blocks for providers without primer support.
  Mitigation: primer section FR-006 + labelled block marker; size-margin rule
  avoids tiny unhelpful encodes.