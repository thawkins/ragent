# UI Remediation Plan for ragent TUI

**Project:** ragent Terminal User Interface  
**Crate:** `crates/ragent-tui`  
**Generated:** From swarm audit reports (visual, components, layout, accessibility)  
**Status:** Ready for implementation

---

## 1. Executive Summary

### Top Issues by Impact

| Rank | Issue | Impact | Affected Users | Effort |
|------|-------|--------|----------------|--------|
| 1 | **No Centralized Theme System** | Inconsistent visual experience, maintenance burden | All users | Medium |
| 2 | **Hardcoded Colors (318+ occurrences)** | No dark/light mode support, accessibility issues | Vision-impaired users | High |
| 3 | **Mixed Icon Types (emoji + Unicode)** | Rendering inconsistencies across terminals | All users | Medium |
| 4 | **Inconsistent Focus Indicators** | Navigation confusion, accessibility barriers | Keyboard users | Medium |
| 5 | **Color Contrast Failures** | WCAG AA non-compliance (DarkGray on Black = 2.9:1) | Low vision users | Medium |
| 6 | **Duplicate Layout Helpers** | Code duplication (`centered_rect` defined twice) | Developers | Low |
| 7 | **Inconsistent Dialog Patterns** | User confusion, inconsistent UX | All users | High |
| 8 | **Magic Numbers for Spacing** | No design system, inconsistent layouts | All users | Medium |

### Severity Distribution

| Audit Area | Critical | High | Medium | Low | Total |
|------------|----------|------|--------|-----|-------|
| Visual Design | 2 | 16 | 13 | - | 31 |
| Components | - | 8 | 12 | 4 | 24 |
| Layout | - | - | 5 | - | 5 |
| Accessibility | - | 3 | 5 | 4 | 12 |
| **TOTAL** | **2** | **27** | **35** | **8** | **72** |

---

## 2. Categorized Findings Summary

### 2.1 Visual Design System Issues

**Color Palette Chaos**
- 318+ occurrences of `.fg()` and `.bg()` with hardcoded colors
- No semantic color mapping (e.g., `Color::Blue` used for both buttons AND links)
- Top offenders: `Color::DarkGray` (38x), `Color::White` (27x), `Color::Cyan` (24x)

**Typography Inconsistencies**
- 40+ occurrences of `Modifier::BOLD` without semantic hierarchy
- No defined text scale (heading/body/caption)
- Italics used only 2 times (for reasoning text)

**Spacing Token Absence**
- No consistent spacing scale (4px/8px/16px/24px)
- Magic numbers: `Length(2)`, `Length(4)`, `Min(3)`, `Percentage(60)`
- Border calculations done inline (`+2` / `-2`)

**Iconography Fragmentation**
- Mix of emoji icons (💭📄📝) and Unicode symbols (●✓✗)
- No terminal compatibility fallbacks
- Status icons use same symbol with only color differences

### 2.2 Component Pattern Issues

**Button Inconsistencies**
- Agents/Teams toggle buttons have duplicate logic
- No standardized primary/secondary/danger button hierarchy
- Different selection indicators: `▸` vs background change

**Input Field Variations**
- Three different input patterns across dialogs:
  - Provider Setup: `> ` prefix with cyan color
  - Question Dialog: `▶ ` prefix with green color
  - LSP/MCP: `with_cursor_marker` helper

**Dialog Fragmentation**
- 9 different dialog implementations with inconsistent:
  - Border colors: Cyan, Yellow, Magenta
  - Title styles: plain, with slashes, with emoji
  - Sizing: percentage-based, fixed rows, dynamic

**Status Indicator Confusion**
- Same icon (`●`) used for different states (green=healthy, yellow=unknown)
- Team status colors differ from Active Agent colors
- No legend or tooltip for status meanings

### 2.3 Layout System Issues

**Hardcoded Dimensions**
- Magic number `88` used inconsistently for max-width
- Dialog sizes vary: 60x30, 60x50, 90x56, 58x56, 90x70
- No standard dialog size presets

**Code Duplication**
- `centered_rect()` defined identically in 2 locations:
  - `layout.rs:3225-3242`
  - `widgets/permission_dialog.rs:39-57`

**Inconsistent Constraint Values**
- Border padding assumed as 2, but not standardized
- Width calculations: `82`, `72`, `88`, `50`, `60` (no consistency)

### 2.4 Accessibility Issues

**Focus Indicator Inconsistency**
| Component | Selected | Unselected |
|-----------|----------|------------|
| Provider List | `▸ ` | `  ` |
| LSP/MCP | `▸ ` | `  ` |
| Team Teammates | `▸` | ` ` |
| Active Agents | `● ` / `◦ ` | varies |

**Color Contrast Failures**
- `Color::DarkGray` on Black = 2.9:1 (WCAG AA requires 4.5:1)
- `Color::LightBlue` on Black = 4.5:1 (borderline)
- Status icons rely on color alone (colorblind accessibility issue)

**Keyboard Navigation Gaps**
- No visible keyboard hints in dialogs
- Tab navigation not consistently supported
- Alt+↑/↓ may conflict with terminal shortcuts

---

## 3. Milestones

### Phase 1: Critical Fixes (Week 1-2)
**Goal:** Address accessibility barriers and critical visual inconsistencies

- [ ] **1.1** Create theme module foundation
- [ ] **1.2** Fix color contrast issues (DarkGray → accessible gray)
- [ ] **1.3** Standardize focus indicators
- [ ] **1.4** Add keyboard navigation hints to dialogs

### Phase 2: Design System Implementation (Week 3-4)
**Goal:** Establish centralized theming and spacing system

- [ ] **2.1** Complete theme module with color palette
- [ ] **2.2** Create typography system with semantic roles
- [ ] **2.3** Define spacing token constants
- [ ] **2.4** Extract `centered_rect` to shared utility

### Phase 3: Component Standardization (Week 5-6)
**Goal:** Refactor duplicate components into reusable abstractions

- [ ] **3.1** Create `Dialog` component with variants
- [ ] **3.2** Create `Button` component with states
- [ ] **3.3** Create `InputField` unified component
- [ ] **3.4** Create `SelectableList` component
- [ ] **3.5** Merge LSP/MCP discover dialogs

### Phase 4: Polish (Week 7-8)
**Goal:** Consistency pass and documentation

- [ ] **4.1** Standardize icon set (emoji vs Unicode decision)
- [ ] **4.2** Unify dialog border colors and title styles
- [ ] **4.3** Create status icon legend
- [ ] **4.4** Documentation and style guide

---

## 4. Detailed Task Breakdown

### Phase 1: Critical Fixes

#### Task 1.1: Create Theme Module Foundation
**Files to modify:**
- Create: `crates/ragent-tui/src/theme.rs` (new file)
- Modify: `crates/ragent-tui/src/lib.rs` (add module export)

**Implementation approach:**
```rust
// theme.rs - Phase 1 MVP
pub mod colors {
    use ratatui::style::Color;
    
    // Accessible grays
    pub const HINT: Color = Color::Rgb(170, 170, 170);
    pub const DISABLED: Color = Color::Rgb(140, 140, 140);
    
    // Semantic status colors
    pub const SUCCESS: Color = Color::Green;
    pub const ERROR: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;
    pub const INFO: Color = Color::Cyan;
}

pub mod focus {
    pub const SELECTED: &str = "▸ ";
    pub const UNSELECTED: &str = "  ";
}
```

**Estimated effort:** 2-3 hours  
**Dependencies:** None  
**Parallelizable with:** Task 1.2, 1.3, 1.4

---

#### Task 1.2: Fix Color Contrast Issues
**Files to modify:**
- `crates/ragent-tui/src/layout.rs` (lines 163, 416-418)
- `crates/ragent-tui/src/layout_teams.rs` (line 127)
- `crates/ragent-tui/src/widgets/message_widget.rs` (search for `Color::DarkGray`)

**Implementation approach:**
1. Replace `Color::DarkGray` with `theme::colors::HINT`
2. Update selection highlight: `LightBlue` → `Rgb(0, 100, 200)` with `White` fg
3. Update health status colors to use theme constants

**Estimated effort:** 4-6 hours (across 5 files)  
**Dependencies:** Task 1.1  
**Parallelizable with:** Task 1.3, 1.4

---

#### Task 1.3: Standardize Focus Indicators
**Files to modify:**
- `crates/ragent-tui/src/layout.rs` (provider/model/agent lists)
- `crates/ragent-tui/src/layout_teams.rs` (teammate list)
- `crates/ragent-tui/src/layout_active_agents.rs` (agent list)

**Implementation approach:**
```rust
// Use theme::focus constants
let prefix = if selected { theme::focus::SELECTED } else { theme::focus::UNSELECTED };
```

**Estimated effort:** 2-3 hours  
**Dependencies:** Task 1.1  
**Parallelizable with:** Task 1.2, 1.4

---

#### Task 1.4: Add Keyboard Navigation Hints
**Files to modify:**
- `crates/ragent-tui/src/layout.rs` (dialogs: provider setup, LSP, MCP, plan approval)

**Implementation approach:**
Add footer line to each modal dialog:
```rust
lines.push(Line::from(Span::styled(
    "Tab: switch field  Enter: confirm  Esc: cancel",
    Style::default().fg(theme::colors::HINT),
)));
```

**Estimated effort:** 2-3 hours  
**Dependencies:** Task 1.1  
**Parallelizable with:** Task 1.2, 1.3

---

### Phase 2: Design System Implementation

#### Task 2.1: Complete Theme Module
**Files to modify:**
- `crates/ragent-tui/src/theme.rs` (expand)

**Implementation approach:**
```rust
pub mod colors {
    // Full semantic palette
    pub const PRIMARY: Color = Color::Blue;
    pub const SECONDARY: Color = Color::Cyan;
    pub const SUCCESS: Color = Color::Green;
    pub const WARNING: Color = Color::Yellow;
    pub const ERROR: Color = Color::Red;
    pub const MUTED: Color = Color::Rgb(170, 170, 170);
    pub const TEXT: Color = Color::White;
    pub const BACKGROUND: Color = Color::Black;
    
    // Component-specific
    pub mod dialog {
        pub const INFO: Color = Color::Cyan;
        pub const WARNING: Color = Color::Yellow;
        pub const DANGER: Color = Color::Red;
        pub const SUCCESS: Color = Color::Green;
    }
}

pub mod typography {
    use ratatui::style::{Style, Modifier};
    
    pub fn heading() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
    
    pub fn emphasis() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
    
    pub fn muted() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }
}
```

**Estimated effort:** 3-4 hours  
**Dependencies:** Phase 1 complete  
**Parallelizable with:** Task 2.2, 2.3, 2.4

---

#### Task 2.2: Create Typography System
**Files to modify:**
- `crates/ragent-tui/src/theme.rs` (add typography module)
- `crates/ragent-tui/src/layout.rs` (update all text styles)
- `crates/ragent-tui/src/layout_teams.rs`
- `crates/ragent-tui/src/widgets/message_widget.rs`

**Implementation approach:**
Replace inline style creation with semantic helpers:
```rust
// Before
Style::default().fg(Color::White).add_modifier(Modifier::BOLD)

// After
theme::typography::heading().fg(theme::colors::TEXT)
```

**Estimated effort:** 6-8 hours  
**Dependencies:** Task 2.1  
**Parallelizable with:** Task 2.3, 2.4

---

#### Task 2.3: Define Spacing Tokens
**Files to modify:**
- `crates/ragent-tui/src/theme.rs` (add spacing module)
- All layout files (replace magic numbers)

**Implementation approach:**
```rust
pub mod spacing {
    use ratatui::layout::Constraint;
    
    pub const XS: u16 = 1;
    pub const SM: u16 = 2;
    pub const MD: u16 = 4;
    pub const LG: u16 = 8;
    pub const XL: u16 = 16;
    
    pub fn length(n: u16) -> Constraint {
        Constraint::Length(n)
    }
}

pub mod layout {
    pub const MAX_CONTENT_WIDTH: u16 = 88;
    pub const BORDER_PADDING: u16 = 2;
}

pub mod dialog_sizes {
    pub const SMALL: (u16, u16) = (60, 30);
    pub const MEDIUM: (u16, u16) = (70, 40);
    pub const LARGE: (u16, u16) = (90, 70);
}
```

**Estimated effort:** 8-10 hours (across all files)  
**Dependencies:** Task 2.1  
**Parallelizable with:** Task 2.2, 2.4

---

#### Task 2.4: Extract Centered Rect Utility
**Files to modify:**
- Create: `crates/ragent-tui/src/utils.rs` (or add to existing util module)
- `crates/ragent-tui/src/layout.rs` (remove duplicate)
- `crates/ragent-tui/src/widgets/permission_dialog.rs` (use shared version)

**Implementation approach:**
```rust
// utils.rs
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
```

**Estimated effort:** 1-2 hours  
**Dependencies:** None  
**Parallelizable with:** Task 2.2, 2.3

---

### Phase 3: Component Standardization

#### Task 3.1: Create Dialog Component
**Files to modify:**
- Create: `crates/ragent-tui/src/widgets/dialog.rs`
- `crates/ragent-tui/src/widgets/mod.rs` (export)

**Implementation approach:**
```rust
pub enum DialogVariant {
    Info,      // Cyan border
    Warning,   // Yellow border
    Danger,    // Red border
    Success,   // Green border
}

pub enum DialogSize {
    Small,     // 60x30
    Medium,    // 70x40
    Large,     // 90x70
    Custom(u16, u16),
}

pub struct Dialog {
    pub title: String,
    pub variant: DialogVariant,
    pub size: DialogSize,
    pub alignment: Alignment,
}

impl Dialog {
    pub fn border_color(&self) -> Color {
        match self.variant {
            DialogVariant::Info => theme::colors::dialog::INFO,
            DialogVariant::Warning => theme::colors::dialog::WARNING,
            DialogVariant::Danger => theme::colors::dialog::DANGER,
            DialogVariant::Success => theme::colors::dialog::SUCCESS,
        }
    }
    
    pub fn render(&self, frame: &mut Frame, content: impl Widget) {
        // Standardized dialog rendering
    }
}
```

**Estimated effort:** 6-8 hours  
**Dependencies:** Phase 2 complete  
**Parallelizable with:** Task 3.2, 3.3, 3.4

---

#### Task 3.2: Create Button Component
**Files to modify:**
- Create: `crates/ragent-tui/src/widgets/button.rs`
- `crates/ragent-tui/src/layout.rs` (refactor Agents/Teams toggle)

**Implementation approach:**
```rust
pub enum ButtonState {
    Enabled,
    Disabled,
    Active,  // Currently selected/pressed
}

pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
}

pub struct Button {
    pub label: String,
    pub state: ButtonState,
    pub variant: ButtonVariant,
    pub icon: Option<String>,
}
```

**Estimated effort:** 4-6 hours  
**Dependencies:** Task 2.1  
**Parallelizable with:** Task 3.1, 3.3, 3.4

---

#### Task 3.3: Create InputField Component
**Files to modify:**
- Create: `crates/ragent-tui/src/widgets/input_field.rs`
- `crates/ragent-tui/src/layout.rs` (refactor all input fields)

**Implementation approach:**
```rust
pub struct InputField {
    pub value: String,
    pub prefix: Option<(String, Style)>,
    pub mask_char: Option<char>,
    pub cursor_style: Style,
}

impl InputField {
    pub fn with_prefix(prefix: &str, style: Style) -> Self { ... }
    pub fn masked(mask: char) -> Self { ... }
    pub fn render(&self, area: Rect, buf: &mut Buffer) { ... }
}
```

**Estimated effort:** 6-8 hours  
**Dependencies:** Task 2.1  
**Parallelizable with:** Task 3.1, 3.2, 3.4

---

#### Task 3.4: Create SelectableList Component
**Files to modify:**
- Create: `crates/ragent-tui/src/widgets/selectable_list.rs`
- `crates/ragent-tui/src/layout.rs` (refactor provider/model/agent lists)

**Implementation approach:**
```rust
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected: usize,
    pub display_fn: fn(&T) -> String,
}

impl<T> SelectableList<T> {
    pub fn next(&mut self) { ... }
    pub fn prev(&mut self) { ... }
    pub fn render(&self, area: Rect, buf: &mut Buffer) { ... }
}
```

**Estimated effort:** 4-6 hours  
**Dependencies:** Task 1.3  
**Parallelizable with:** Task 3.1, 3.2, 3.3

---

#### Task 3.5: Merge LSP/MCP Discover Dialogs
**Files to modify:**
- Create: `crates/ragent-tui/src/widgets/discover_dialog.rs`
- `crates/ragent-tui/src/layout.rs` (replace render_lsp_discover_dialog, render_mcp_discover_dialog)

**Implementation approach:**
```rust
pub struct DiscoverDialog {
    pub title: String,
    pub border_color: Color,
    pub items: Vec<DiscoverItem>,
    pub selected: usize,
}

// Usage:
// let dialog = DiscoverDialog::new("/lsp discover", theme::colors::INFO, items);
// let dialog = DiscoverDialog::new("/mcp discover", theme::colors::SECONDARY, items);
```

**Estimated effort:** 6-8 hours  
**Dependencies:** Task 3.1  
**Parallelizable with:** None (last component task)

---

### Phase 4: Polish

#### Task 4.1: Standardize Icon Set
**Files to modify:**
- `crates/ragent-tui/src/theme.rs` (add icons module)
- `crates/ragent-tui/src/widgets/message_widget.rs`
- `crates/ragent-tui/src/layout.rs`

**Decision Required:** Choose ONE of:
- **Option A:** Unicode symbols only (●✓✗◌▸▶)
- **Option B:** Nerd Fonts (requires font support)
- **Option C:** Emoji with fallbacks

**Implementation approach:**
```rust
pub mod icons {
    // Unicode-only option
    pub const THOUGHT: &str = "◌ ";
    pub const FILE: &str = "▸ ";
    pub const TASK: &str = "▶ ";
    pub const SUCCESS: &str = "● ";
    pub const ERROR: &str = "✗ ";
    pub const RUNNING: &str = "◆ ";
    pub const HEALTHY: &str = "● ";
    pub const UNKNOWN: &str = "◌ ";
}
```

**Estimated effort:** 4-6 hours (depends on decision)  
**Dependencies:** Phase 3 complete  
**Parallelizable with:** Task 4.2

---

#### Task 4.2: Unify Dialog Border Colors
**Files to modify:**
- `crates/ragent-tui/src/layout.rs` (all dialog rendering functions)
- `crates/ragent-tui/src/widgets/permission_dialog.rs`

**Implementation approach:**
Replace hardcoded colors with semantic variants:
```rust
// Before
.border_style(Style::default().fg(Color::Cyan))
.border_style(Style::default().fg(Color::Magenta))  // Inconsistent!

// After
.border_style(Style::default().fg(theme::colors::dialog::INFO))
```

**Estimated effort:** 2-3 hours  
**Dependencies:** Task 3.1  
**Parallelizable with:** Task 4.1

---

#### Task 4.3: Create Status Icon Legend
**Files to modify:**
- `crates/ragent-tui/src/layout.rs` (add to status bar or help panel)

**Implementation approach:**
Add legend line to status bar:
```rust
// In teams panel footer
lines.push(Line::from(vec![
    Span::styled("◌ spawning  ", theme::styles::muted()),
    Span::styled("▶ working  ", theme::styles::muted()),
    Span::styled("● idle  ", theme::styles::muted()),
    Span::styled("◈ blocked  ", theme::styles::muted()),
    Span::styled("✗ failed", theme::styles::muted()),
]));
```

**Estimated effort:** 2-3 hours  
**Dependencies:** Task 4.1  
**Parallelizable with:** Task 4.2

---

#### Task 4.4: Documentation and Style Guide
**Files to modify:**
- Create: `docs/ui-style-guide.md`

**Content:**
- Color palette with usage guidelines
- Typography scale and semantic roles
- Spacing token reference
- Component usage examples
- Accessibility guidelines

**Estimated effort:** 4-6 hours  
**Dependencies:** All Phase 4 tasks  
**Parallelizable with:** None

---

## 5. Definition of Done Criteria

### Phase 1 Done When:
- [ ] `theme.rs` module exists and compiles
- [ ] All `Color::DarkGray` instances replaced with accessible grays
- [ ] Focus indicators use `theme::focus` constants
- [ ] All modal dialogs display keyboard navigation hints

### Phase 2 Done When:
- [ ] Complete color palette defined in `theme::colors`
- [ ] Typography system with semantic roles implemented
- [ ] Spacing tokens defined and used throughout
- [ ] `centered_rect` exists in single shared location

### Phase 3 Done When:
- [ ] `Dialog` component used in all modal dialogs
- [ ] `Button` component replaces inline button logic
- [ ] `InputField` component unifies all input patterns
- [ ] `SelectableList` component used for all lists
- [ ] LSP and MCP dialogs share `DiscoverDialog` component

### Phase 4 Done When:
- [ ] All icons use standardized set from `theme::icons`
- [ ] Dialog border colors follow semantic rules
- [ ] Status legend visible in UI
- [ ] UI style guide documentation complete

### Final Done When:
- [ ] All 72 audit findings addressed or explicitly deferred
- [ ] No hardcoded colors outside `theme.rs`
- [ ] No duplicate layout helpers
- [ ] WCAG AA contrast compliance achieved for all text
- [ ] Consistent focus indicators across all interactive elements
- [ ] `cargo clippy` passes with no new warnings
- [ ] `cargo test` passes

---

## Appendix: Dependency Graph

```
Phase 1
├── Task 1.1 (Theme Module) ──┬── Task 1.2 (Color Contrast)
│                               ├── Task 1.3 (Focus Indicators)
│                               └── Task 1.4 (Keyboard Hints)
│
Phase 2 (depends on Phase 1)
├── Task 2.1 (Complete Theme) ──┬── Task 2.2 (Typography)
│                               ├── Task 2.3 (Spacing Tokens)
│                               └── Task 2.4 (Centered Rect) [can be parallel]
│
Phase 3 (depends on Phase 2)
├── Task 3.1 (Dialog) ──┬── Task 3.5 (Merge LSP/MCP)
├── Task 3.2 (Button)
├── Task 3.3 (InputField)
└── Task 3.4 (SelectableList)

Phase 4 (depends on Phase 3)
├── Task 4.1 (Standardize Icons)
├── Task 4.2 (Unify Border Colors)
├── Task 4.3 (Status Legend)
└── Task 4.4 (Documentation) [depends on all Phase 4]
```

---

*Generated from swarm audit reports: visual.md, components.md, layout.md, a11y.md*  
*Total estimated effort: 70-90 hours across all phases*