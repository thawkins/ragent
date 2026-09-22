# Implementation Plan: `/config save` and `/config list`

This plan adds two new subcommands to the existing `/config` slash command:
`save` snapshots the global `ragent.json` and `list` presents an interactive
picker to restore a backup. Each task maps to one or more requirements in
[`SPEC.md`](SPEC.md).

## Summary

| Item | Value |
|------|-------|
| Spec ID | `configsave` |
| Primary crate | `ragent-tui` |
| Key files modified | `app/slash.rs`, `app/state.rs`, `app/input_handler.rs`, `layout.rs` |
| Backup location | `<global config dir>/saves/ragent.json.[date].[time]` |
| New state struct | `ConfigSavePickerState` |
| Reused patterns | `HistoryPickerState`, `render_history_picker`, `atomic_config_update` |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Extend `/config` autocomplete with `save` and `list` subcommands | FR-002 | S | High | completed | — |
| T-002 | Implement `Config::backup_global_config()` helper in `ragent-config` | FR-001, FR-003, FR-011 | M | Critical | completed | — |
| T-003 | Add `ConfigSavePickerState` to TUI app state | FR-007, FR-008 | S | High | completed | — |
| T-004 | Implement `/config save` slash-command handler | FR-003, FR-006 | M | High | completed | T-002 |
| T-005 | Implement `/config list` slash-command handler (open picker) | FR-004, FR-006 | M | High | completed | T-003 |
| T-006 | Add config-save picker key handler in `input_handler.rs` | FR-005, FR-007 | M | Critical | completed | T-003, T-005 |
| T-007 | Render config-save picker overlay in `layout.rs` | FR-008 | M | High | completed | T-003, T-006 |
| T-008 | Implement atomic restore from selected backup | FR-005, FR-012 | M | Critical | completed | T-006, T-002 |
| T-009 | Write unit tests for backup naming and directory creation | FR-003, FR-011 | S | Medium | completed | T-002 |
| T-010 | Write integration test for save → list → restore round-trip | FR-003, FR-005, FR-006 | M | High | completed | T-008 |
| T-011 | Update QUICKSTART.md and SPEC.md with new subcommands | FR-002 | S | Low | completed | T-010 |
## Task details

### T-001 — Extend `/config` autocomplete with `save` and `list` subcommands

- In `crates/ragent-tui/src/app/slash.rs`, locate the `slash_subcommands`
  helper (around line 127) and change the `"config"` arm from
  `vec!["show".to_string()]` to
  `vec!["show".to_string(), "save".to_string(), "list".to_string()]`.
- No other autocomplete changes needed — the existing `update_slash_menu`
  already subfilters by prefix.

### T-002 — Implement `Config::backup_global_config()` helper in `ragent-config`

Add a standalone function (or a method on `Config`) in
`crates/ragent-config/src/config.rs`:

- Resolve the global config dir via `dirs::config_dir().join("ragent")`.
- Read `ragent.json` from that directory. If it does not exist, return
  `Err` with a clear message.
- Compute a timestamp string using `chrono::Utc::now()` formatted as
  `YYYY-MM-DD.HH-MM-SS` (hyphens in the time portion for Windows
  compatibility — colons are illegal in NTFS file names).
- Build the backup path as `<global config dir>/saves/ragent.json.[timestamp]`.
- Create the `saves/` directory with `std::fs::create_dir_all`.
- Write the copied file contents via a temp-file-then-rename so the backup
  is never left half-written.
- Return `Ok(PathBuf)` with the path to the new backup so the caller can
  report it.

This satisfies FR-001 (reuse the same path logic), FR-003 (copy into
`ragent.json.[date].[time]` inside `saves/`), and FR-011 (never overwrite an
existing backup — each call produces a new file name).

### T-003 — Add `ConfigSavePickerState` to TUI app state

In `crates/ragent-tui/src/app/state.rs`, add a struct mirroring
`HistoryPickerState`:

```rust
#[derive(Debug, Clone)]
pub struct ConfigSavePickerState {
    /// Backup file paths discovered in the `saves/` directory, newest first.
    pub entries: Vec<std::path::PathBuf>,
    /// Currently highlighted row.
    pub selected: usize,
    /// Scroll offset for long lists.
    pub scroll_offset: usize,
    /// Resolved global config directory (for the title display and restore target).
    pub config_dir: std::path::PathBuf,
}
```

Add a field `pub config_save_picker: Option<ConfigSavePickerState>` to the
`App` struct and initialise it to `None` in `App::new` / the `init.rs`
constructor alongside `history_picker`.

### T-004 — Implement `/config save` slash-command handler

In `execute_slash_command_inner` (`slash.rs`), extend the existing
`"config" => match args.trim()` arm (around line 858):

- Add a `"save"` arm that calls `Config::backup_global_config()` (T-002).
- On success, append an assistant text line:
  `From: /config save\n✅ Saved backup to <path>`.
- Set `self.status = "config: saved".to_string()`.
- On error (no global config file, I/O failure), append an error line and
  log via `push_log_no_agent(LogLevel::Error, …)`.

Also add a `"list"` arm that delegates to T-005.

Update the catch-all `_` arm usage string to mention `save` and `list`.

### T-005 — Implement `/config list` slash-command handler (open picker)

- Resolve the global config dir and `saves/` subdirectory.
- If the directory does not exist or contains no files matching
  `ragent.json.*`, set `self.status` and append an assistant text message
  stating no saves are available (FR-006). Do **not** open an empty picker.
- Otherwise, collect matching paths, sort them newest-first (by file
  modification time, FR-010), and construct a `ConfigSavePickerState` with
  `selected: 0`, `scroll_offset: 0`.
- Assign it to `self.config_save_picker` so the input handler and layout
  render hooks pick it up.
- Clear the input buffer to prevent stray characters.

### T-006 — Add config-save picker key handler in `input_handler.rs`

Add a `handle_config_save_picker_key(&mut self, key: KeyEvent)` method
following `handle_history_picker_key` as the template:

- `Esc` → `self.config_save_picker = None` (cancel, FR-007).
- `Up` / `k` → decrement `selected`, adjust `scroll_offset`.
- `Down` / `j` → increment `selected` (bounded by `entries.len()`).
- `Enter` → read the selected path, call the restore routine (T-008), then
  set `self.config_save_picker = None`.

Add a guard at the top of `handle_key_event` (right after the
`history_picker` guard at line ~663):

```rust
if self.config_save_picker.is_some() {
    self.handle_config_save_picker_key(key);
    self.assert_ui_invariants();
    return;
}
```

### T-007 — Render config-save picker overlay in `layout.rs`

Add a `render_config_save_picker(frame, app)` function modelled on
`render_history_picker` (line ~4708):

- Use the same `centered_rect(80, 70, area)` popup geometry.
- Title: ` Config saves (N entries) — ↑/↓ navigate · Enter restore · Esc cancel `.
- Each item shows the backup file name (and optionally the last-modified
  time parsed from the timestamp suffix, FR-009).
- Highlight the selected row with the same `Color::Black`/`bg(Cyan)` style.
- Add the render call in the top-level `render` function next to the
  history picker check (line ~73):

```rust
if app.config_save_picker.is_some() {
    render_config_save_picker(frame, app);
}
```

### T-008 — Implement atomic restore from selected backup

Add a method on `App` (or a free function in `state.rs`):

- Read the selected backup file contents.
- Validate that the target path is exactly the resolved global
  `ragent.json` (FR-012) — reject any mismatch.
- Use `atomic_config_update` (or the equivalent temp-write-rename pattern)
  to write the backup contents to the global `ragent.json`.
- On success, log via `push_log_no_agent(LogLevel::Info, "config: restored from <backup>")`.
- On failure, log an error and leave the picker open so the user can retry
  or cancel.

### T-009 — Write unit tests for backup naming and directory creation

In `crates/ragent-config/tests/test_config_save.rs`:

- Use a temp directory as a stand-in global config dir (override via a
  test-only env var or by passing the dir into the helper).
- Create a `ragent.json` with known content, call the backup helper, and
  assert:
  - The `saves/` directory was created.
  - The backup file name matches `ragent.json.YYYY-MM-DD.HH-MM-SS`.
  - The backup content equals the source content (byte-for-byte).
  - A second call produces a different file name (no overwrite, FR-011).

### T-010 — Write integration test for save → list → restore round-trip

In `crates/ragent-tui/tests/test_config_save_list.rs`:

- Construct an `App` with a temp-dir-backed global config.
- Simulate `/config save`, assert a backup file appears.
- Simulate `/config list`, assert `config_save_picker` is `Some` with one
  entry.
- Simulate pressing `Enter` on that entry, assert the global `ragent.json`
  now matches the backup content.
- Simulate `/config list` again and assert the picker is non-empty.
- Simulate `Esc` and assert the picker closes without restoring.

### T-011 — Update QUICKSTART.md and SPEC.md with new subcommands

- Add `/config save` and `/config list` to the slash-command table in
  QUICKSTART.md.
- Add a short note in SPEC.md's configuration section mentioning the backup
  directory and restore flow.

## Definition of done

- All tasks T-001 through T-010 are completed and their tests pass.
- `cargo test -p ragent-config test_config_save` passes.
- `cargo test -p ragent-tui test_config_save_list` passes.
- `cargo clippy` produces no new warnings in the modified files.
- `cargo fmt --check` passes.
- The `/config` usage string lists `show`, `save`, and `list`.
- Typing `/config ` in the TUI shows all three subcommands in autocomplete.
- `/config save` writes a timestamped backup into `saves/` and reports the
  path.
- `/config list` opens a picker; `Enter` restores; `Esc` cancels.