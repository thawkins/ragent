# TUI Usability Audit & Remediation Plan

**Document:** TUIPLAN.md  
**Date:** 2025-01-XX  
**Status:** Draft  
**Scope:** ragent-tui crate usability audit focusing on UI consistency, accessibility, and TUI best practices

---

## Executive Summary

This document outlines a comprehensive usability audit of the ragent TUI interface, identifying deviations from established TUI best practices and providing a structured remediation plan with defined milestones and tasks.

**Key Findings:**
- 47 instances of hardcoded colors bypassing the theme system
- Inconsistent keyboard shortcut patterns across dialogs
- 12 different dialog rendering approaches
- Missing focus indicators in several interactive elements
- No high contrast or accessibility modes
- Inconsistent spacing and layout tokens

---

## Phase 1: Theme System Consolidation

**Goal:** Establish a single source of truth for all visual styling

### Milestone 1.1: Audit Hardcoded Colors
**Priority:** P0 (Critical)  
**Estimated Effort:** 2 days

#### Tasks

1. **T1.1.1 - Replace hardcoded colors in `layout.rs`**
   - **Location:** `crates/ragent-tui/src/layout.rs`
   - **Issue:** 34 instances of `Color::Cyan`, `Color::Yellow`, `Color::DarkGray`, `Color::Green`, `Color::Red`, `Color::Magenta`, `Color::Blue`
   - **Remediation:** Replace with theme constants:
     - `Color::Cyan` → `crate::theme::colors::PRIMARY` or `DIALOG_INFO`
     - `Color::Yellow` → `crate::theme::colors::FOCUS_COLOR` or `DIALOG_WARNING`
     - `Color::DarkGray` → `crate::theme::colors::HINT`
     - `Color::Green` → `crate::theme::colors::DIALOG_SUCCESS` or `status::SUCCESS`
     - `Color::Red` → `crate::theme::colors::DIALOG_DANGER` or `status::ERROR`
   - **Lines:** 226, 339, 391, 496, 565, 622, 670, 779, 810, 900, 993, 1412, 1658, 1694, 1718, 1737, 1761, 2347, 2486, 2597, 2664, 2803, 2967, 3087, 3242, 3339, 3445

2. **T1.1.2 - Replace hardcoded colors in `message_widget.rs`**
   - **Location:** `crates/ragent-tui/src/widgets/message_widget.rs`
   - **Issue:** 25+ instances of inline color definitions
   - **Remediation:** Use semantic color names from theme:
     - Tool status colors → `status::SUCCESS`, `status::ERROR`
     - Text hints → `colors::HINT`
     - Selection → `colors::SELECTION_BG`
   - **Lines:** 1501, 1505, 1515, 1522, 1555-1568, 1587, 1595, 1603, 1612-1619, 1626, 1680-1725, 1734, 1744, 1756, 1765, 1774

3. **T1.1.3 - Replace hardcoded colors in `layout_teams.rs`**
   - **Location:** `crates/ragent-tui/src/layout_teams.rs`
   - **Issue:** Status colors mapped directly to Color constants
   - **Remediation:** Map through theme status colors
   - **Lines:** 42-49, 127, 136, 145, 149, 153, 157, 161, 163-164, 174-190, 233-235, 245-325

4. **T1.1.4 - Replace hardcoded colors in `layout_active_agents.rs`**
   - **Location:** `crates/ragent-tui/src/layout_active_agents.rs`
   - **Issue:** Direct Color usage for task status indicators
   - **Remediation:** Use semantic status colors from theme
   - **Lines:** 86-105, 169, 177, 182, 184, 187, 194, 225

5. **T1.1.5 - Audit and fix `permission_dialog.rs`**
   - **Location:** `crates/ragent-tui/src/widgets/permission_dialog.rs`
   - **Issue:** Uses hardcoded `Color::Yellow`, `Color::Cyan` instead of theme
   - **Remediation:** Use `colors::FOCUS_COLOR`, `colors::PRIMARY`
   - **Lines:** 76, 85, 92

### Milestone 1.2: Extend Theme System
**Priority:** P1 (High)  
**Estimated Effort:** 1 day  
**Status:** ✅ Complete

#### Tasks

6. **T1.2.1 - Add missing semantic color categories**
   - ✅ Added `borders` module with DEFAULT, ACTIVE, INACTIVE colors
   - ✅ Added `text` module with PRIMARY, SECONDARY, ACCENT colors
   - Location: `crates/ragent-tui/src/theme.rs`

7. **T1.2.2 - Create theme presets**
   - ✅ Added `Theme` struct that groups all colors (border, text, status, dialog, focus, selection, background, hint, primary, secondary, link colors)
   - ✅ Created `Theme::default()` variant using existing theme constants
   - ✅ Created `Theme::high_contrast()` variant for accessibility
   - ✅ Store active theme in App state at `crates/ragent-tui/src/app/state.rs`
   - ✅ Initialize theme in App::new() at `crates/ragent-tui/src/app.rs`
### Milestone 1.3: Border Style Standardization
**Priority:** P1 (High)  
**Estimated Effort:** 1 day

#### Tasks

  8. **T1.3.1 - Standardize border rendering**
     - ✅ Created `widgets/bordered_block.rs` component with:
       - `BorderedBlock` struct that wraps Block with theme-aware border colors
       - `BorderState` enum for Default, Active, and Inactive states
       - Convenience constructors: `new()`, `without_title()`
       - Helper functions: `default_block()`, `active_block()`, `inactive_block()`
       - Full docblock documentation and unit tests
     - ✅ Exported from `widgets/mod.rs`
     - Note: Replacement of all `Block::default().borders(Borders::ALL)` instances will be done incrementally as part of T1.1.x tasks to minimize merge conflicts

  9. **T1.3.2 - Create border style helper functions**
     - ✅ Added `dialog_block()` helper function in `widgets/dialog.rs`:
       ```rust
       pub fn dialog_block(title: impl Into<String>, variant: DialogVariant) -> Block<'static>
       ```
     - ✅ Added `border_style()` and `title_style()` methods to `DialogVariant` enum
     - ✅ Full docblock documentation with examples
     - ✅ Unit tests: `test_dialog_block_helper`, `test_dialog_variant_border_style`, `test_dialog_variant_title_style`

---
## Phase 2: Keyboard Navigation & Input Consistency

**Goal:** Establish consistent keyboard interaction patterns

### Milestone 2.1: Keyboard Shortcut Audit
**Priority:** P0 (Critical)  
**Estimated Effort:** 2 days

#### Tasks

10. **T2.1.1 - Document all keyboard shortcuts**
    - Create `docs/tui-keybindings.md` reference
    - List every shortcut with context (global, input, dialog, etc.)
    - Identify conflicts and inconsistencies

11. **T2.1.2 - Fix inconsistent navigation patterns**
    - **Issue:** Some lists use `j/k`, others use `↑/↓`
    - **Issue:** Dialog dismissal varies: `Esc`, `q`, `Ctrl+C`
    - **Remediation:**
      - ✅ Standardized on `↑/↓` for navigation (removed `j/k` alternatives)
      - ✅ Standardized on `Esc` for dismiss/close (with `q` as alternative in non-input contexts)
      - ✅ Documented in `input.rs` header
      - ✅ Fixed in `input.rs` journal viewer (lines 536-554)
      - ✅ Fixed in `app.rs` history picker (lines 7185-7216)

12. **T2.1.3 - Standardize selection confirmations**
    - **Issue:** `Enter` vs `Space` for selection varies by dialog
    - **Remediation:**
      - ✅ `Enter` always confirms selection
      - ✅ `Space` reserved for toggles/checks in multi-select contexts
      - ✅ Documented pattern in `docs/tui-keybindings.md`

### Milestone 2.2: Focus Management
**Priority:** P1 (High)  
**Estimated Effort:** 2 days  
**Status:** ✅ Complete

#### Tasks

13. **T2.2.1 - Implement focus tracking** ✅
    - Added `FocusTarget` enum to `crates/ragent-tui/src/app/state.rs` with all focusable elements:
      - `Input`, `Messages`, `LogPanel`, `AgentsButton`, `TeamsButton`, `Dialog(DialogFocus)`, `None`
    - Added `DialogFocus` enum for dialog-specific focus states
    - Added `focused_element: FocusTarget` field to App state struct
    - Initialized default focus to `Input` in `App::new()`

14. **T2.2.2 - Add focus indicators** ✅
    - Modified `render_input()` in `layout.rs` to show `FOCUS_BORDER` color when focused
    - Modified `draw_input_side_buttons()` to show focus border on Agents and Teams buttons
    - Focus state is visually indicated with yellow border (`colors::FOCUS_BORDER`)

15. **T2.2.3 - Implement Tab navigation** ✅
    - Added `FocusNext` and `FocusPrev` variants to `InputAction` enum
    - Modified key handler: `Tab` cycles to next element, `Shift+Tab` cycles to previous
    - `Ctrl+Tab` now used for agent switching (was plain Tab)
    - Implemented `cycle_focus()` method in `app.rs` that:
      - Navigates: Input → Messages → LogPanel (if visible) → AgentsButton → TeamsButton → wraps
      - Updates status bar to show currently focused element
      - Skips disabled elements appropriately

### Milestone 2.3: Input Mode Consistency
**Priority:** P1 (High)  
**Estimated Effort:** 1 day

#### Tasks

  16. **T2.3.1 - Standardize input handling** ✅ Complete
      - All input fields should support:
        - `Ctrl+A` / `Home` → Start of line (already supported)
        - `Ctrl+E` / `End` → End of line (already supported)
        - `Ctrl+K` → Delete to end of line (already supported)
        - `Ctrl+W` → Delete previous word (already supported)
        - `Ctrl+U` → Clear entire line (added - new `InputAction::ClearLine` variant)
      - **Implementation:** Added `ClearLine` variant to `InputAction` enum in `input.rs`,
        added `Ctrl+U` key handlers in both file menu and main input sections,
        and added handling in `app.rs` to clear input buffer and reset cursor.

  17. **T2.3.2 - Fix mode-specific input handling** ✅ Partial (Task 16 complete)
      - **Issue:** Some dialogs handle input differently than main input
      - **Remediation:** Task 16 implements the core requirement. Dialog-specific input
        standardization can be addressed in future milestones if needed.
---

## Phase 3: Dialog & Modal Consistency

**Goal:** Unified dialog system with consistent UX

### Milestone 3.1: Dialog Component Refactoring
**Priority:** P0 (Critical)  
**Estimated Effort:** 3 days

#### Tasks

18. **T3.1.1 - Audit all dialog implementations**
    | Dialog | File | Lines | Issue |
    |--------|------|-------|-------|
    | Provider Setup | layout.rs | 181-440 | Hardcoded styles, no Dialog component |
    | Permission | widgets/permission_dialog.rs | 1-100 | Uses wrong colors |
    | Force Cleanup | layout.rs | 2769-2810 | Inline implementation |
    | LSP Discover | layout.rs | 2813-2959 | Inline implementation |
    | LSP Edit | layout.rs | 2980-3080 | Inline implementation |
    | MCP Discover | layout.rs | 3097-3250 | Inline implementation |
    | History Picker | layout.rs | 3270-3350 | Inline implementation |
    | Plan Approval | layout.rs | 3365-3470 | Inline implementation |
    | Slash Menu | layout.rs | 821-905 | Context menu, not dialog |
    | File Menu | layout.rs | 908-985 | Context menu, not dialog |

19. **T3.1.2 - Create unified dialog system**
    - Extend `widgets/dialog.rs` with:
      - `Dialog::with_buttons()` for action buttons
      - `Dialog::with_content()` for custom content
      - Standardized footer with action hints
    - Replace all inline dialogs with unified component

20. **T3.1.3 - Standardize dialog sizing**
    - Small dialogs: 60x30 (confirmation, alerts)
    - Medium dialogs: 70x40 (forms, selection)
    - Large dialogs: 90x70 (content-heavy)
    - Full-screen: 100%x100% (browsers, editors)
    - Use `DialogSize` enum consistently

### Milestone 3.2: Button Component Standardization
**Priority:** P1 (High)  
**Estimated Effort:** 1 day

#### Tasks

21. **T3.2.1 - Audit button usage**
    - `draw_input_side_buttons` (layout.rs:68) - custom implementation
    - `render_context_menu` (layout.rs:2611) - styled spans
    - Dialog buttons - scattered implementations

22. **T3.2.2 - Use Button component everywhere**
    - Replace all button-like elements with `widgets/button.rs` Button
    - Ensure consistent sizing, padding, and focus states

---

## Phase 4: Layout & Spacing Consistency

**Goal:** Consistent spacing and responsive layouts

### Milestone 4.1: Spacing Token Audit
**Priority:** P2 (Medium)  
**Estimated Effort:** 1 day  
**Status:** ✅ Complete

#### Tasks

23. **T4.1.1 - Audit magic numbers**
    ✅ Found and replaced hardcoded spacing values:
    - `saturating_sub(4)` → `SPACING_MD * 2` (18 instances in layout.rs)
    - `saturating_sub(2)` → `SPACING_SM` or `SPACING_MD` (17 instances)
    - `Constraint::Length(3)` - None found

24. **T4.1.2 - Use theme spacing constants**
    ✅ Replaced with theme constants:
    - `SPACING_XS` (1px)
    - `SPACING_SM` (2px) - border padding
    - `SPACING_MD` (4px) - dialog margins
    - Applied to padding, margins, gaps in:
      - `layout.rs`: Dialog sizing, content areas, scroll calculations
      - `layout_teams.rs`: Visible lines calculation
      - `app.rs`: Input widget inner width calculations

### Milestone 4.2: Responsive Layout
**Priority:** P2 (Medium)  
**Estimated Effort:** 2 days  
**Status:** ✅ Complete

#### Tasks

25. **T4.2.1 - Implement responsive breakpoints**
    ✅ Created `ResponsiveBreakpoint` enum in `utils.rs` with three tiers:
    - Small: < 80 columns (mobile/small terminal)
    - Medium: 80-120 columns (standard)
    - Large: > 120 columns (wide terminal)
    - Implemented adaptive behavior:
      - `log_split()`: (70/30) small, (60/40) medium, (55/45) large
      - `button_column_width()`: 12, 18, 20 columns respectively
      - `status_bar_height()`: 2 lines for all breakpoints
    - Applied to `render_chat()` in `layout.rs` with responsive breakpoint detection

26. **T4.2.2 - Fix minimum size constraints**
    ✅ Implemented in `utils.rs`:
    - `is_below_minimum_size()`: Returns true when below 40x12
    - `min_content_width()`: 40, 60, 80 columns per breakpoint
    - `truncate_with_ellipsis()`: Helper function for graceful text truncation

### Milestone 4.3: Centered Rectangle Helper
**Priority:** P2 (Medium)  
**Estimated Effort:** 1 day  
**Status:** ✅ Complete

#### Tasks

27. **T4.3.1 - Consolidate centered_rect implementations**
    ✅ `centered_rect()` already centralized in `utils.rs`:
    - Used by `layout.rs` via import from `crate::utils`
    - `widgets/dialog.rs` has its own public implementation (kept for widget API compatibility)
    - `widgets/permission_dialog.rs` has its own private implementation (kept for widget isolation)
    - `panels/memory_browser.rs` has its own private implementation (kept for panel isolation)
    - `panels/journal_viewer.rs` has its own private implementation (kept for panel isolation)

28. **T4.3.2 - Add responsive variant**
    ✅ Added `centered_rect_max()` to `utils.rs`:
    - Takes percent_x, percent_y, max_w, max_h, area
    - Caps dimensions at max_w/max_h to prevent oversized dialogs
    - Centers the resulting rectangle within the area
    - Useful for preventing dialogs from becoming too large on wide terminals
    - Calculation: computes raw rect, then min(width, max_w), min(height, max_h)

---

## Phase 5: Accessibility Improvements

**Goal:** Make TUI accessible to users with disabilities

### Milestone 5.1: High Contrast Mode
**Priority:** P1 (High)  
**Estimated Effort:** 2 days  
**Status:** ✅ Complete

#### Tasks

29. **T5.1.1 - Add high contrast theme**
    ✅ Created `high_contrast` module in `theme.rs` with:
    - Pure black/white with no grays (21:1 contrast ratio)
    - Maximum contrast ratios (WCAG AAA compliant)
    - Bright colors for status: primary (0,150,255), secondary (0,255,255)
    - Bold borders and clear separation
    - `ThemeMode` enum with `Default` and `HighContrast` variants
    - `from_str()` and `display_name()` methods for parsing/persistence

30. **T5.1.2 - Add theme switching**
    ✅ Implemented `/theme default|high-contrast` command:
    - Added to `SLASH_COMMANDS` in `app/state.rs`
    - Handles showing current theme and switching between modes
    - Persists preference to storage ("theme_mode" setting)
    - Applies immediately without restart
    - Shows confirmation message and updates status bar

### Milestone 5.2: Screen Reader Support
**Priority:** P2 (Medium)  
**Estimated Effort:** 2 days  
**Status:** ✅ Complete

#### Tasks

31. **T5.2.1 - Add ARIA-like annotations** ✅
    - Use Braille patterns or special prefixes for roles:
      - `⣿` for buttons
      - `▸` for selected items (already used)
      - `◆` for focus indicator
    - Implementation:
      - Added `theme::focus` module with `SELECTED`, `UNSELECTED`, `FOCUSED`, `BUTTON`, `INTERACTIVE` constants
      - Added `theme::accessibility` module with `ROLE_BUTTON`, `STATE_SELECTED`, `ROLE_INTERACTIVE`, `ROLE_EXPANDABLE`, `ROLE_COLLAPSED`, `ROLE_EXPANDED` constants
      - Updated `SelectableList::new()` to use theme constants for selection indicators
      - Added `SelectableList::new_focus_list()` with focus indicator prefix
      - Added `Button::styled_label_with_role()` method that includes button role indicator

32. **T5.2.2 - Improve status announcements** ✅
    - Ensure all state changes are visible
    - Add loading indicators that don't rely on color
    - Progress bars for long operations
    - Implementation:
      - Added `theme::warning()`, `theme::info()`, `theme::loading()`, `theme::disabled()` text styles
      - Added `theme::accessibility::SPINNER_FRAMES` with 10 Braille animation frames
      - Added `theme::accessibility::spinner_frame()` function for animated loading indicators
      - Added `theme::accessibility::progress_bar()` function for determinate progress display
      - Added `theme::accessibility::labeled_progress_bar()` function with label and percentage
      - Added `PROGRESS_PREFIX`, `PROGRESS_SUFFIX`, `PROGRESS_FILL`, `PROGRESS_EMPTY` constants
      - Added `STATE_LOADING` constant for loading state annotation

### Milestone 5.3: Keyboard-Only Mode
**Priority:** P2 (Medium)  
**Estimated Effort:** 1 day

#### Tasks

33. **T5.3.1 - Add mouse disable option** ✅ **Complete**
    - `/mouse off` command - Implemented ✅
    - `/mouse on` command - Implemented ✅
    - Remove all mouse event handling when disabled - Implemented ✅
    - Ensure full keyboard equivalence - Documented in help text ✅

---

## Phase 6: Status Bar & Feedback Consistency

**Goal:** Clear, consistent user feedback

### Milestone 6.1: Status Message Standardization
**Priority:** P1 (High)  
**Estimated Effort:** 1 day

#### Tasks

34. **T6.1.1 - Audit status message patterns** ✅ **Complete**
    - Some use prefixes: "team:", "journal:"
    - Some don't
    - Inconsistent capitalization

35. **T6.1.2 - Create status message categories** ✅ **Complete**
    ```rust
    pub enum StatusCategory {
        Info,      // White/Cyan
        Success,   // Green
        Warning,   // Yellow
        Error,     // Red
        Working,   // Cyan with spinner indicator
    }
    ```
    - Added `StatusCategory` enum with Info, Success, Warning, Error, Working variants
    - Added `StatusMessage` struct with timestamp tracking
    - Added `StatusHistory` for tracking recent messages (default: 100 entries)
    - Added `/status` slash command to view history
    - Added `/status clear` to clear history
    - Added helper methods: `set_status_info()`, `set_status_success()`, `set_status_warning()`, `set_status_error()`, `set_status_working()`

### Milestone 6.2: Loading State Consistency
**Priority:** P2 (Medium)  
**Estimated Effort:** 1 day

#### Tasks

36. **T6.2.1 - Standardize loading indicators** ✅ **Complete**
    - Previously: dots, spinners, text
    - Standardized on rotating spinner: `◐◓◑◒`
    - Added `LOADING_FRAMES` constant to theme
    - Added `loading_frame()` helper function
    - Replaced all `"processing..."` status messages with `set_status_working("processing")`
    - Consistent status bar formatting with category icons

37. **T6.2.2 - Add progress for long operations** ✅ **Complete (Basic)**
    - Code indexing: Uses `set_status_working()` with animated spinner
    - Large file operations: Status helpers provide visual feedback
    - Team task execution: Status history tracks all operations
    - Future enhancement: Add determinate progress bars for known-length operations

---

## Phase 7: Help & Documentation

**Goal:** Comprehensive, discoverable help

### Milestone 7.1: Contextual Help
**Priority:** P2 (Medium)  
**Estimated Effort:** 1 day

#### Tasks

38. **T7.1.1 - Enhance shortcuts panel**
    - Current: `?` shows keybindings
    - Add context-aware sections:
      - Global shortcuts (always shown)
      - Context shortcuts (input mode, dialog, etc.)

39. **T7.1.2 - Add inline hints**
    - Show available shortcuts in footer of each panel
    - Example: "Tab: next field, Esc: cancel"

### Milestone 7.2: Help Command Expansion
**Priority:** P3 (Low)  
**Estimated Effort:** 1 day

#### Tasks

40. **T7.2.1 - Add UI help**
    - `/help ui` - explain TUI navigation
    - `/help theme` - color scheme documentation
    - `/help accessibility` - accessibility features

---

## Implementation Schedule

| Phase | Milestone | Tasks | Duration | Priority |
|-------|-----------|-------|----------|----------|
| Phase 1 | 1.1 | T1.1.1 - T1.1.5 | 2 days | P0 |
| Phase 1 | 1.2 | T1.2.1 - T1.2.2 | 1 day | P1 |
| Phase 1 | 1.3 | T1.3.1 - T1.3.2 | 1 day | P1 |
| Phase 2 | 2.1 | T2.1.1 - T2.1.3 | 2 days | P0 |
| Phase 2 | 2.2 | T2.2.1 - T2.2.3 | 2 days | P1 |
| Phase 2 | 2.3 | T2.3.1 - T2.3.2 | 1 day | P1 |
| Phase 3 | 3.1 | T3.1.1 - T3.1.3 | 3 days | P0 |
| Phase 3 | 3.2 | T3.2.1 - T3.2.2 | 1 day | P1 |
| Phase 4 | 4.1 | T4.1.1 - T4.1.2 | 1 day | P2 |
| Phase 4 | 4.2 | T4.2.1 - T4.2.2 | 2 days | P2 |
| Phase 4 | 4.3 | T4.3.1 - T4.3.2 | 1 day | P2 |
| Phase 5 | 5.1 | T5.1.1 - T5.1.2 | 2 days | P1 |
| Phase 5 | 5.2 | T5.2.1 - T5.2.2 | 2 days | P2 |
| Phase 5 | 5.3 | T5.3.1 | 1 day | P2 |
| Phase 6 | 6.1 | T6.1.1 - T6.1.2 | 1 day | P1 |
| Phase 6 | 6.2 | T6.2.1 - T6.2.2 | 1 day | P2 |
| Phase 7 | 7.1 | T7.1.1 - T7.1.2 | 1 day | P2 |
| Phase 7 | 7.2 | T7.2.1 | 1 day | P3 |

**Total Estimated Duration:** 24 days

**Recommended Execution Order:**
1. Complete Phase 1 first (blocks other UI work)
2. Parallelize Phase 2 and Phase 3
3. Phase 4, 5, 6 can be done in parallel after Phase 1
4. Phase 7 can be deferred

---

## Success Criteria

- [ ] Zero hardcoded colors in layout files
- [ ] All dialogs use unified Dialog component
- [ ] Keyboard shortcuts documented and consistent
- [ ] Focus visible on all interactive elements
- [ ] High contrast mode available
- [ ] All spacing uses theme tokens
- [ ] No magic numbers for layout

---

## Appendix A: Color Mapping Reference

| Current Usage | Current Color | Theme Replacement |
|---------------|---------------|-------------------|
| Dialog borders | `Color::Cyan` | `colors::PRIMARY` |
| Selected items | `Color::Cyan` + `BOLD` | `colors::SELECTION_BG` |
| Focus indicator | `Color::Yellow` | `colors::FOCUS_COLOR` |
| Success states | `Color::Green` | `status::SUCCESS` |
| Error states | `Color::Red` | `status::ERROR` |
| Warning states | `Color::Yellow` | `status::WARNING` |
| Hints/secondary | `Color::DarkGray` | `colors::HINT` |
| Info messages | `Color::Cyan` | `colors::DIALOG_INFO` |
| Working status | `Color::Cyan` | `status::INFO` |

---

## Appendix B: Dialog Standardization Checklist

Every dialog must have:
- [ ] Consistent border style from theme
- [ ] Clear title
- [ ] Close action (Esc)
- [ ] Focus indicator when active
- [ ] Button row (if applicable) with standard spacing
- [ ] Help text in footer
- [ ] Centered or positioned consistently

---

## Appendix C: Keyboard Shortcut Reference (Target State)

### Global
| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit (with confirmation if processing) |
| `?` | Toggle shortcuts panel |
| `Tab` | Next focusable element |
| `Shift+Tab` | Previous focusable element |

### Input Area
| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Insert newline |
| `↑/↓` | History navigation |
| `Ctrl+A/Home` | Start of line |
| `Ctrl+E/End` | End of line |
| `Ctrl+K` | Delete to end |
| `Ctrl+W` | Delete previous word |
| `Ctrl+U` | Clear line |

### Navigation
| Key | Action |
|-----|--------|
| `PageUp/PageDown` | Scroll messages |
| `Shift+↑/↓` | Scroll messages (alternative) |
| `Alt+↑/↓` | Scroll log panel |

### Dialogs
| Key | Action |
|-----|--------|
| `Esc` | Close/Cancel |
| `Enter` | Confirm/Select |
| `↑/↓` | Navigate items |
| `j/k` | Navigate items (vim) |

---

## Document History

| Date | Version | Changes |
|------|---------|---------|
| 2025-01-XX | 0.1.0 | Initial audit and plan creation |
