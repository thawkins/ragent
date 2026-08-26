# ragent-tui

Ratatui-based terminal interface for ragent. Provides the full-screen TUI
with provider setup dialog, slash-command autocomplete, streaming chat,
markdown rendering, tool-call display, permission dialogs, and status bar.

## Workspace Dependencies

- ragent-agent
- ragent-bench
- ragent-team
- ragent-config
- ragent-types
- ragent-storage
- ragent-codeindex
- ragent-tools-core
- ragent-tools-extended
- ragent-tools-vcs
- ragent-server
- ragent-telemetry
- ragent-prompt_opt
- ragent-specs
- ragent-research
- ragent-llm

## External Dependencies

- axum, dirs, tokio, serde, serde_json, anyhow, tracing, tracing-subscriber
- ratatui, crossterm, tokio-stream, futures
- chrono, rand, arboard, image
- html2text, pulldown-cmark, async-trait, fs2, lru, regex, uuid, tempfile

Dev-dependencies: tempfile, criterion, parking_lot, filetime.
Build-dependencies: chrono.

## Public API (crate root)

### Modules

- **app** — Application state and event handling; contains `App` and all UI state types.
- **clipboard** — Shared clipboard helpers (text/image read/write, temp file pruning).
- **input** — Keyboard input handling; maps key events to `InputAction`s.
- **input_field** — Single-line text input field with full editing support.
- **layout** — Main TUI layout and rendering engine (`render`, `render_messages`).
- **layout_active_agents** — Subpanel rendering for active background agents.
- **layout_statusbar** — Status bar rendering engine with modular 3-section layout.
- **layout_teams** — Subpanel rendering for team/swarm members.
- **logo** — ASCII art logo for the ragent home screen.
- **panels** — TUI overlay panels (re-exports from app state).
- **research_adapter** — Re-exports shared research adapter from `ragent_agent`.
- **research_progress** — Research progress tracking for the `/research create` slash command.
- **theme** — Centralized theming: colors, typography, spacing, icons, status categories.
- **tips** — Rotating tips displayed on the home screen.
- **tracing_layer** — Custom tracing subscriber layer forwarding log records to the TUI.
- **utils** — Layout utilities (responsive breakpoints, centered rect, truncation).
- **widgets** — Reusable ratatui widgets (button, dialog, message widget, permission dialog, selectable list).

### Crate-root items

- **App** (struct) — Core TUI application state holding messages, input, scroll, permissions, token counters, and EventBus reference.
- **TerminalGuard** (struct) — RAII guard that sets up the terminal (raw mode, alternate screen, mouse capture) and restores it on drop.
- **run_tui** (async fn) — Main entry point: enters alternate screen, creates `App`, runs the event loop until quit, restores terminal on exit.

## Module: app

Re-exports from submodules. Notable public items:

- **App** (struct) — Core TUI application state.
- **MdWorker** (struct) — Background markdown rendering worker.
- **StatusBarCache** (struct) / **ModelPickerRowsCache** (struct) — Cached render data.
- **cron** (sub-module) — `CronSchedulerHandle`, `start_cron_scheduler`.
- **skillgen** (sub-module) — `SkillgenResult`, `generate_graphify_skill`, `generate_graphify_skill_in`.
- **sanitize_for_display** (fn) / **image_dimensions_or_placeholder** (fn).
- **app/state** items: `ScreenMode` (enum), `LogEntry` (struct), `BgTaskView` (struct), `LlmRequestStat` (struct), `LlmStatsSummary` (struct), `ModelPickerEntry` (struct), `SlashCommandDef` (struct), `SLASH_COMMANDS` (const), `SlashMenuState` (struct), `QuestionRequest` (struct), `App` (struct), `ResearchViewState` (struct), `PlanApprovalState` (struct), `SpecImplState` (struct), `RoleMode` (enum), and many UI state types.

## Module: clipboard

- **get_clipboard_text** / **get_clipboard_text_sync** / **set_clipboard_text** / **set_clipboard_text_sync** (fns).
- **get_clipboard_image** / **clipboard_image_to_temp** (fns).
- **prune_clipboard_temp_files** / **prune_clipboard_temp_files_in** (fns).
- **ClipboardTestOverrideGuard** (struct) / **set_clipboard_text_test_override** (fn).
- **CLIPBOARD_TEMP_MAX_AGE** (const).

## Module: input

- **InputAction** (enum) — High-level action (SendMessage, Quit, ScrollUp, SlashCommand, CancelAgent, etc.).
- **handle_key** (fn) — Maps a `KeyEvent` to an `InputAction`.

## Module: input_field

- **InputField** (struct) — Single-line text input with cursor, selection, clipboard, word navigation.

## Module: layout

- **render** (fn) — Main TUI frame renderer.
- **render_messages** (fn) — Chat message list panel renderer.
- **markdown_to_lines_testable** (fn) — Markdown to ratatui `Line`s.

## Module: layout_statusbar

- **StatusBarConfig** (struct) / **ResponsiveMode** (enum) / **render_status_bar_v2** (fn).
- Sub-modules: `colors`, `indicators`, `spinner`, `abbreviations`.

## Module: theme

- **ThemeMode** (enum) / **StatusCategory** (enum) / **StatusMessage** (struct) / **StatusHistory** (struct).
- Style functions: `heading`, `emphasis`, `muted`, `error`, `success`, `warning`, `info`, `think`, `loading`, `disabled`.
- Constants: `SPACING_*`, `LAYOUT_*`, `ICON_*`, `LOADING_FRAMES`.
- **loading_frame** (fn) — Current spinner frame.
- Sub-modules: `status`, `colors`, `high_contrast`, `focus`, `accessibility`.

## Module: widgets

- **button** — `ButtonVariant` (enum), `ButtonState` (enum), `Button` (struct), `ButtonBar` (struct).
- **dialog** — `DialogVariant` (enum), `DialogSize` (enum), `Dialog` (struct), `DialogContent` (enum).
- **message_widget** — `MessageWidget` (struct), `tool_input_summary`, `tool_result_summary`, `pluralize`, `canonical_tool_name`.
- **permission_dialog** — `PermissionDialog` (struct).
- **selectable_list** — `SelectableList` (struct), `SelectableListRender` (struct).

## Module: utils

- **ResponsiveBreakpoint** (enum) / **is_below_minimum_size** (fn).
- **centered_rect** / **centered_rect_max** (fns).
- **truncate_with_ellipsis** (fn).

## Module: tracing_layer

- **TuiLogRecord** (struct) / **TuiLogReceiver** (type) / **tui_log_channel** (fn) / **TuiTracingLayer** (struct).

## Module: research_progress

- **StepStatus** (enum) / **ResearchStep** (struct) / **ResearchProgress** (struct).
- **encode_progress_event** / **decode_progress_event** (fns) / **DecodedProgress** (struct).
- **PROGRESS_SENTINEL** (const).