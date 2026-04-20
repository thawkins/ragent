# Status Bar Implementation Guide

**Document:** STATUSBAR_IMPLEMENTATION_GUIDE.md  
**Version:** 1.0  
**Status:** Ready for Implementation  
**Created:** 2025-01-16  
**Phases to Implement:** 2-6 (7 weeks, 27 tasks)

---

## Executive Summary

This guide provides a detailed implementation roadmap for the status bar redesign across all phases. It's designed to be executable by developers with clear steps, file locations, and testing requirements.

---

## Phase 2: Core Layout Engine (Weeks 2-3, 10 days)

### Task 2.1: Create New Layout Structure

**Objective:** Define data structures to represent the new 3-section layout

**Location:** Create new file `crates/ragent-tui/src/layout_statusbar.rs`

**Steps:**

1. **Define Enums and Structs**

```rust
/// Represents one section of the status bar (left, center, or right)
#[derive(Debug, Clone)]
pub struct StatusBarSection {
    /// Section position (left, center, right)
    pub position: SectionPosition,
    /// Width in characters
    pub width: u16,
    /// Content as formatted spans
    pub content: Vec<Span<'static>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SectionPosition {
    Left,
    Center,
    Right,
}

/// Represents one line of the status bar
#[derive(Debug, Clone)]
pub struct StatusBarLine {
    pub left: StatusBarSection,
    pub center: StatusBarSection,
    pub right: StatusBarSection,
}

/// Complete status bar (2 lines)
#[derive(Debug, Clone)]
pub struct StatusBar {
    pub line1: StatusBarLine,    // Context & Status
    pub line2: StatusBarLine,    // System & Resources
}

/// Configuration for responsive layout
#[derive(Debug, Clone, Copy)]
pub struct StatusBarConfig {
    pub terminal_width: u16,
    pub breakpoint: ResponsiveBreakpoint,
    pub max_left_width: u16,
    pub max_center_width: u16,
    pub show_detailed_metrics: bool,
}
```

2. **Define Builder Functions**

```rust
impl StatusBar {
    /// Create a new status bar with all sections empty
    pub fn new() -> Self {
        // Implementation
    }

    /// Build status bar from app state
    pub fn from_app(app: &App, config: StatusBarConfig) -> Self {
        // Implementation
    }

    /// Render status bar to ratatui Line objects
    pub fn render(&self) -> (Line, Line) {
        // Implementation
    }
}
```

**Testing:**
- Create unit tests for structure creation
- Test with various terminal widths

**Files to Modify:**
- `crates/ragent-tui/src/lib.rs` — Add module declaration
- `crates/ragent-tui/src/layout_statusbar.rs` — New file (create above)

---

### Task 2.2: Implement Line 1 Rendering (Context & Status)

**Objective:** Render left, center, right sections for Line 1

**Location:** `crates/ragent-tui/src/layout_statusbar.rs` (in new file from Task 2.1)

**Components:**

**Left Section: Working Directory (25 chars max)**

```rust
fn build_line1_left(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Get current directory from app.cwd
    // 2. Replace /home/username with ~ if applicable
    // 3. Truncate to 25 chars (shorten_middle if needed)
    // 4. Add read-only indicator if needed
    // 5. Pad right to 25 chars
    // Return Vec<Span> with styled text
}
```

**Center Section: Git Branch & Status (15 chars max)**

```rust
fn build_line1_center(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Get git branch from app.git_branch
    // 2. Determine git status (clean, changes, conflict, etc.)
    // 3. Return branch + status indicator (●, ◦, ✗, ↑, ↓)
    // 4. Apply color based on status
    // 5. Pad to 15 chars max
}
```

**Right Section: Session Status (flexible)**

```rust
fn build_line1_right(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Get status message from app.status
    // 2. If processing, get operation name and show ⟳ spinner
    // 3. Return right-aligned status message
    // 4. Apply color: Green (ready), Yellow (processing), Red (error)
}
```

**Steps:**

1. Implement `build_line1_left()` function
2. Implement `build_line1_center()` function
3. Implement `build_line1_right()` function
4. Implement `build_line1()` function that combines all three sections
5. Test each section independently
6. Test combined rendering

**Testing:**
- Test with various directory paths
- Test git status variants
- Test status messages
- Test responsive truncation

**Files to Modify:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Add functions
- `crates/ragent-tui/src/layout.rs` — Call new functions (not yet, in Task 2.4)

---

### Task 2.3: Implement Line 2 Rendering (System & Resources)

**Objective:** Render left, center, right sections for Line 2

**Location:** `crates/ragent-tui/src/layout_statusbar.rs` (continued)

**Components:**

**Left Section: LLM Provider (25 chars max)**

```rust
fn build_line2_left(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Get provider health: ● (green), ◔ (yellow), ✗ (red)
    // 2. Get provider name: "claude", "gpt-4o", "gemini", etc.
    // 3. Get context window: "4K", "128K", "200K"
    // 4. Format: "● claude │ 200K"
    // 5. Pad right to 25 chars
}
```

**Center Section: Resources & Metrics (25 chars max)**

```rust
fn build_line2_center(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Get token usage: current / max
    // 2. Calculate percentage
    // 3. Build progress bar (████░░░░░░) 10 chars
    // 4. Color bar: Green (<50%), Yellow (50-80%), Red (>80%)
    // 5. Format: "tokens: 45% ████░░░░░░"
    // 6. Optional: Show active task count if > 0
    // 7. Pad to 25 chars max
}
```

**Right Section: Service Status (flexible)**

```rust
fn build_line2_right(app: &App, config: StatusBarConfig) -> Vec<Span<'static>> {
    // 1. Build service indicators:
    //    - LSP:✓ (green if connected, red if error)
    //    - CodeIdx:✓ (green if ready, yellow if indexing)
    //    - AIWiki:✓ (green if enabled, red if disabled)
    // 2. Optional: Show Log count if verbose mode
    // 3. Right-align all content
    // 4. Return with appropriate colors
}
```

**Steps:**

1. Implement `build_line2_left()` function
2. Implement `build_line2_center()` function (with progress bar logic)
3. Implement `build_line2_right()` function
4. Implement `build_line2()` function that combines all three sections
5. Test each section independently
6. Test progress bar rendering at various percentages
7. Test service status indicators

**Testing:**
- Test with different providers
- Test token usage at 0%, 45%, 80%, 100%
- Test service status combinations
- Test progress bar colors

**Files to Modify:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Add functions

---

### Task 2.4: Integration with Main Layout

**Objective:** Integrate new status bar rendering into main render function

**Location:** `crates/ragent-tui/src/layout.rs`

**Steps:**

1. Create new `render_status_bar_v2()` function that:
   - Calls new layout_statusbar functions
   - Returns two rendered lines
   - Keeps old `render_status_bar()` as fallback

2. Update `render_chat()` to:
   - Detect if redesign is enabled
   - Call either new or old render function

3. Test integration without breaking existing functionality

**Code Pattern:**

```rust
pub fn render_status_bar_v2(frame: &mut Frame, app: &mut App, area: Rect) {
    let config = StatusBarConfig {
        terminal_width: area.width,
        breakpoint: ResponsiveBreakpoint::from_width(area.width),
        max_left_width: 25,
        max_center_width: 15,
        show_detailed_metrics: false,
    };

    let status_bar = StatusBar::from_app(app, config);
    let (line1, line2) = status_bar.render();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    frame.render_widget(Paragraph::new(line1), rows[0]);
    frame.render_widget(Paragraph::new(line2), rows[1]);
}
```

**Testing:**
- Test rendering at full width
- Test at various terminal widths
- Verify no breaking changes to existing functionality

**Files to Modify:**
- `crates/ragent-tui/src/layout.rs` — Add v2 function, update render_chat()

---

## Phase 3: Visual Polish & Indicators (Week 4, 5 days)

### Task 3.1: Implement Status Indicators

**Objective:** Add color support and visual indicators

**Location:** `crates/ragent-tui/src/layout_statusbar.rs` (continued)

**Components:**

1. **Status Dot Colors**

```rust
fn get_status_color(status: StatusType) -> Color {
    match status {
        StatusType::Healthy => Color::Green,
        StatusType::Warning => Color::Yellow,
        StatusType::Error => Color::Red,
        StatusType::Info => Color::Cyan,
    }
}
```

2. **Indicator Symbols**

```rust
const INDICATOR_HEALTHY: &str = "●";
const INDICATOR_WARNING: &str = "◔";
const INDICATOR_ERROR: &str = "✗";
const INDICATOR_CHECK: &str = "✓";
const INDICATOR_UP: &str = "↑";
const INDICATOR_DOWN: &str = "↓";
const INDICATOR_SPINNER: &str = "⟳";
```

3. **Progress Bar**

```rust
fn build_progress_bar(percentage: u8) -> String {
    let filled = (percentage / 10).min(10) as usize;
    let empty = 10 - filled;
    format!("{}{}",
        "█".repeat(filled),
        "░".repeat(empty)
    )
}

fn get_progress_color(percentage: u8) -> Color {
    match percentage {
        0..=50 => Color::Green,
        51..=80 => Color::Yellow,
        _ => Color::Red,
    }
}
```

**Steps:**

1. Create color mapping functions
2. Implement progress bar rendering
3. Create indicator symbol constants
4. Update all section builders to use colors and indicators
5. Test colors on different terminal types

**Testing:**
- Test color rendering on 16-color, 256-color, true color
- Test all status types (healthy, warning, error, info)
- Test progress bar at various percentages

**Files to Modify:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Add color/indicator functions
- `crates/ragent-tui/src/theme.rs` — Update theme if needed (optional)

---

### Task 3.2: Implement Color Coding

**Objective:** Apply semantic color palette throughout

**Location:** `crates/ragent-tui/src/theme.rs` (optional, or inline in layout_statusbar.rs)

**Color Palette:**

```rust
pub const COLOR_HEALTHY: Color = Color::Green;    // #00FF00
pub const COLOR_WARNING: Color = Color::Yellow;    // #FFFF00
pub const COLOR_ERROR: Color = Color::Red;         // #FF0000
pub const COLOR_INFO: Color = Color::Cyan;         // #00FFFF
pub const COLOR_TEXT: Color = Color::White;        // #FFFFFF
pub const COLOR_LABEL: Color = Color::DarkGray;    // #808080
pub const COLOR_BG: Color = Color::Black;          // #000000
```

**Steps:**

1. Define color constants in theme.rs (or layout_statusbar.rs)
2. Create functions for semantic styling:
   - `style_healthy(text) -> Style`
   - `style_warning(text) -> Style`
   - `style_error(text) -> Style`
   - `style_info(text) -> Style`
3. Update all section builders to use styling functions
4. Test color consistency across all sections
5. Test on light and dark terminal backgrounds

**Testing:**
- Test color rendering on various terminal themes
- Test contrast/readability
- Test colorblind accessibility (optional colorblind mode)

**Files to Modify:**
- `crates/ragent-tui/src/theme.rs` — Add color constants
- `crates/ragent-tui/src/layout_statusbar.rs` — Use color constants

---

## Phase 4: Responsive & Adaptive Behavior (Week 5, 5 days)

### Task 4.1: Responsive Breakpoints

**Objective:** Support different terminal widths gracefully

**Location:** `crates/ragent-tui/src/layout_statusbar.rs` (continued)

**Responsive Modes:**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponsiveMode {
    /// Full layout: 120+ characters
    Full,
    /// Compact layout: 80-120 characters
    Compact,
    /// Minimal layout: <80 characters
    Minimal,
}

impl ResponsiveMode {
    pub fn from_width(width: u16) -> Self {
        match width {
            120.. => Self::Full,
            80..120 => Self::Compact,
            _ => Self::Minimal,
        }
    }
}
```

**Steps:**

1. Define `ResponsiveMode` enum
2. Implement mode detection from terminal width
3. Create variant functions:
   - `build_line1_full()`, `build_line1_compact()`, `build_line1_minimal()`
   - `build_line2_full()`, `build_line2_compact()`, `build_line2_minimal()`
4. Update section builders to accept mode parameter
5. Test graceful degradation at each breakpoint

**Width Calculations:**

- **Full (120+):**
  - Left: 25 chars
  - Center: 15 chars
  - Right: remaining
  
- **Compact (80-120):**
  - Left: 20 chars
  - Center: 12 chars
  - Right: remaining
  
- **Minimal (<80):**
  - Left: 15 chars
  - Center: 10 chars
  - Right: remaining

**Testing:**
- Test at 120, 100, 80, 60 character widths
- Verify no information truncation (only abbreviation)
- Test path abbreviations
- Test metric abbreviations

**Files to Modify:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Add responsive functions

---

### Task 4.2: Dynamic Information Hiding

**Objective:** Show/hide info based on available space

**Location:** `crates/ragent-tui/src/layout_statusbar.rs` (continued)

**Strategies:**

1. **On Minimal layouts:**
   - Hide detailed metrics
   - Show only critical info
   - Defer to `/status` command for full info

2. **Optional verbose mode:**
   - Add `app.verbose_status` flag
   - Show all metrics when enabled

3. **Smart abbreviations:**
   - "tokens:" → "tok:" on compact
   - "tasks:" → "t:" on minimal
   - "LSP:✓" stays same (compact already)

**Steps:**

1. Update section builders to check available width
2. Implement abbreviation logic
3. Add verbose mode detection
4. Test information preservation (nothing lost, just deferred)

**Testing:**
- Verify info is not lost, only hidden
- Test `/status` command shows full info
- Test verbose mode toggle

**Files to Modify:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Add hiding logic
- `crates/ragent-tui/src/app.rs` — Add verbose_status flag

---

## Phase 5: Testing & Validation (Week 6, 5 days)

### Task 5.1: Unit & Integration Tests

**Objective:** Comprehensive test coverage

**Location:** `crates/ragent-tui/tests/test_statusbar_layout.rs` (new file)

**Test Suite 1: Layout Structure Tests**

```rust
#[test]
fn test_statusbar_creation() { /* ... */ }

#[test]
fn test_line1_sections_present() { /* ... */ }

#[test]
fn test_line2_sections_present() { /* ... */ }

#[test]
fn test_section_width_constraints() { /* ... */ }
```

**Test Suite 2: Content Rendering Tests**

```rust
#[test]
fn test_working_dir_formatting() { /* ... */ }

#[test]
fn test_git_branch_display() { /* ... */ }

#[test]
fn test_token_percentage_calculation() { /* ... */ }

#[test]
fn test_progress_bar_rendering() { /* ... */ }

#[test]
fn test_service_indicators() { /* ... */ }
```

**Test Suite 3: Color Tests**

```rust
#[test]
fn test_healthy_status_color() { /* ... */ }

#[test]
fn test_warning_status_color() { /* ... */ }

#[test]
fn test_error_status_color() { /* ... */ }

#[test]
fn test_progress_bar_colors() { /* ... */ }
```

**Steps:**

1. Create test file with test structures
2. Implement layout structure tests
3. Implement content rendering tests
4. Implement color tests
5. Run all tests, achieve 100% coverage of layout_statusbar.rs

**Testing Commands:**

```bash
cargo test --package ragent-tui --lib layout_statusbar --
cargo test --package ragent-tui test_statusbar_layout --
```

**Files to Create:**
- `crates/ragent-tui/tests/test_statusbar_layout.rs` — New test file

---

### Task 5.2: Visual Testing & Feedback

**Objective:** Manual validation on real terminals

**Steps:**

1. **Set up test terminals:**
   - 80x24 (minimal)
   - 120x40 (compact)
   - 180x50 (full)

2. **Test scenarios:**
   - Healthy idle system
   - Active processing
   - Error state
   - High token usage
   - Various git states

3. **Validation checklist:**
   - [ ] Layout is clean and professional
   - [ ] Text is readable at all widths
   - [ ] Colors are appropriate
   - [ ] No information truncation (only abbreviation)
   - [ ] Animations are smooth
   - [ ] Terminal compatibility (xterm, iTerm, Windows Terminal, etc.)

4. **Document results:**
   - Take screenshots
   - Note any issues or unexpected behavior
   - Record feedback

**Files to Modify:**
- `STATUSBAR_MOCKUPS.md` — Update with actual screenshots (optional)

---

### Task 5.3: Performance Testing

**Objective:** Ensure no rendering performance regressions

**Steps:**

1. Create benchmark file: `crates/ragent-tui/benches/bench_statusbar.rs`

2. Benchmark scenarios:
   - Status bar rendering time (target: <5ms)
   - Memory allocation (target: <1KB per render)
   - Color calculation (target: <1ms)

3. Compare against baseline (old implementation)

4. Validate no regressions

**Benchmark Code:**

```rust
#[bench]
fn bench_render_status_bar(b: &mut Bencher) {
    let app = create_test_app();
    let config = StatusBarConfig::default();

    b.iter(|| {
        StatusBar::from_app(&app, config)
    });
}
```

**Testing Command:**

```bash
cargo bench --package ragent-tui statusbar
```

**Files to Create:**
- `crates/ragent-tui/benches/bench_statusbar.rs` — Benchmark file (optional)

---

## Phase 6: Documentation & Release (Week 7, 3 days)

### Task 6.1: Update Specification

**Objective:** Document new status bar in SPEC.md

**Location:** `SPEC.md` Section 4.1.1 (Main Screen)

**Changes:**

1. Update "Status Bar" subsection:
   - Replace old description with new layout
   - Add ASCII art mockups
   - Document responsive breakpoints
   - Document color meanings
   - Document indicators

2. Add subsection "Status Bar Components":
   - Line 1: Context & Status
   - Line 2: System & Resources
   - Column specifications

3. Add subsection "Responsive Behavior":
   - Full layout (120+)
   - Compact layout (80-120)
   - Minimal layout (<80)

4. Add subsection "Status Indicators":
   - Color meanings
   - Symbol meanings
   - Git status variants

**Files to Modify:**
- `SPEC.md` — Update Section 4.1.1

---

### Task 6.2: User Documentation

**Objective:** Create user guide for new status bar

**Location:** Create new file `docs/tui-statusbar-guide.md`

**Contents:**

1. **Quick Start:**
   - What the status bar shows
   - How to interpret indicators

2. **Line 1: Context & Status**
   - Working directory display
   - Git branch and status
   - Status messages

3. **Line 2: System & Resources**
   - Provider health and model
   - Token usage
   - Service indicators

4. **Indicators Reference:**
   - Color meanings
   - Symbol meanings
   - Status examples

5. **Terminal Compatibility:**
   - Supported terminal types
   - Color mode support
   - Accessibility modes

6. **Troubleshooting:**
   - Common questions
   - How to get full status info

**Files to Create:**
- `docs/tui-statusbar-guide.md` — New user guide

---

### Task 6.3: Changelog & Release Notes

**Objective:** Document changes for release

**Steps:**

1. Update `CHANGELOG.md`:
   - Add entry under "Unreleased" section
   - Document all status bar improvements
   - Link to related documentation

2. Update `RELEASE.md`:
   - Add version number (0.1.0-alpha.45 or higher)
   - Add status bar improvements to highlights

3. Create release commit:
   - Message: "feat: Redesign status bar with 3-section layout"
   - Include all documentation updates

**Changelog Entry Example:**

```markdown
## [0.1.0-alpha.45] - 2025-02-07

### Changed
- **Status Bar Redesign:** Complete visual overhaul with 3-section layout per line
  - Line 1: Working directory, Git branch, Session status (clean, professional)
  - Line 2: LLM provider, Token usage (with progress bar), Service indicators
  - Semantic color coding (Green/Yellow/Red/Cyan) for quick status understanding
  - Responsive layouts for terminal widths: full (120+), compact (80-120), minimal (<80)
  - Graceful degradation on small terminals with no information loss
  - [See STATUSBAR_PLAN.md for detailed specifications]
  - [See docs/tui-statusbar-guide.md for user guide]

### Added
- New `layout_statusbar.rs` module for dedicated status bar rendering
- `StatusBar`, `StatusBarLine`, `StatusBarSection` data structures
- `ResponsiveMode` enum for responsive layout support
- Comprehensive status bar tests in `test_statusbar_layout.rs`

### Fixed
- Visual clutter in status bar (information organization)
- Inconsistent separator usage
- Flat visual hierarchy
```

**Files to Modify:**
- `CHANGELOG.md` — Add release notes
- `RELEASE.md` — Add version highlights

---

## Implementation Dependencies & Order

```
Phase 2 (Core Layout) - Must complete first
  ├─ Task 2.1: Create structures (Required by 2.2, 2.3)
  ├─ Task 2.2: Line 1 rendering (Parallel with 2.3)
  ├─ Task 2.3: Line 2 rendering (Parallel with 2.2)
  └─ Task 2.4: Integration (Requires 2.1, 2.2, 2.3)

Phase 3 (Visual Polish) - Depends on Phase 2
  ├─ Task 3.1: Status indicators (Parallel with 3.2)
  └─ Task 3.2: Color coding (Parallel with 3.1)

Phase 4 (Responsive) - Depends on Phase 2
  ├─ Task 4.1: Responsive breakpoints (Required by 4.2)
  └─ Task 4.2: Dynamic hiding (Requires 4.1)

Phase 5 (Testing) - Depends on Phase 2, 3, 4
  ├─ Task 5.1: Unit tests (Can parallel with 5.2)
  ├─ Task 5.2: Visual tests (Parallel with 5.1)
  └─ Task 5.3: Performance (Parallel with 5.1, 5.2)

Phase 6 (Release) - Depends on Phase 2, 3, 4, 5
  ├─ Task 6.1: Update spec (After Phase 5 complete)
  ├─ Task 6.2: User guide (After Phase 5 complete)
  └─ Task 6.3: Release (After 6.1, 6.2 complete)
```

---

## Success Metrics

✅ **Code Quality:**
- All new code has 100% test coverage
- No compiler warnings
- Code reviews passed
- Performance benchmarks show <5ms per render

✅ **Visual Quality:**
- Professional appearance, no visual glitches
- Readable on all terminal sizes
- Colors render correctly on various terminal types
- Responsive layouts work smoothly

✅ **Documentation:**
- SPEC.md updated with new layout
- User guide created (docs/tui-statusbar-guide.md)
- Code is well-documented with doc comments
- CHANGELOG updated with changes

✅ **Functionality:**
- No breaking changes to existing features
- All information preserved (responsive degradation only)
- Status bar renders correctly on all tested terminals
- Indicators work correctly in all states

---

## Verification Checklist

Before considering the implementation complete, verify:

**Code:**
- [ ] New `layout_statusbar.rs` module created and integrated
- [ ] All Phase 2-5 functions implemented
- [ ] All tests passing (`cargo test --all`)
- [ ] No compiler warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

**Visual:**
- [ ] Renders correctly at 80, 120, 180 character widths
- [ ] Colors display on 16-color, 256-color, true color terminals
- [ ] No information truncation (only abbreviation on small terminals)
- [ ] Professional appearance

**Documentation:**
- [ ] SPEC.md Section 4.1.1 updated
- [ ] User guide created (docs/tui-statusbar-guide.md)
- [ ] CHANGELOG.md updated
- [ ] Code is well-commented

**Release:**
- [ ] Version number updated
- [ ] RELEASE.md updated
- [ ] Changelog entry complete
- [ ] Ready for git push

---

## Rollback Plan

If critical issues are found:

1. Keep old `render_status_bar()` function as fallback
2. Add feature flag: `old_statusbar` for reverting to old layout
3. Users can use environment variable or config option to toggle

```toml
[dependencies]
# In Cargo.toml
ragent-tui = { path = "crates/ragent-tui", features = ["old_statusbar"] }
```

---

**End of Implementation Guide**

