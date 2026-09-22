# Implementation Plan: TESTPLAN.md Generation in `/spec create`

This plan extends the `/spec create` prompt so that a `TESTPLAN.md`
manual test plan is generated alongside `SPEC.md` and `PLAN.md`. Each
task maps to one or more requirements in [`SPEC.md`](SPEC.md).

## Summary

| Item | Value |
|------|-------|
| Spec ID | `testplan` |
| Crate touched | `ragent-specs` |
| Primary change site | `crates/ragent-specs/src/commands.rs` — `build_create_prompt` |
| Secondary change sites | `build_create_message`, `build_create_log`, `build_create_status` |
| New file per spec | `specs/<id>/TESTPLAN.md` |
| No new dependencies | ✅ |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Extend `build_create_prompt` to request `TESTPLAN.md` | FR-001, FR-003, FR-005, FR-006, FR-009 | S | Critical | completed | — |
| T-002 | Update `build_create_message` to list `TESTPLAN.md` | FR-004 | S | High | completed | T-001 |
| T-003 | Update `build_create_log` to mention `TESTPLAN.md` | FR-004 | S | Medium | completed | T-001 |
| T-004 | Update `build_create_status` to mention `TESTPLAN.md` | FR-004 | S | Medium | completed | T-001 |
| T-005 | Update existing tests in `test_slash_spec.rs` to assert `TESTPLAN.md` appears in the prompt, message, and log | FR-003, FR-004 | S | High | completed | T-001, T-002, T-003 |
| T-006 | Add a new test asserting the prompt instructs writing `TESTPLAN.md` with `## Test Cases` and `TC-NNN` IDs | FR-005, FR-006 | S | High | completed | T-001 |
| T-007 | Add a new test asserting the prompt excludes `#[test]` / `cargo test` instructions from the TESTPLAN section | FR-009 | S | Medium | completed | T-001 |
| T-008 | Update TUI `test_slash_commands.rs` create test if it asserts the create message or prompt content | FR-004 | S | Low | completed | T-002 |
| T-009 | Update `CHANGELOG.md` with the new `TESTPLAN.md` generation behaviour | FR-001 | S | Low | completed | T-001 |
## Task details

### T-001 — Extend `build_create_prompt` to request `TESTPLAN.md`

Append a third numbered item to the prompt in
`crates/ragent-specs/src/commands.rs::build_create_prompt`:

```
3. `specs/{specname}/TESTPLAN.md` — A manual test plan for a human tester:
   - Start with YAML frontmatter containing `status: draft`
   - Include a `## Test Cases` section
   - Each test case has an ID (TC-001, TC-002, …), title, preconditions,
     numbered step-by-step instructions, the exact data to enter into each
     field, and the expected result after each step
   - Cover every UI navigation path and data-entry field required to verify
     the feature
   - This is a manual test plan, NOT automated tests — do not include
     #[test] functions or cargo test commands
   - Optionally include `## Prerequisites` and `## Cleanup` sections
```

The change is strictly additive — the existing `SPEC.md` and `PLAN.md`
instructions remain unchanged, satisfying FR-010.

### T-002 — Update `build_create_message`

In `build_create_message`, add a bullet line for `TESTPLAN.md`:

```
- `specs/{specname}/TESTPLAN.md` — manual test plan with human-verifiable steps
```

### T-003 — Update `build_create_log`

In `build_create_log`, mention `TESTPLAN.md`:

```
Creating spec '{specname}' for feature: {feature} → specs/{specname}/SPEC.md, PLAN.md, TESTPLAN.md
```

### T-004 — Update `build_create_status`

In `build_create_status`, include `TESTPLAN.md`:

```
spec: writing specs/{specname}/SPEC.md + specs/{specname}/PLAN.md + specs/{specname}/TESTPLAN.md…
```

### T-005 — Update existing tests

In `crates/ragent-specs/tests/test_slash_spec.rs::test_slash_spec_create_starts_generation`,
add assertions:

- `prompt.contains("TESTPLAN.md")`
- `msg.contains("specs/websocket/TESTPLAN.md")`
- `log.contains("TESTPLAN.md")`
- `status.contains("TESTPLAN.md")`

### T-006 — Add prompt-structure test

New test `test_slash_spec_create_prompt_includes_testplan_section`
asserting the prompt contains `## Test Cases` and `TC-001` example ID
guidance, and mentions `status: draft` for the TESTPLAN frontmatter.

### T-007 — Add no-automated-tests test

New test `test_slash_spec_create_prompt_testplan_excludes_automated_tests`
asserting the TESTPLAN instructions in the prompt explicitly say the plan
is manual and should not include `#[test]` or `cargo test`.

### T-008 — Update TUI create test (if needed)

Check `crates/ragent-tui/tests/test_slash_commands.rs` for any test that
asserts the exact create message or prompt text; update it to account for
the new `TESTPLAN.md` mention. If no such assertion exists, this task is a
no-op.

### T-009 — Update CHANGELOG.md

Add an entry under the Unreleased section:

```
- `/spec create` now generates a `TESTPLAN.md` manual test plan alongside
  `SPEC.md` and `PLAN.md`, with step-by-step human-verifiable test cases.
```

## Definition of done

1. `build_create_prompt` instructs the agent to write `TESTPLAN.md`.
2. `build_create_message`, `build_create_log`, and `build_create_status`
   all mention `TESTPLAN.md`.
3. Existing tests pass with the new assertions.
4. New tests verify the prompt structure and the no-automated-tests rule.
5. `cargo test -p ragent-specs` and `cargo clippy -p ragent-specs` pass.