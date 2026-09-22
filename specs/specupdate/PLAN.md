---
spec_id: specupdate
---

# Implementation Plan: `/spec update` Subcommand

## Overview

This plan implements the `/spec update [spec-id]` subcommand as specified in
`specs/specupdate/SPEC.md`. The feature adds a new `Update` variant to the
`SpecCommand` enum, parses it from the slash command string, validates the
target spec, and delegates to an LLM agent to regenerate `PLAN.md` and
`TESTPLAN.md` from the current `SPEC.md` content.

The implementation touches two crates:

1. **`ragent-specs`** — new `Update` variant, parser, usage-error guard,
   help-message entry, and builder helper functions.
2. **`ragent-tui`** — TUI dispatch handler in the `/spec` slash command branch.

No changes are needed to `SpecManager`, `SpecIo`, `Spec`, or the HTTP server
layer — the feature reuses existing read/validate infrastructure and delegates
file writing to the LLM agent via the session processor.

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `Update { spec_id: String }` variant to `SpecCommand` enum | FR-002 | S | Critical | completed | — |
| T-002 | Add `"update"` match arm in `SpecCommand::parse()` — parse spec ID, return `Unknown("update")` if empty | FR-001, FR-012 | S | Critical | completed | T-001 |
| T-003 | Add `"update"` to `is_usage_error()` guard list | FR-003 | S | Critical | completed | T-001 |
| T-004 | Add `update` row to `build_help_message()` constant | FR-004 | S | High | completed | T-001 |
| T-005 | Implement `build_update_status(spec_id)` helper | FR-009 | S | High | completed | T-001 |
| T-006 | Implement `build_update_message(spec_id)` helper | FR-009 | S | High | completed | T-001 |
| T-007 | Implement `build_update_log(spec_id)` helper | FR-009 | S | High | completed | T-001 |
| T-008 | Implement `build_update_prompt(spec_id)` helper — instructs agent to re-read SPEC.md and regenerate PLAN.md + TESTPLAN.md | FR-008, FR-011 | M | Critical | completed | T-001 |
| T-009 | Add `SpecCommand::Update` dispatch handler in TUI `execute_slash_command_inner` — validate spec ID, read spec, guard archived, select explore agent, build prompt, spawn processor | FR-005, FR-006, FR-007, FR-010, FR-013 | M | Critical | completed | T-001, T-002, T-008 |
| T-010 | Add `Unknown("update")` to the TUI usage-error branch so missing spec-id shows `"Usage: /spec update — try /spec help"` | FR-012 | S | High | completed | T-002, T-009 |
| T-011 | Add unit tests in `crates/ragent-specs/tests/test_slash_spec.rs` — parse `update <id>`, parse `update` (empty), `is_usage_error` for update, help-message contains update row, builder helpers return expected substrings | FR-001, FR-003, FR-004, FR-009, NFR-003 | S | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008 |
| T-012 | Add TUI slash-command test in `crates/ragent-tui/tests/test_slash_commands.rs` — verify `Update` variant dispatches without panic and sets expected status | FR-010, NFR-003 | S | Medium | completed | T-009 |
## Task Detail

### T-001 — Add Update variant

Add a new variant to the `SpecCommand` enum in
`crates/ragent-specs/src/commands.rs`:

```rust
/// Re-read the existing SPEC.md and regenerate PLAN.md and TESTPLAN.md.
Update {
    /// Spec identifier.
    spec_id: String,
},
```

Place it before the `Unknown(String)` catch-all variant.

### T-002 — Parser match arm

In `SpecCommand::parse()`, add a new match arm after the `"jtbd"` arm and before
the `other => Self::Unknown(other.to_string())` catch-all:

```rust
"update" => {
    let spec_id = rest.trim();
    if spec_id.is_empty() {
        Self::Unknown("update".to_string())
    } else {
        Self::Update {
            spec_id: spec_id.to_string(),
        }
    }
}
```

### T-003 — is_usage_error guard

In `is_usage_error()`, add `|| s == "update"` to the existing guard list.

### T-004 — Help message row

In `build_help_message()`, add a new row to the table (before the Example
line):

```text
| `/spec update <spec-id>` | required `spec-id` | Re-read the existing `SPEC.md` and regenerate `PLAN.md` and `TESTPLAN.md` from its current content. |
```

### T-005 — build_update_status

```rust
pub fn build_update_status(spec_id: &str) -> String {
    format!(
        "spec: updating specs/{spec_id}/PLAN.md + specs/{spec_id}/TESTPLAN.md…"
    )
}
```

### T-006 — build_update_message

A user-facing message consistent with `build_add_message` / `build_jtbd_message`:

```rust
pub fn build_update_message(spec_id: &str) -> String {
    format!(
        "From: /spec update\n🔄 **Regenerating plan and test plan…**\n\n\
         Re-reading `specs/{spec_id}/SPEC.md` and regenerating:\n\
         - `specs/{spec_id}/PLAN.md` — implementation plan with tasks\n\
         - `specs/{spec_id}/TESTPLAN.md` — manual test plan with test cases\n\n\
         The `SPEC.md` file will not be modified.\n\
         This may take a few moments."
    )
}
```

### T-007 — build_update_log

```rust
pub fn build_update_log(spec_id: &str) -> String {
    format!(
        "Regenerating PLAN.md + TESTPLAN.md for spec '{spec_id}' from existing SPEC.md"
    )
}
```

### T-008 — build_update_prompt

A prompt that instructs the agent to read the existing SPEC.md and regenerate
PLAN.md and TESTPLAN.md. The prompt includes the existing PLAN.md content so
the agent can preserve task statuses for unchanged task IDs (FR-011). The prompt
must NOT instruct the agent to modify SPEC.md.

Key prompt content:
- "You are an expert specification writer. An existing spec has been updated.
  Re-read the current `specs/<spec-id>/SPEC.md` and regenerate
  `specs/<spec-id>/PLAN.md` and `specs/<spec-id>/TESTPLAN.md` to match."
- PLAN.md requirements: `## Tasks` table with columns ID, Title, Requirement,
  Effort, Priority, Dependencies; IDs T-001+; effort S/M/L; priority
  Critical/High/Medium/Low.
- TESTPLAN.md requirements: YAML frontmatter `status: draft`; `## Test Cases`
  section; manual test cases TC-001+ with preconditions, steps, test data,
  expected results; no automated test code.
- "Do NOT modify the `SPEC.md` file."
- "Use the `write` tool to overwrite `PLAN.md` and `TESTPLAN.md`."

### T-009 — TUI dispatch handler

In `crates/ragent-tui/src/app/slash.rs`, in the `execute_slash_command_inner`
method's `SpecCommand` match block, add a new arm for `SpecCommand::Update`:

1. Call `append_assistant_text(build_update_message)`.
2. Call `push_log_no_agent(build_update_log)`.
3. Resolve `working_dir` / `specs_root` / `SpecManager`.
4. Validate `SpecId::new(spec_id)` — on failure, set status + error message,
   return.
5. `block_in_place` + `block_on`: call `mgr.read_spec(&sid)` — on failure,
   list available specs and show error (mirror `/spec impl` error path).
6. Check `spec.status == Archived` — on true, show archived error, return.
7. Read existing `PLAN.md` content (for status preservation in prompt).
8. Select explore agent (fallback to `self.agent_info.clone()`).
9. Apply selected model/thinking; set `default_permissions()`.
10. Call `build_update_prompt(spec_id)`.
11. Push `Message::user_text` with the prompt.
12. Clone `session_processor`, create cancel `AtomicBool`, set `is_processing`,
    set `self.status` to `build_update_status`.
13. `tokio::spawn` the processor call (identical to `/spec add` / `/spec jtbd`).
14. On any error path: set `self.status`, append error text, return.

### T-010 — TUI Unknown("update") branch

In the TUI `match` block, the `SpecCommand::Unknown(sub)` arm already handles
the usage-error subcommands via a guard that checks `sub == "create" || ... ||
sub == "jtbd"`. Add `|| sub == "update"` to that guard so the missing-spec-id
case displays `"Usage: /spec update — try /spec help"`.

### T-011 — ragent-specs unit tests

In `crates/ragent-specs/tests/test_slash_spec.rs`, add tests:

- `test_spec_command_update_parses` — `parse("update myspec")` returns
  `Update { spec_id: "myspec" }`.
- `test_spec_command_update_missing_spec_id` — `parse("update")` returns
  `Unknown("update")` and `is_usage_error()` is true.
- `test_spec_help_contains_update` — `build_help_message()` contains
  `"/spec update"`.
- `test_spec_update_status_message` — `build_update_status("myspec")`
  contains `"specs/myspec/PLAN.md"` and `"specs/myspec/TESTPLAN.md"`.
- `test_spec_update_message` — `build_update_message("myspec")` contains
  `"SPEC.md"` and `"PLAN.md"` and `"TESTPLAN.md"`.
- `test_spec_update_log` — `build_update_log("myspec")` contains
  `"myspec"`.
- `test_spec_update_prompt` — `build_update_prompt("myspec")` contains
  `"specs/myspec/SPEC.md"`, `"specs/myspec/PLAN.md"`,
  `"specs/myspec/TESTPLAN.md"`, and `"## Test Cases"`.

### T-012 — TUI slash-command test

In `crates/ragent-tui/tests/test_slash_commands.rs`, add a test that verifies
the `Update` variant is dispatched without panic and that a missing spec-id
produces a usage-error status. This follows the pattern of
`test_spec_command_unknown_subcommand` in `test_slash_spec.rs`.

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| LLM overwrites SPEC.md despite instructions | Low | Medium | Prompt explicitly says "Do NOT modify SPEC.md"; TUI does not pass write-permission for SPEC.md path specifically (uses default_permissions, same as create/add) |
| LLM does not preserve existing task statuses | Medium | Low | FR-011 is best-effort; prompt instructs preservation but the feature is primarily about regeneration. Users can manually fix statuses after. |
| Help message constant becomes too long | Low | Low | The constant is already large; one additional row is negligible |
| User loses existing PLAN.md content | Medium | Medium | The prompt includes existing PLAN.md content for reference; the `write` tool overwrites atomically so there is no partial-write risk |

---

## Definition of Done

1. `/spec update <spec-id>` parses correctly and is listed in `/spec help`.
2. `/spec update` without a spec ID shows a usage error.
3. `/spec update <invalid-id>` shows an invalid spec ID error.
4. `/spec update <nonexistent-id>` shows a not-found error with available specs.
5. `/spec update <archived-id>` shows an archived error.
6. `/spec update <valid-id>` spawns an LLM agent that regenerates PLAN.md and
   TESTPLAN.md without modifying SPEC.md.
7. All unit tests pass (`cargo test -p ragent-specs`).
8. TUI slash-command test passes (`cargo test -p ragent-tui`).
9. `cargo clippy` and `cargo fmt --check` pass.

---

*End of Implementation Plan*