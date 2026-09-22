---
status: draft
audit:
  - { time: 1784332295, from: "none", to: "draft", actor: "system" }
---
# `/config save` and `/config list` Slash Commands

## Context

ragent stores its global configuration at `~/.config/ragent/ragent.json` (the
exact path depends on `dirs::config_dir()` for the host platform: `~/Library/
Application Support/ragent/ragent.json` on macOS, `%APPDATA%\ragent\ragent.json`
on Windows). Users editing this file — adding providers, adjusting permission
rules, tuning compaction or memory settings — currently have no built-in safety
net: a bad edit or an accidental overwrite means the working configuration is
lost with no recovery path.

The existing `/config show` slash command displays resolved paths and
configuration summaries but offers no backup/restore capability. The
`/init config` command creates a default global config from scratch when none
exists, but does not help users who already have a config they wish to
preserve before making changes.

This specification defines two new subcommands that complete the configuration
lifecycle: `/config save` snapshots the current global `ragent.json` into a
timestamped backup file, and `/config list` presents an interactive picker of
all saved backups with the ability to restore any one over the active global
config.

The implementation builds on established patterns already in the codebase:

- **Slash command dispatch** — `execute_slash_command_inner` in
  `crates/ragent-tui/src/app/slash.rs` already handles `"config"` with a
  subcommand `match` arm (currently `show` and a catch-all usage string).
  `save` and `list` will be added as new subcommand arms in that same match.

- **Autocomplete suggestions** — the `slash_subcommands` helper at the top of
  `slash.rs` already returns a `vec!["show".to_string()]` for `"config"`; this
  will be extended to include `"save"` and `"list"`.

- **Interactive picker overlay** — the `HistoryPickerState` struct
  (`crates/ragent-tui/src/app/state.rs:840`) plus its key handler
  (`handle_history_picker_key` in `input_handler.rs:24`) and renderer
  (`render_history_picker` in `layout.rs:4708`) provide a proven template for a
  keyboard-navigable, popup list with Up/Down/Enter/Esc semantics. A new
  `ConfigSavePickerState` will follow the same shape and integrate through the
  same input-handler guard and layout-render hook.

- **Global config path resolution** — `/config show`, `/init config`, and the
  `telemetry_config_source_path` helper all resolve the global config via
  `dirs::config_dir().map(|d| d.join("ragent").join("ragent.json"))`. The new
  subcommands will reuse that resolution for both the source file and the
  backup directory (the `saves/` subfolder lives alongside `ragent.json`).

- **Atomic file writes** — `atomic_config_update` in `state.rs:47` and
  `Config::write_config_if_changed` in `config.rs:1246` demonstrate the
  project's lock-then-rename discipline for config mutations. The restore
  operation will follow the same pattern to avoid writing a partial
  `ragent.json`.

## Requirements

### Ubiquitous requirements

FR-001: The configuration backup subsystem **shall** always resolve the global
`ragent.json` path via the same platform-aware logic used by `/config show` and
`/init config` (`dirs::config_dir().join("ragent").join("ragent.json")`).

FR-002: The `/config` slash command **shall** accept `save` and `list` as valid
subcommands in addition to the existing `show` subcommand.

### Event-driven requirements

FR-003: When the user invokes `/config save`, the system **shall** copy the
current contents of the global `ragent.json` file into a new file named
`ragent.json.[date].[time]` inside a `saves/` subdirectory located in the same
directory as `ragent.json`, creating the `saves/` directory if it does not
already exist.

FR-004: When the user invokes `/config list`, the system **shall** display an
interactive picker listing every file in the `saves/` subdirectory whose name
matches the `ragent.json.*` backup pattern.

FR-005: When the user presses `Enter` on a highlighted entry in the config-save
picker, the system **shall** overwrite the active global `ragent.json` with the
contents of the selected backup file using an atomic write (write-to-temp,
fsync, rename), then close the picker and emit a confirmation log entry.

FR-006: When the `saves/` directory does not exist or contains no matching
backup files, the system **shall** display a user-facing message stating that
no saved configurations are available rather than opening an empty picker.

### State-driven requirements

FR-007: While the config-save picker is open and has the keyboard focus, the
system **shall** intercept `Up`/`Down` (and `k`/`j`) to move the selection
cursor, `Enter` to restore, and `Esc` to cancel — mirroring the behaviour of
the existing history picker.

FR-008: While a config-save picker is open, the system **shall** render it as a
centered popup overlay on top of the normal chat layout, following the same
visual conventions (border colour, highlighted selected row, entry count in the
title) as `render_history_picker`.

### Optional requirements

FR-009: The system **may** include the timestamp of each backup (parsed from
the file name or the file's last-modified time) in the picker display alongside
the file name so users can identify recent saves.

FR-010: The system **may** sort the backup list with the most recent save first,
consistent with the newest-first ordering used by the history picker.

### Unwanted requirements

FR-011: The system **shall not** delete or overwrite any existing backup file as
a side-effect of `/config save`; each save produces a new uniquely named file.

FR-012: The system **shall not** restore a backup without first confirming the
overwrite target path is the resolved global `ragent.json`; restore must never
write to an arbitrary or attacker-controlled path.

FR-013: The system **shall not** persist the picker state across TUI restarts;
if the picker is open when the application exits, it is discarded.

## Glossary

- **Global config directory** — the directory returned by
  `dirs::config_dir().join("ragent")`, e.g. `~/.config/ragent` on Linux.
- **Global `ragent.json`** — the primary global configuration file at
  `<global config directory>/ragent.json`.
- **`saves/` subdirectory** — a subfolder of the global config directory
  (`<global config directory>/saves/`) that holds timestamped backup copies.
- **Backup file** — a file named `ragent.json.[date].[time]` where `[date]` is
  `YYYY-MM-DD` and `[time]` is `HH-MM-SS` (using hyphens to avoid colons which
  are illegal in Windows file names).
- **Config-save picker** — the interactive TUI overlay, analogous to the
  history picker, that lists backups and lets the user restore one.

## References

- `crates/ragent-tui/src/app/slash.rs` — `/config show` handler (line ~858),
  autocomplete suggestions (line ~127).
- `crates/ragent-tui/src/app/state.rs` — `HistoryPickerState` struct (line ~840).
- `crates/ragent-tui/src/app/input_handler.rs` — `handle_history_picker_key`
  (line ~24), key-dispatch guard (line ~663).
- `crates/ragent-tui/src/layout.rs` — `render_history_picker` (line ~4708),
  overlay render hook (line ~73).
- `crates/ragent-config/src/config.rs` — `Config::load` (line ~1030),
  `Config::save` (line ~1179), global path resolution (line ~1036).
- `crates/ragent-tui/src/app/state.rs` — `atomic_config_update` (line ~47).