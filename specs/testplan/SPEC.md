---
status: draft
audit:
  - { time: 1786472988, from: "none", to: "draft", actor: "system" }
---
# Extend `/spec create` to Generate a TESTPLAN.md Manual Test Plan

## Context

When a user runs `/spec create <id> <feature>`, ragent delegates to an
explore agent using a prompt built by
`SpecCommand::build_create_prompt` (`crates/ragent-specs/src/commands.rs`).
The prompt currently instructs the agent to write two files:

1. `specs/<id>/SPEC.md` — an EARS-notation requirements specification.
2. `specs/<id>/PLAN.md` — an implementation plan with a task table.

There is no companion artefact that describes **manual** verification steps a
human tester can follow to confirm the feature works end-to-end. Unit and
integration tests cover developer-facing regressions, but UI navigation,
data entry, and observable behaviour in the running TUI or HTTP server are
not captured anywhere in the spec lifecycle.

This specification extends the create prompt so that a third file,
`specs/<id>/TESTPLAN.md`, is always generated alongside `SPEC.md` and
`PLAN.md`. The file contains a structured manual test plan with
step-by-step instructions, expected results, and the data to enter — not
automated test code.

## Requirements

### Ubiquitous requirements

FR-001: The `/spec create` command **shall** always produce a
`TESTPLAN.md` file in the same spec directory as `SPEC.md` and `PLAN.md`
for every spec it creates.

FR-002: The generated `TESTPLAN.md` **shall** begin with YAML
frontmatter containing `status: draft` so it is consistent with the other
generated spec artefacts.

### Event-driven requirements

FR-003: When `/spec create <id> <feature>` is invoked, the prompt sent to
the explore agent **shall** instruct the agent to write
`specs/<id>/TESTPLAN.md` using the `write` tool alongside the existing
`SPEC.md` and `PLAN.md` writes.

FR-004: When `/spec create` completes, the user-facing status message and
log entry **shall** mention `TESTPLAN.md` so the tester knows a manual
test plan was produced.

### State-driven requirements

FR-005: While `TESTPLAN.md` is being generated, it **shall** contain a
`## Test Cases` section with one or more manual test cases, each having an
ID (`TC-001`, `TC-002`, …), a title, preconditions, step-by-step
instructions, test data to enter, and expected results.

FR-006: While the feature described in `SPEC.md` includes user-interface
navigation, the corresponding `TESTPLAN.md` test cases **shall** enumerate
every UI navigation step (keys pressed, menus opened, dialogs interacted
with) and the exact data to enter into each field.

### Optional requirements

FR-007: The `TESTPLAN.md` **may** include a `## Prerequisites` section
listing environment setup, provider configuration, or sample files needed
before the manual tests can be executed.

FR-008: The `TESTPLAN.md` **may** include a `## Cleanup` section
describing teardown steps to run after the manual tests complete.

### Unwanted requirements

FR-009: The `TESTPLAN.md` **shall not** contain automated test code,
`#[test]` functions, or references to `cargo test`; it is a human-readable
manual test plan only.

FR-010: The generation of `TESTPLAN.md` **shall not** alter the existing
content, numbering, or structure of `SPEC.md` or `PLAN.md`; the change is
strictly additive to the create prompt.

## Glossary

- **Manual test plan** — A human-readable document of step-by-step
  instructions a tester follows to verify a feature works, distinct from
  automated unit or integration tests.
- **EARS** — Easy Approach to Requirements Syntax, the notation used for
  requirements in `SPEC.md`.
- **Explore agent** — The agent type that receives the create prompt and
  writes the spec files.