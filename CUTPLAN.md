# CUTPLAN — Cut / Copy / Paste Remediation Plan

**Scope:** All cut/copy/paste code paths in `crates/ragent-tui`.
**Goal:** Remove duplicated clipboard helpers, unify selection handling, close
UI/UX gaps, and remediate the temp-file security/performance findings that
touch the clipboard image paste path.

## Executive Summary

The TUI currently has **three independent clipboard implementations** and
several related behavioural inconsistencies:

1. `crates/ragent-tui/src/app/session_ops.rs` ��� `App::{get,set}_clipboard`,
   `paste_text_from_clipboard`, `paste_image_from_clipboard`.
2. `crates/ragent-tui/src/input_field.rs` — `InputField::{get,set}_clipboard`,
   `paste_clipboard`, `copy_selection`, `cut_selection`.
3. `crates/ragent-tui/src/input.rs` — inline `arboard` code for copying a
   Copilot/GitHub device-flow user code (around lines 1195–1211).

In addition, text paste from the terminal's bracketed-paste event does not
replace an active selection, the context-menu paste path is missing support for
the telemetry-setup dialog, and clipboard image temp files are created outside
`target/temp/` without explicit restrictive permissions.

## Inconsistencies and Gaps

| ID | Issue | Files | Severity |
|----|-------|-------|----------|
| C-01 | Duplicated `get/set_clipboard` helpers: `App`, `InputField`, and device-flow inline code all contain the same `#[cfg(target_os = "linux")]` X11 wait logic. | `session_ops.rs:1320–1341`, `input_field.rs:291–316`, `input.rs:1195–1211` | Medium |
| C-02 | Linux clipboard set behaviour differs: `App::set_clipboard` spawns a thread and uses `set().wait().text()`; `InputField::set_clipboard` does not wait, risking lost data under X11. | `session_ops.rs:1320–1335`, `input_field.rs:307–316` | Medium |
| C-03 | Paste method naming is inconsistent (`InputField::paste_clipboard` vs `App::paste_text_from_clipboard`). | `input_field.rs:284`, `session_ops.rs:656` | Low |
| C-04 | Bracketed terminal paste (`CtEvent::Paste`) inserts text but does not replace the active keyboard/mouse selection, unlike Ctrl+V and context-menu Paste. | `lib.rs:655–659` | Medium |
| C-05 | Context-menu `Paste` in provider setup handles `EnterKey` and `GitLabSetup` but not `TelemetrySetup`, even though the menu-enable check includes telemetry. | `input_handler.rs:553–558`, `session_ops.rs:1424–1430` | Medium |
| C-06 | Provider-setup paste has two separate code paths: `handle_provider_setup_key` calls `InputField::paste_clipboard`, while right-click uses `execute_context_action` → `paste_provider_setup_from_clipboard`. | `input.rs:1169–1175`, `session_ops.rs:1404–1431` | Low |
| C-07 | Clipboard image temp files are written to the OS temp directory, not the project-mandated `target/temp/`, and permissions default to `umask`. | `state.rs:186–199` | Medium |
| C-08 | `save_clipboard_image_to_temp` copies the pixel buffer with `to_vec()` before encoding, temporarily doubling memory for large images. | `state.rs:182` | Low |
| C-09 | `paste_image_from_clipboard` only resolves `file://` and absolute/relative paths; it does not validate that a resolved file is still inside a safe scope. | `session_ops.rs:1346–1400` | Low |
| C-10 | `App::paste_image_from_clipboard` is `pub` while related text helpers are `pub(crate)`, creating inconsistent visibility. | `session_ops.rs:1346` | Low |

## Remediation Plan

### Milestone 1 — Centralise clipboard helpers
**Goal:** One correct clipboard implementation used everywhere.

- **Task 1.1** Create `crates/ragent-tui/src/clipboard.rs` exposing:
  - `pub fn get_clipboard_text() -> Option<String>`
  - `pub fn set_clipboard_text(text: &str)` (spawns a thread, uses
    `set().wait().text()` on Linux, `set_text()` elsewhere)
  - `pub fn clipboard_image_to_temp(img: &ImageData<'_>) -> Result<PathBuf>`
    (moves image handling here from `state.rs`)
- **Task 1.2** Replace `App::{get,set}_clipboard` with thin wrappers calling the
  shared helpers; keep them `pub(crate)` to avoid breaking call sites.
- **Task 1.3** Replace `InputField::{get,set}_clipboard` with calls to the shared
  helpers. This fixes the missing `wait()` on Linux.
- **Task 1.4** Replace the inline device-flow copy in `input.rs` with the shared
  `set_clipboard_text` helper.
- **Task 1.5** Add unit tests for the new module covering text round-trip,
  repeated writes, and empty text.

**Acceptance:** `grep` shows zero independent `arboard::Clipboard::new()` calls
outside `clipboard.rs`; all existing tests pass.

### Milestone 2 — Unify text-paste behaviour
**Goal:** Every paste entry point replaces an active selection and strips `\r`.

- **Task 2.1** Rename `InputField::paste_clipboard` to
  `InputField::paste_text_from_clipboard` (or add an alias) to match the App API.
- **Task 2.2** Update the bracketed-paste handler in `lib.rs` to first remove
  any active selection (`kb_selection_char_range` and `input_selection_char_range`)
  before calling `insert_text_at_cursor`.
- **Task 2.3** Verify that Ctrl+V, right-click Paste, terminal paste, and
  provider-setup paste all produce identical results for the same clipboard
  content.

**Acceptance:** New test asserts that a bracketed-paste event replaces both
keyboard and mouse selections; existing selection tests still pass.

### Milestone 3 — Fix provider-setup paste gaps
**Goal:** All setup dialogs support context-menu and keyboard paste consistently.

- **Task 3.1** Extend `execute_context_action(ContextAction::Paste)` so the
  `TelemetrySetup` variant is routed through `paste_text_into_provider_setup`,
  matching `EnterKey` and `GitLabSetup`.
- **Task 3.2** Add a test that opens `ProviderSetupStep::TelemetrySetup` with a
  mock clipboard and asserts that `execute_context_action(ContextAction::Paste)`
  writes the clipboard text into the active telemetry field.

**Acceptance:** The test from Task 3.2 passes; `TelemetrySetup` no longer falls
through to the generic input-paste branch.

### Milestone 4 — Harden clipboard image temp files
**Goal:** Align with `AGENTS.md` temp-file guidance and `SECPLAN` / `COMPLIANCE`
Task 4.2.

- **Task 4.1** Move the temp-file directory to `target/temp/`:
  - Use `std::env::current_dir()` / `target/temp` as the parent when creating the
    temp file.
  - Ensure `target/temp/` is in `.gitignore` (it already should be).
- **Task 4.2** On Unix, set file permissions to `0o600` after writing the PNG.
- **Task 4.3** Investigate and implement an auto-prune policy: either delete the
  temp file after the attached image is sent, or on startup clean orphaned
  `ragent_paste_*.png` files older than a configurable age. Document the chosen
  policy.
- **Task 4.4** Update `save_clipboard_image_to_temp` (or its successor in
  `clipboard.rs`) to avoid the `to_vec()` copy by consuming `ImageData.bytes`
  when possible.
- **Task 4.5** Extend `tests/test_clipboard_tempfile.rs` to assert:
  - The file is created under `target/temp/`.
  - On Unix, permissions are `0o600`.
  - No extra buffer copy can be detected by a size/dimension mismatch test.

**Acceptance:** Security reviewer sign-off that clipboard temp files no longer
rely on `umask`; performance reviewer confirms the extra copy is removed.

### Milestone 5 — Visibility, validation, and docs
**Goal:** Polish the public surface and update user-facing docs.

- **Task 5.1** Make `App::paste_image_from_clipboard` `pub(crate)` unless there
  is a genuine external consumer.
- **Task 5.2** Add a lightweight path check in `paste_image_from_clipboard`: if a
  resolved path is outside the working directory or home directory, log a warning
  but still attach (do not silently reject user-intended files).
- **Task 5.3** Update `TUI-QUICKSTART.md` and `QUICKSTART.md` to clarify:
  - Right-click = copy/cut/paste on the current selection.
  - `Alt+V` = paste image attachment.
  - Terminal bracketed paste is supported and behaves like Ctrl+V.
- **Task 5.4** Update `CHANGELOG.md` once the plan is implemented.

**Acceptance:** `cargo doc` and `cargo test -p ragent-tui` pass; docs reflect
the unified behaviour.

## Dependencies on Other Plans

- `SECPLAN.md` M5.4 (TUI secret-mask toggle & clipboard temp hardening) —
  Milestone 4 here implements the clipboard temp-file portion.
- `crates/ragent-tui/COMPLIANCE.md` Task 4.2 (restrictive clipboard temp
  permissions and lifecycle docs) — covered by Milestone 4.
- `crates/ragent-tui/performance_findings.md` item 7 (clipboard image buffer
  copy) — covered by Task 4.4.

## Success Criteria

1. `arboard` is only instantiated inside a single `clipboard.rs` module.
2. All text-paste paths strip `\r` and replace active selections.
3. `TelemetrySetup` can be pasted via right-click context menu.
4. Clipboard image temp files live in `target/temp/` with `0o600` permissions
   on Unix.
5. The image pixel buffer is not cloned before PNG encoding.
6. New unit tests cover the centralised helpers, selection replacement, and
   telemetry setup paste.
7. `cargo test -p ragent-tui` and `cargo clippy -p ragent-tui` pass.
