# Status Bar Redesign — Executive Summary

**Date:** 2025-01-16  
**Status:** ✅ Complete (Ready for Review)  
**Document:** STATUSBAR_PLAN.md (564 lines)

---

## Overview

The ragent TUI status bar is information-rich but **visually cluttered and inconsistently organized**. This plan proposes a comprehensive redesign that improves aesthetics, clarity, and responsive behavior while preserving all critical information.

### Current Problems

| Issue | Impact | Severity |
|-------|--------|----------|
| Cluttered layout (too much info in 2 lines) | Cognitive overload | 🔴 High |
| Inconsistent separators (│ ⎇ mixed) | Visual confusion | 🟡 Medium |
| Flat hierarchy (all items equal weight) | Loss of information priority | 🔴 High |
| Fixed size (no responsive adaptation) | Breaks on small terminals | 🟡 Medium |
| AIWiki indicator overwhelming | Takes up 1/4 of Line 1 | 🟡 Medium |

---

## Proposed Solution: 3-Section Layout

### Line 1: Context & Status
```
/home/user/project                 main ●                 Ready
└─ Working Directory (25 chars)     └─ Git Branch (15)     └─ Status Msg
```

**Contents:**
- **Left**: Working directory (with tilde shortening, 25-char limit)
- **Center**: Git branch + status indicator (● clean, ◦ changes, ✗ conflict)
- **Right**: Session status message (animated if working)

**Visual Weight:** Equal distribution, clear grouping

---

### Line 2: System & Resources
```
● claude │ tokens: 45% ████░░░░   tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
└─ LLM Provider (25 chars)          └─ Resources (25)      └─ Services (30)
```

**Contents:**
- **Left**: LLM provider + model + health (● green/✗ red)
- **Center**: Token usage (percentage + visual bar)
- **Right**: Service status (LSP, CodeIdx, AIWiki as compact indicators)

**Visual Weight:** Provider priority > Resources > Services

---

## Key Improvements

| Metric | Current | Proposed | Improvement |
|--------|---------|----------|-------------|
| **Visual Sections** | Unstructured | 3 per line | Clearer hierarchy |
| **Separator Consistency** | Mixed (│ ⎇) | Uniform (│) | Professional |
| **Color Coding** | Limited | Semantic 4-color | Intuitive |
| **Responsive Layouts** | None | 3 breakpoints | Accessible |
| **Info Density** | Fixed | Adaptive | Smart scaling |
| **Overall Clutter** | High | Low | Reduced noise |

---

## Implementation Roadmap

### 6 Phases, 7 Weeks, 27 Tasks

```
Week 1:  Design & Approval                    [Milestone 1.1]
Week 2-3: Core Layout Engine                 [Milestone 2.1]
Week 4:   Visual Polish & Indicators         [Milestone 3.1]
Week 5:   Responsive & Adaptive              [Milestone 4.1]
Week 6:   Testing & Validation               [Milestone 5.1]
Week 7:   Documentation & Release            [Milestone 6.1]
```

### Phase Details

**Phase 1: Specification (Week 1)**
- [ ] Finalize visual design
- [ ] Create before/after mockups
- [ ] Gather feedback from team
- [ ] Approve direction

**Phase 2: Core Layout (Weeks 2-3)**
- [ ] Create `StatusBarLine` structure
- [ ] Implement Line 1 (Context & Status)
- [ ] Implement Line 2 (System & Resources)
- [ ] Responsive width handling

**Phase 3: Visual Polish (Week 4)**
- [ ] Status indicators (● ◔ ✗ ✓)
- [ ] Color coding (Green/Yellow/Red/Cyan)
- [ ] Progress bars (████░)
- [ ] Theme integration

**Phase 4: Responsive Design (Week 5)**
- [ ] Full layout (≥120 chars)
- [ ] Compact layout (80-120 chars)
- [ ] Minimal layout (<80 chars)
- [ ] Graceful degradation

**Phase 5: Testing (Week 6)**
- [ ] Unit & integration tests
- [ ] Visual validation (multiple terminal sizes)
- [ ] Performance benchmarks (<5ms/frame)
- [ ] Accessibility review

**Phase 6: Release (Week 7)**
- [ ] Update SPEC.md
- [ ] Create user guide (docs/tui-statusbar-guide.md)
- [ ] Update CHANGELOG.md
- [ ] Push to remote

---

## Responsive Breakpoints

### Breakpoint 1: Full Layout (≥120 chars)
```
/home/user/project                 main ●                 Ready
● claude │ tokens: 45% ████░░░░   tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
```

### Breakpoint 2: Compact Layout (80-120 chars)
```
~/project              main ●      Ready
claude  tok:45% ████   tasks:1  LSP:✓ CodeIdx:✓ AIWiki:✓
```

### Breakpoint 3: Minimal Layout (<80 chars)
```
~/proj  main ●
claude  tok:45%  LSP:✓
(Full info via `/status` command)
```

---

## Color Palette

**Semantic Color Coding:**
```
● Green (#00FF00)   — Ready, healthy, enabled, clean
◔ Yellow (#FFFF00)  — Warning, slow, processing, changes
✗ Red (#FF0000)     — Error, failed, disabled, conflict
◦ Cyan (#00FFFF)    — In progress, changed, syncing
```

**Application:**
- Health indicators: ● (green for healthy, ✗ for error)
- Git status: ● clean, ◦ changes, ✗ conflict
- Service status: ✓ enabled (green), ✗ disabled (red)
- Warnings: Yellow when > 80% capacity

---

## Success Criteria

### Visual & Aesthetic
✅ Reduced clutter (max 3-4 sections per line)
✅ Consistent visual hierarchy
✅ Professional appearance
✅ Clear information grouping

### Functional
✅ No loss of critical information
✅ All current metrics preserved
✅ Accessible via commands (e.g., `/status verbose`)
✅ Responsive to terminal size changes

### Technical
✅ No performance regressions (<5ms/frame)
✅ Smooth animations (no jank)
✅ Cross-terminal compatibility
✅ 100% test coverage of layout logic

---

## Files to Modify

**New Files:**
- `crates/ragent-tui/src/layout_statusbar.rs` — Dedicated status bar rendering
- `docs/tui-statusbar-guide.md` — User guide
- `crates/ragent-tui/tests/test_statusbar_layout.rs` — Tests
- `crates/ragent-tui/tests/test_statusbar_responsive.rs` — Responsive tests

**Modified Files:**
- `crates/ragent-tui/src/layout.rs` — Refactor status_bar function
- `crates/ragent-tui/src/theme.rs` — New styles for status bar
- `crates/ragent-tui/src/utils.rs` — Responsive breakpoint logic
- `crates/ragent-tui/src/app.rs` — Add verbose_status flag
- `SPEC.md` — Update Section 4.1.1 (Main Screen)
- `CHANGELOG.md` — Document changes
- `RELEASE.md` — Version notes

---

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| **Layout breaks on small terminals** | Comprehensive responsive testing across 80x24, 120x40, 180x50 |
| **Color blindness accessibility** | Use patterns + colors (not color-only); colorblind testing |
| **Performance regression** | Before/after benchmarks; optimize rendering pipeline |
| **User confusion with new layout** | Clear documentation; optional `/statusbar old` fallback |
| **Git status detection failure** | Fallback to branch-only display if status unavailable |

---

## Future Enhancements

These are NOT part of this phase but identified for future work:

1. **Click Detection** — Click on sections to expand/collapse
2. **Hover Tooltips** — Detailed info on hover
3. **Customizable Layout** — User-defined section ordering
4. **Color Themes** — Pre-built themes (Solarized, Dracula, etc.)
5. **Animated Transitions** — Smooth fade-in/out
6. **Status History** — Scroll through recent status changes
7. **Custom Indicators** — User-defined status symbols

---

## Next Steps (Actionable)

1. **Review & Approve**
   - [ ] Read STATUSBAR_PLAN.md
   - [ ] Provide feedback by [DATE]
   - [ ] Get sign-off from TUI team

2. **Create GitHub Issues**
   - [ ] One issue per phase (6 total)
   - [ ] Link to STATUSBAR_PLAN.md
   - [ ] Assign to team members

3. **Schedule Kickoff**
   - [ ] Phase 1 kickoff meeting
   - [ ] Finalize mockups
   - [ ] Assign Phase 2 tasks

4. **Communicate Plan**
   - [ ] Update team wiki/docs
   - [ ] Post announcement in chat
   - [ ] Set expectations for timeline

---

## Timeline Summary

| Phase | Duration | Effort | Milestone |
|-------|----------|--------|-----------|
| 1. Design | 1 week | 5 days | Design Document ✅ |
| 2. Core Layout | 2 weeks | 10 days | Rendering Complete ✅ |
| 3. Visual Polish | 1 week | 5 days | Visual Design ✅ |
| 4. Responsive | 1 week | 5 days | Responsive Design ✅ |
| 5. Testing | 1 week | 5 days | Testing Complete ✅ |
| 6. Release | 1 week | 3 days | Release Ready ✅ |
| **TOTAL** | **7 weeks** | **~33 days** | **Production** |

---

## Deliverables

✅ **STATUSBAR_PLAN.md** — Comprehensive implementation plan (564 lines)

**Upon Completion:**
- New status bar layout engine
- Responsive design system (3 breakpoints)
- Test suite (unit + integration + responsive)
- User documentation
- Updated SPEC.md and CHANGELOG.md

---

## Document Location

📄 **File:** `/home/thawkins/Projects/ragent/STATUSBAR_PLAN.md`  
📊 **Size:** 564 lines, 19 KB  
📅 **Created:** 2025-01-16  
✅ **Status:** Ready for Review

---

## Contact & Feedback

Please review STATUSBAR_PLAN.md and provide feedback on:
- Design direction (layout, colors, indicators)
- Feasibility assessment
- Timeline and resource needs
- Technical concerns or questions
- Alternative approaches

---

**End of Summary**

