# BORDERFIX Plan

## Objective

Audit every ratatui panel and widget in the TUI to ensure they render with proper Unicode box-drawing border characters (`┌─┐│└┘` etc.) and eliminate any remaining simple ASCII fallback (`+|-|+`).

---

## Panel Inventory

### Side Panels (layout.rs) — all already use proper Unicode borders via `Block::default().borders(Borders::ALL)`

| Panel | Function | Border Status |
|-------|----------|---------------|
| Telemetry | `render_telemetry_panel` | Unicode ✓ |
| Context | `render_context_panel` | Unicode ✓ |
| Log | `render_log_panel` | Unicode ✓ |
| Tasks | `render_tasks_panel` | Unicode ✓ |
| Profile | `render_profile_panel` | Unicode ✓ |
| Memory | `render_memory_panel` | Unicode ✓ |

### Modal / Popup Panels (layout.rs)

| Panel | Function | Border Status |
|-------|----------|---------------|
| Backend picker | `render_backend_panel` | Unicode ✓ |
| Connector catalogue | `render_connector_catalogue_panel` | **ASCII — FIX NEEDED** |
| Queue show | `render_queue_show_panel` | Unicode ✓ |
| Plugin store | `render_plugin_store_panel` | **ASCII — FIX NEEDED** |
| Shortcuts help | `render_shortcuts_panel` | Unicode ✓ |
| Plugins list | `render_plugins_list_panel` | Unicode ✓ |

### Sub-Panels

| Panel | File | Function | Border Status |
|-------|------|----------|---------------|
| Teams | `layout_teams.rs` | `render_teams_subpanel` | Unicode ✓ |
| Active agents | `layout_active_agents.rs` | `render_active_agents_subpanel` | No border (text-only) ✓ |

### Reusable Widgets

| Widget | File | Border Status |
|--------|------|---------------|
| Dialog | `widgets/dialog.rs` | Unicode ✓ |
| Permission dialog | `widgets/permission_dialog.rs` | Unicode ✓ |
| Selectable list | `widgets/selectable_list.rs` | Unicode ✓ |
| Button (bordered mode) | `widgets/button.rs` | Unicode ✓ |

### Dialogs / Overlays with Borders (layout.rs) — all Unicode ✓

`render_yes_no_confirm`, `render_queue_clear_confirm`, `render_memory_clear_confirm`, `render_provider_setup_dialog`, `render_router_save_dialog`, `render_memory_view_overlay`, `render_memory_delete_dialog`, `render_stop_confirm_dialog`, `render_research_view_overlay`, `render_model_loading_popup`, `render_model_download_popup`, `render_agents_window_overlay`, `render_teams_window_overlay`, `render_permission_dialog`, `render_question_dialog`, `render_force_cleanup_dialog`, `render_loop_setup_dialog`, `render_mcp_discover_dialog`, `render_history_picker`, `render_config_save_picker`, `render_slash_menu`, `render_file_menu`, `render_context_menu`.

---

## Issues Found

### Issue 1: Connector Catalogue Panel uses ASCII borders

- **File:** `crates/ragent-tui/src/layout.rs`
- **Function:** `render_connector_catalogue_panel` (line 562)
- **Line:** 594 — `.border_set(PLUGIN_STORE_ASCII_BORDER)`
- **Impact:** Renders with `+`, `-`, `|` instead of Unicode box-drawing

### Issue 2: Plugin Store Panel uses ASCII borders

- **File:** `crates/ragent-tui/src/layout.rs`
- **Function:** `render_plugin_store_panel` (line 1219)
- **Line:** 1253 — `.border_set(PLUGIN_STORE_ASCII_BORDER)`
- **Impact:** Renders with `+`, `-`, `|` instead of Unicode box-drawing

### Root Cause

A single constant `PLUGIN_STORE_ASCII_BORDER` (layout.rs:178) defines the ASCII border set. It was introduced to satisfy spec `pluginstores` FR-004 which required "only ASCII glyphs" for the browse panel. This requirement is now being reversed.

---

## Tasks

### T-001 — Remove ASCII border constant
- **File:** `crates/ragent-tui/src/layout.rs`
- **Action:** Delete `PLUGIN_STORE_ASCII_BORDER` constant (lines 173–187) and its doc comment
- **Acceptance:** Constant no longer exists; `cargo check` passes

### T-002 — Convert connector catalogue panel to Unicode borders
- **File:** `crates/ragent-tui/src/layout.rs`
- **Action:** Remove `.border_set(PLUGIN_STORE_ASCII_BORDER)` call from `render_connector_catalogue_panel` (line 594)
- **Acceptance:** Panel renders with default ratatui Unicode borders; `cargo check` passes

### T-003 — Convert plugin store panel to Unicode borders
- **File:** `crates/ragent-tui/src/layout.rs`
- **Action:** Remove `.border_set(PLUGIN_STORE_ASCII_BORDER)` call from `render_plugin_store_panel` (line 1253)
- **Acceptance:** Panel renders with default ratatui Unicode borders; `cargo check` passes

### T-004 — Update inline ASCII-only comments
- **File:** `crates/ragent-tui/src/layout.rs`
- **Action:** Review and update/remove the 15 "ASCII only" / "Only ASCII glyphs" doc comments that are now outdated (lines 109, 114, 118, 124, 173+, 398, 533, 807, 858, 1186, 1527, 1669, 1729, 1794, 1855)
- **Acceptance:** Comments no longer claim ASCII-only rendering where Unicode is now used

### T-005 — Update spec `pluginstores` FR-004
- **File:** `specs/pluginstores/SPEC.md`
- **Action:** Remove "using only ASCII glyphs" clause from FR-004 (around line 158); update any acceptance criteria that reference ASCII borders
- **Acceptance:** Spec no longer mandates ASCII-only rendering for the browse panel

### T-006 — Update test plan if needed
- **File:** `specs/pluginstores/TESTPLAN.md`
- **Action:** Search for any test cases asserting ASCII output or box-drawing absence and update them
- **Acceptance:** No test cases enforce ASCII-only borders

### T-007 — Regression test
- **Action:** Run `cargo test -p ragent-tui --test all` and verify zero failures
- **Action:** Run `cargo fmt --check` and verify clean
- **Acceptance:** All tests pass; formatting clean

---

## Milestones

| Milestone | Tasks | Definition of Done |
|-----------|-------|----------------------|
| M1 — Code fix complete | T-001, T-002, T-003, T-004 | `cargo check` passes; both panels render Unicode borders |
| M2 — Spec updated | T-005, T-006 | SPEC.md and TESTPLAN.md no longer require ASCII glyphs |
| M3 — Verified | T-007 | `cargo test -p ragent-tui --test all` passes; `cargo fmt --check` clean |

---

## Estimated Effort

- T-001 through T-003: ~5 minutes (pure deletions)
- T-004: ~10 minutes (comment review)
- T-005, T-006: ~10 minutes (spec edits)
- T-007: ~15 minutes (build + test)
- **Total:** ~40 minutes
