# Status Bar Redesign Plan

## Executive Summary

The current 2-line status bar is information-rich but visually cluttered, with inconsistent grouping and alignment. This plan proposes a comprehensive redesign that reduces visual clutter, improves information hierarchy, and maintains consistency while preserving all critical information.

---

## Current Layout Analysis

### Line 1: Context Information
```
● Ragent: v0.1.0-alpha.44 │ /home/user/project → /tmp/shell │ ⎇ main [status] AIWiki: ✓ 5src/10pg [⟳⊙] 2/15
```

**Contents (Left to Right):**
- Ragent version (green indicator)
- Current working directory
- Shell-specific cwd (if different)
- Git branch
- Status message (if not "Ready")
- AIWiki indicator with stats
- AIWiki sync status

**Issues:**
- Too much information crammed into one line
- Inconsistent use of separators (│ → ⎇ inconsistent)
- AIWiki stats visually overwhelming
- Status message placement unclear
- No clear visual grouping

---

### Line 2: Resources & System State
```
● anthropic/claude-sonnet-4 │ tokens: 2,450/200,000 │ active: 1 │ LSP: ✓ 3 │ CodeIdx: ✓ │ Log: 234
```

**Contents (Left to Right):**
- LLM provider with health indicator
- Token usage (current/max)
- Context window usage percentage
- Active background tasks count
- LSP status and count
- Code index status
- Log entry count

**Issues:**
- Separator consistency (mixed │ usage)
- Token display could be more intuitive
- LSP/CodeIdx abbreviations lack clarity
- Log indicator uninformative
- All items have same visual weight

---

## Design Goals

1. **Reduce Clutter** — Group related information, use consistent visual hierarchy
2. **Preserve Information** — No loss of critical data; improve discoverability
3. **Improve Aesthetics** — Consistent spacing, alignment, and indicator usage
4. **Better Consistency** — Uniform separators, color coding, and styling patterns
5. **Responsive** — Gracefully degrade on smaller terminals without truncating critical info

---

## Proposed Layout

### Restructuring Approach

**The new design uses 3 "sections" per line instead of unrelated items:**

#### Line 1: Context & Status (Working Directory, Git, Status)
```
 Project   │ Session  │ Status
─────────────────────────────────────────────────────────────────────
/home/user/project  main ●  •  Ready
```

**Structure:**
- **Left**: Working directory (shortened if needed)
- **Center**: Git branch with status indicator
- **Right**: Agent status with visual indicator

---

#### Line 2: System & Resources (Provider, Tokens, Tasks, Tools)
```
 Model    │ Resources │ Tools & Services
─────────────────────────────────────────────────────────────────────
claude    tokens: 45%  tasks: 1  LSP: ✓  CodeIdx: ✓  AIWiki: ✓
```

**Structure:**
- **Left**: LLM provider + model name + health
- **Center**: Token usage (percentage + bar)
- **Right**: Service status indicators (LSP, CodeIdx, AIWiki, Log count)

---

## Detailed Layout Specifications

### NEW LINE 1: Context & Status

```
┌─ SECTION 1: Working Directory ──┬─ SECTION 2: Git ──┬─ SECTION 3: Status ──┐
│                                  │                   │                       │
│  /home/user/project              │  main ●           │  Ready                │
│  ~20 chars (shortened)           │  branch + status  │  or status message    │
└──────────────────────────────────┴───────────────────┴───────────────────────┘
```

#### Design Details

**Column 1: Working Directory**
- Show current directory path
- Use tilde (`~`) for home directory
- Shorten middle if >20 chars: `/home/user/proj…/src`
- Right-align indicator: `●` (home) or `✓` (other)
- Visual indicator for writable: ✓ (green), ⚠ (warning), ✗ (read-only)
- No separators within section

**Column 2: Git Information**
- Branch name in green
- Status indicator: 
  - `●` = clean (green)
  - `◦` = uncommitted changes (yellow)
  - `✗` = conflict (red)
  - `↓` = behind remote (cyan)
  - `↑` = ahead of remote (cyan)
- Shell cwd if different: `shell: /tmp/work` (optional, compact row if space)
- Consistent width with branch names

**Column 3: Session Status**
- Large message area for status updates
- "Ready" on idle
- Animated indicator for active operations: `⟳ Executing...`
- Color-coded: Green (ready), Yellow (working), Red (error), Cyan (busy)

**Column Separators:** Light gray `│` or `┆` (subtle, not bold)

---

### NEW LINE 2: System Resources & Services

```
┌─ SECTION 1: LLM Provider ──┬─ SECTION 2: Resources ─┬─ SECTION 3: Services ──┐
│                            │                         │                        │
│  ● claude │ 4K tokens      │  tasks: 1, mem: 128MB   │  LSP:✓ CodeIdx:✓ AIWiki:✓ │
│  icon color model          │  visual + numbers       │  compact indicators      │
└────────────────────────────┴─────────────────────────┴────────────────────────┘
```

#### Design Details

**Column 1: LLM Provider**
- Health indicator: 
  - `●` green = healthy
  - `◔` yellow = slow
  - `✗` red = error
- Provider/model label: `claude` or `gpt-4o` (short form)
- Context window: `4K tokens` (truncated, e.g., "4K", "128K", "200K")
- Width: ~25 chars (fixed)

**Column 2: Resource Indicators**
- **Token usage** (primary):
  - Percentage with bar: `tokens: 45% ████░░░░░░`
  - Or numerical: `2.4K / 8K`
  - Color: Green (<50%), Yellow (50-80%), Red (>80%)
  - Alternative: Only show if >75% or on hover
  
- **Active tasks**: `↻tasks:1` (if >0), compact format
- **Memory**: Optional, shown only if >200MB or on demand
  
- Width: ~30 chars

**Column 3: Service Status**
- Compact service indicators (icon only, no text except on demand):
  - `LSP:✓` (green if connected, red if error)
  - `CodeIdx:✓` (green if ready, yellow if indexing)
  - `AIWiki:✓` (green if enabled, red if disabled, cyan if syncing)
- Optional compact counter: `Log: 234 entries` (hidden by default, shown on demand)
- Width: Flexible, right-aligned, fits remaining space

---

## Information Hierarchy

### Line 1 (Context)
```
Priority 1 (Always):   Working directory (where you are)
Priority 2 (Always):   Git branch & status (what you're working on)
Priority 3 (Dynamic):  Session status (what's happening now)
```

### Line 2 (System)
```
Priority 1 (Always):   LLM provider health (can agent respond?)
Priority 2 (Always):   Token usage (am I running out?)
Priority 3 (Always):   Service status (are tools working?)
Priority 4 (On-demand): Detailed metrics (log count, memory, etc.)
```

---

## Visual Styling

### Colors (Consistent throughout)
```
Status/Health Indicators:
  ● Green (#00FF00)   — Ready, healthy, enabled
  ◔ Yellow (#FFFF00)  — Warning, slow, processing
  ✗ Red (#FF0000)     — Error, failed, disabled
  ◦ Cyan (#00FFFF)    — In progress, changed, syncing

Text:
  Default     — White (#FFFFFF)
  Emphasis    — Cyan (#00FFFF) for values/metrics
  Deemphasis — DarkGray (#808080) for labels
  Alert       — Red/Yellow as needed
```

### Separators
```
Column divider (subtle):  │ or ┆ in DarkGray
Visual grouping:          Light spacing or background color change
Status animation:         ⟳ spinning indicator
```

### Indicators
```
Status dots:    ● (circle for state)
Check marks:    ✓ (enabled/healthy)
Cross marks:    ✗ (disabled/error)
Directional:    ↑ ↓ ← → (direction/flow)
Progress:       ⟳ (spinner), ▓ (bar)
```

---

## Responsive Behavior

### Terminal Width ≥120 chars (Full Layout)
```
Line 1: /home/user/project     main ●            Ready
Line 2: ● claude │ tokens: 45% ████░░  LSP:✓ CodeIdx:✓ AIWiki:✓ Log:234
```

### Terminal Width 80-120 chars (Compact Layout)
```
Line 1: ~/project  main ●  Ready
Line 2: claude  tok:45% ████  LSP:✓ CodeIdx:✓ AIWiki:✓
```

### Terminal Width <80 chars (Minimal Layout)
```
Line 1: ~/proj  main ●
Line 2: claude  tok:45%  LSP:✓
(Additional info available via `/status` command)
```

---

## Implementation Roadmap

### Phase 1: Specification & Planning (Week 1)
- [ ] Finalize visual design in STATUSBAR_PLAN.md
- [ ] Create before/after mockups
- [ ] Gather feedback from team
- [ ] Approve design direction

**Milestone 1.1:** Design Document Complete

---

### Phase 2: Core Layout Engine (Week 2-3)

#### Task 2.1: Create New Layout Structure
- [ ] Define `StatusBarSection` enum (Left, Center, Right)
- [ ] Define `StatusBarLine` struct with 3 sections
- [ ] Create `StatusBarLayout` trait for different responsive modes
- **Estimated:** 1 day
- **Files to Create/Modify:**
  - `crates/ragent-tui/src/layout_statusbar.rs` (new)
  - `crates/ragent-tui/src/layout.rs` (refactor render_status_bar)

#### Task 2.2: Implement Line 1 Rendering (Context & Status)
- [ ] Working directory section (shortening, color coding)
- [ ] Git branch section (branch + status indicator)
- [ ] Status message section (alignment, animation)
- [ ] Responsive width handling
- **Estimated:** 2 days
- **Files to Modify:**
  - `crates/ragent-tui/src/layout_statusbar.rs`
  - `crates/ragent-tui/src/layout.rs`

#### Task 2.3: Implement Line 2 Rendering (System & Resources)
- [ ] LLM provider section (health, model, tokens)
- [ ] Resource section (token percentage, bar, active tasks)
- [ ] Service indicators section (LSP, CodeIdx, AIWiki, Log)
- [ ] Responsive width handling
- **Estimated:** 2 days
- **Files to Modify:**
  - `crates/ragent-tui/src/layout_statusbar.rs`
  - `crates/ragent-tui/src/layout.rs`

**Milestone 2.1:** Core Layout Rendering Complete

---

### Phase 3: Visual Polish & Indicators (Week 4)

#### Task 3.1: Implement Status Indicators
- [ ] Health status dots (● ◔ ✗) with colors
- [ ] Service status badges (✓ ✗ with colors)
- [ ] Direction indicators (↑ ↓ → for git status)
- [ ] Progress spinners (⟳ for syncing/processing)
- [ ] Token usage bar (████░░░░░░)
- **Estimated:** 2 days
- **Files to Modify:**
  - `crates/ragent-tui/src/layout_statusbar.rs`
  - `crates/ragent-tui/src/theme.rs` (add new styles)

#### Task 3.2: Implement Color Coding
- [ ] Consistent color palette (Green, Yellow, Red, Cyan)
- [ ] Semantic coloring (status = color meaning)
- [ ] Theme integration (respect user theme settings)
- [ ] Test on common terminal backgrounds
- **Estimated:** 1.5 days
- **Files to Modify:**
  - `crates/ragent-tui/src/theme.rs`
  - `crates/ragent-tui/src/layout_statusbar.rs`

**Milestone 3.1:** Visual Design Complete

---

### Phase 4: Responsive & Adaptive Behavior (Week 5)

#### Task 4.1: Responsive Breakpoints
- [ ] Full layout (≥120 chars)
- [ ] Compact layout (80-120 chars)
- [ ] Minimal layout (<80 chars)
- [ ] Graceful degradation (no truncation of critical info)
- **Estimated:** 2 days
- **Files to Modify:**
  - `crates/ragent-tui/src/utils.rs` (ResponsiveBreakpoint)
  - `crates/ragent-tui/src/layout_statusbar.rs`
  - `crates/ragent-tui/src/layout.rs`

#### Task 4.2: Dynamic Information Hiding
- [ ] Detailed metrics hidden until space available
- [ ] Optional toggle for verbose mode (`/status verbose`)
- [ ] Hover/click to expand sections (future enhancement)
- **Estimated:** 1.5 days
- **Files to Modify:**
  - `crates/ragent-tui/src/layout_statusbar.rs`
  - `crates/ragent-tui/src/app.rs` (add verbose flag)

**Milestone 4.1:** Responsive Design Complete

---

### Phase 5: Testing & Validation (Week 6)

#### Task 5.1: Unit & Integration Tests
- [ ] Layout rendering tests (various terminal widths)
- [ ] Indicator logic tests (colors, animations)
- [ ] Responsive breakpoint tests
- [ ] Git status indicator tests
- **Estimated:** 2 days
- **Test Files:**
  - `crates/ragent-tui/tests/test_statusbar_layout.rs` (new)
  - `crates/ragent-tui/tests/test_statusbar_responsive.rs` (new)

#### Task 5.2: Visual Testing & Feedback
- [ ] Manual testing on 80x24, 120x40, 180x50 terminals
- [ ] Color compatibility testing (dark/light themes)
- [ ] Animation smoothness validation
- [ ] Accessibility review (color blind friendly?)
- **Estimated:** 2 days

#### Task 5.3: Performance Testing
- [ ] Rendering performance (<5ms per frame)
- [ ] Memory usage (no regressions)
- [ ] Benchmark on slow terminals
- **Estimated:** 1 day
- **Benchmark Files:**
  - `crates/ragent-tui/benches/bench_statusbar.rs` (if needed)

**Milestone 5.1:** Testing Complete

---

### Phase 6: Documentation & Release (Week 7)

#### Task 6.1: Update Specification
- [ ] Update SPEC.md Section 4.1.1 (Main Screen / Status Bar)
- [ ] Add ASCII art examples of new layout
- [ ] Document responsive breakpoints
- [ ] Document color meanings
- **Estimated:** 1 day
- **Files to Modify:**
  - `SPEC.md` (Section 4.1.1)

#### Task 6.2: User Documentation
- [ ] Update README with status bar examples
- [ ] Create tutorial image (before/after)
- [ ] Document `/status verbose` command
- [ ] Add troubleshooting guide
- **Estimated:** 1 day
- **Files to Create/Modify:**
  - `docs/tui-statusbar-guide.md` (new)
  - `README.md` (update screenshots)

#### Task 6.3: Changelog & Release Notes
- [ ] Add entry to CHANGELOG.md
- [ ] Update RELEASE.md
- [ ] Version bump (minor: 0.2.0 or patch: 0.1.45)
- **Estimated:** 0.5 days

**Milestone 6.1:** Release Ready

---

## Key Files to Modify

### Core Layout Files
- `crates/ragent-tui/src/layout.rs` — Main layout rendering (refactor status_bar function)
- `crates/ragent-tui/src/layout_statusbar.rs` — NEW, dedicated status bar rendering
- `crates/ragent-tui/src/utils.rs` — ResponsiveBreakpoint enhancements
- `crates/ragent-tui/src/theme.rs` — New status bar styles & colors

### Supporting Files
- `crates/ragent-tui/src/app.rs` — Add verbose_status flag
- `crates/ragent-tui/src/app/state.rs` — Status bar state management

### Documentation Files
- `SPEC.md` — Update Section 4.1.1 (Main Screen)
- `CHANGELOG.md` — Document changes
- `RELEASE.md` — Version notes
- `docs/tui-statusbar-guide.md` — NEW user guide

### Test Files
- `crates/ragent-tui/tests/test_statusbar_layout.rs` — NEW
- `crates/ragent-tui/tests/test_statusbar_responsive.rs` — NEW
- `crates/ragent-tui/benches/bench_statusbar.rs` — Optional performance benchmarks

---

## Success Criteria

✅ **Clutter Reduction:**
- Fewer visual elements per line (goal: 3-4 major sections max)
- Consistent use of separators and spacing
- No overlapping or conflicting information

✅ **Information Preservation:**
- All current status bar data remains accessible
- No loss of critical indicators (health, tokens, services)
- Detailed info available via `/status` command

✅ **Visual Consistency:**
- Uniform color palette (Green, Yellow, Red, Cyan)
- Consistent indicator styles across all sections
- Aligned text and spacing

✅ **Responsive Design:**
- Full layout on ≥120 char terminals
- Graceful degradation on smaller terminals
- Critical info always visible (never hidden by truncation)

✅ **Aesthetic Improvement:**
- Terminal-native design (clean, minimal)
- Professional appearance
- Visually distinct sections with clear hierarchy

✅ **Performance:**
- No rendering regressions (<5ms per frame)
- Smooth animations (no jank)
- No memory leaks or excessive allocations

---

## Mockups & Examples

### Current Layout (Status Quo)
```
● Ragent: v0.1.0-alpha.44 │ /home/user/project → /tmp/shell │ ⎇ main [status] AIWiki: ✓ 5src/10pg [⟳⊙] 2/15
● anthropic/claude-sonnet-4 │ tokens: 2,450/200,000 │ active: 1 │ LSP: ✓ 3 │ CodeIdx: ✓ │ Log: 234
```

### Proposed Layout (New Design)
```
/home/user/project                 main ●                 Ready
● claude │ tokens: 45% ████░░░░   tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
```

### Minimal Layout (<80 chars)
```
~/proj  main ●
claude  tok:45%  LSP:✓
```

### Verbose Layout (on demand)
```
/home/user/project                 main ●  +5 changes        Ready
● claude │ tokens: 2.4K/8K (45%) ████  tasks: 1, mem: 128MB   LSP:✓(3) CodeIdx:✓ AIWiki:✓ Log:234
```

---

## Risk Assessment & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Layout breaks on certain terminal sizes | Medium | High | Comprehensive responsive testing across all breakpoints |
| Color blindness accessibility issue | Low | Medium | Use patterns + colors (not color-only); test with colorblind simulator |
| Performance regression on old terminals | Low | High | Benchmark before/after; optimize rendering |
| User confusion with new layout | Medium | Low | Clear documentation + gradual rollout; `/statusbar old` fallback initially |
| Git status detection not working | Low | Medium | Fall back to branch-only display if status unavailable |

---

## Future Enhancements (Post-Release)

1. **Click Detection** — Click on sections to expand/collapse details
2. **Hover Tooltips** — Hover to see full information
3. **Configurable Layout** — Allow users to customize section order/visibility
4. **Themes** — Pre-built color themes (solarized, dracula, etc.)
5. **Animated Transitions** — Smooth fade-in/out when data changes
6. **Status History** — Scroll through recent status changes
7. **Custom Indicators** — User-defined status indicators

---

## Timeline Summary

| Phase | Duration | Milestone |
|-------|----------|-----------|
| 1. Design & Planning | 1 week | Design Document Complete |
| 2. Core Layout | 2 weeks | Layout Rendering Complete |
| 3. Visual Polish | 1 week | Visual Design Complete |
| 4. Responsive Design | 1 week | Responsive Design Complete |
| 5. Testing | 1 week | Testing Complete |
| 6. Documentation | 1 week | Release Ready |
| **Total** | **~7 weeks** | **Production Ready** |

---

## Sign-Off

**Document Created:** 2025-01-16
**Version:** 1.0
**Status:** Draft (Awaiting Review & Approval)

**Reviewers Needed:**
- [ ] TUI Designer/Lead
- [ ] Performance Lead
- [ ] Accessibility Lead
- [ ] Product Manager

---

