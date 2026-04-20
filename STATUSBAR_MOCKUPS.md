# Status Bar Redesign: Before & After Mockups

**Document:** STATUSBAR_MOCKUPS.md  
**Version:** 1.0  
**Status:** Final  
**Created:** 2025-01-16  
**Phase:** 1 (Design & Planning)

---

## Table of Contents

1. [Before & After Overview](#before--after-overview)
2. [Full Layout Mockups (120+ chars)](#full-layout-mockups-120-chars)
3. [Compact Layout Mockups (80-120 chars)](#compact-layout-mockups-80-120-chars)
4. [Minimal Layout Mockups (<80 chars)](#minimal-layout-mockups-80-chars)
5. [State Variations](#state-variations)
6. [Color-Coded Examples](#color-coded-examples)
7. [Animation Frames](#animation-frames)
8. [Accessibility Variants](#accessibility-variants)

---

## Before & After Overview

### Current Status Bar (Cluttered)

```
████████████████████████████████████████████████████████████████████████████
● Ragent: v0.1.0-alpha.44 │ /home/user/project → /tmp/shell │ ⎇ main [AIWiki: ✓ 5src/10pg] [⟳⊙]
● anthropic/claude-sonnet-4 │ tokens: 2,450/200,000 │ active: 1 │ LSP: ✓ 3 │ CodeIdx: ✓ │ Log: 234
████████████████████████████████████████████████████████████████████████████

Problems:
❌ Inconsistent separators (│ vs ⎇)
❌ Too much information crammed together
❌ No clear visual hierarchy or grouping
❌ AIWiki indicator takes up too much space
❌ Status message placement unclear
❌ Difficult to scan for critical information
```

### Proposed Status Bar (Organized)

```
████████████████████████████████████████████████████████████████████████████
/home/user/project                 main ●                 Ready
● claude │ tokens: 45% ████░░░░   tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
████████████████████████████████████████████████████████████████████████████

Benefits:
✅ Clear 3-section layout per line
✅ Consistent visual hierarchy
✅ Uniform separators (│)
✅ Easy to scan
✅ Professional appearance
✅ All information preserved
✅ Responsive to terminal size
```

---

## Full Layout Mockups (120+ chars)

### Scenario 1: Healthy, Idle System

**Current (Old):**
```
● Ragent: v0.1.0-alpha.44 │ /home/developer/ragent → /tmp │ ⎇ main [Ready] AIWiki: ✓ 5src/10pg
● anthropic/claude-sonnet-4 │ tokens: 1,200/200,000 (6%) │ active: 0 │ LSP: ✓ 3 │ CodeIdx: ✓ │ Log: 45
```

**New (Proposed):**
```
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 6% ░░░░░░░░██  tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Improvements:**
- Much cleaner visual layout
- Clear three-section structure
- Consistent spacing and alignment
- Easy to identify status at a glance
- Version info removed from status bar (available via `/about`)

---

### Scenario 2: Active Operation (Refactoring)

**Current (Old):**
```
● Ragent: v0.1.0-alpha.44 │ /home/developer/ragent → /tmp │ ⎇ dev [⟳ Refactoring...] AIWiki: ⟳ 5src/10pg
● anthropic/claude-sonnet-4 │ tokens: 52,400/200,000 (26%) │ active: 2 │ LSP: ✓ 3 │ CodeIdx: ⟳ │ Log: 145
```

**New (Proposed):**
```
/home/developer/ragent             dev ●                 ⟳ Refactoring (4/12)...
● claude │ tokens: 26% ██████░░░░ tasks: 2   LSP:✓ CodeIdx:⟳ AIWiki:✓
```

**Improvements:**
- Status message is now prominent and right-aligned
- Progress indicator (4/12) is visible in status message
- Token usage shown as percentage with visual bar
- Task count and service status clearly visible
- Color coding: Yellow bar indicates warning level (26%)

---

### Scenario 3: Error State (Merge Conflict)

**Current (Old):**
```
✗ Ragent: v0.1.0-alpha.44 │ /home/developer/ragent → /tmp │ ⎇ main [✗ MERGE CONFLICT] AIWiki: ✓
✗ anthropic/claude-sonnet-4 │ tokens: 98,750/200,000 (49%) │ active: 0 │ LSP: ✗ 0 │ CodeIdx: ✗ │ Log: 312
```

**New (Proposed):**
```
/home/developer/ragent             main ✗                 ✗ Merge conflict detected
✗ claude │ tokens: 49% █████░░░░░ tasks: 0   LSP:✗ CodeIdx:✓ AIWiki:✓
```

**Improvements:**
- Red status indicators immediately obvious
- Clear error message in status column
- Service status individually colored (LSP is red, others green)
- User can see exactly what's wrong without extra context
- Less visual noise makes the error stand out

---

### Scenario 4: High Token Usage (Critical)

**Current (Old):**
```
● Ragent: v0.1.0-alpha.44 │ /home/developer/ragent → /tmp │ ⎇ develop [Ready] AIWiki: ✓ 5src/10pg
✗ anthropic/claude-sonnet-4 │ tokens: 183,500/200,000 (92%) │ active: 1 │ LSP: ✓ 3 │ CodeIdx: ✓ │ Log: 523
```

**New (Proposed):**
```
/home/developer/ragent             develop ●              Ready
✗ claude │ tokens: 92% ██████████ tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Improvements:**
- Red progress bar immediately signals critical status
- Provider health indicator (✗) shows provider stress
- User can see at a glance that action is needed
- Token percentage takes precedence over remaining count

---

### Scenario 5: Multiple Services Down

**Current (Old):**
```
● Ragent: v0.1.0-alpha.44 │ /home/developer/ragent → /tmp │ ⎇ main [Ready] AIWiki: ✗ (error)
● anthropic/claude-sonnet-4 │ tokens: 34,200/200,000 (17%) │ active: 0 │ LSP: ✗ error │ CodeIdx: ✗ error │ Log: 89
```

**New (Proposed):**
```
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 17% ██░░░░░░░░  tasks: 0   LSP:✗ CodeIdx:✗ AIWiki:✗
```

**Improvements:**
- Service failures are visually distinct (red indicators)
- Can see at a glance which services are down
- Healthy provider is shown for context
- Status remains "Ready" because it's an informational state

---

## Compact Layout Mockups (80-120 chars)

### Terminal Width: 100 characters

**Current (Truncated, loses info):**
```
● Ragent: v0.1.0 │ /home/developer/ragent → /tmp │ ⎇ main │ AIWiki: ✓
● claude │ tokens: 45% │ active: 1 │ LSP: ✓ │ CodeIdx: ✓ │ Log: 123
```

**New (Graceful degradation):**
```
~/ragent               main ●                 Ready
claude tok: 45% █████░ tasks:1               LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Improvements:**
- Path shortened to home-relative format
- Abbreviated labels (`tok:` instead of `tokens:`)
- Progress bar still visible
- All critical information preserved
- No truncation of important data

---

### Terminal Width: 90 characters

**Current (More truncation):**
```
● Ragent │ /home/develop… │ main │ AIWiki: ✓
● claude │ tokens: 45% │ LSP: ✓ │ CodeIdx: ✓ │ Log
```

**New (Still readable):**
```
~/ragent            main ●           Ready
claude tok:45% ████ tasks:1          LSP:✓ CodeIdx:✓
```

**Improvements:**
- Still shows all critical info
- Compact but readable
- Task count visible
- Service indicators present

---

### Terminal Width: 85 characters

**Current (Information loss):**
```
● Ragent │ /home/dev… │ main │ AIWiki
● claude │ tokens: 45% │ LSP │ CodeI…
```

**New (Minimal but complete):**
```
~/proj            main ●         Ready
claude  tok:45% ██  LSP:✓ CodeIdx:✓
```

**Improvements:**
- Minimum viable information preserved
- Still identifies project location
- Branch status visible
- Critical services shown

---

## Minimal Layout Mockups (<80 chars)

### Terminal Width: 78 characters

**Current (Severe truncation, information loss):**
```
● Ragent │ /home/…/rag │ main │ AIWiki: …
● claude │ tokens: 45% │ LSP: ✓ │ …
(Information cut off, user must scroll)
```

**New (Degraded but complete):**
```
~/proj          main ●
claude  tok:45%  LSP:✓
(Full details available via /status command)
```

**Improvements:**
- Essential info visible without truncation
- User knows they can run `/status` for more detail
- Clear indication: status bar is abbreviated

---

### Terminal Width: 60 characters (Very Small)

**Current (Completely unusable):**
```
● Ragent │ /home… │ …
● claude │ tokens: …
(Most information is lost)
```

**New (Minimal but functional):**
```
~/proj     main ●
claude  tok:45%
(Run /status for full details)
```

**Improvements:**
- Still shows project and branch
- Token usage at a glance
- User directed to `/status` for complete information
- No information loss, just deferral

---

## State Variations

### State 1: Idle (Default)

```
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Characteristics:**
- No animation
- Static display
- Green status (Ready)
- Health indicator: ●

---

### State 2: Processing (Single Operation)

```
/home/developer/ragent             main ●                 ⟳ Executing...
● claude │ tokens: 52% █████░░░    tasks: 1   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Characteristics:**
- Spinning animation on ⟳
- Yellow status (Processing)
- Token usage increased
- Task count: 1

---

### State 3: Processing (Multi-Operation with Progress)

```
/home/developer/ragent             main ●                 ⟳ Refactoring (4/12)...
● claude │ tokens: 67% ███████░    tasks: 2   LSP:✓ CodeIdx:⟳ AIWiki:✓
```

**Characteristics:**
- Progress indicator (4/12) in status message
- Spinning animation on ⟳ and CodeIdx
- Token usage higher (67%)
- Multiple active tasks

---

### State 4: Completed Successfully

```
/home/developer/ragent             main ●                 ✓ Refactoring complete
● claude │ tokens: 71% ███████░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Characteristics:**
- Green checkmark (✓)
- Success message
- Task count returned to 0
- Fades to "Ready" after 1-2 seconds

---

### State 5: Error State

```
/home/developer/ragent             main ●                 ✗ Token limit exceeded
✗ claude │ tokens: 100% ██████████ tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Characteristics:**
- Red status message
- Provider health: ✗
- Token bar: Full (red)
- Critical visual alert

---

### State 6: Git Status Variants

#### Clean Repository
```
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

#### Uncommitted Changes
```
/home/developer/ragent             dev ◦                  Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

#### Merge Conflict
```
/home/developer/ragent             main ✗                 ✗ Merge conflict
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

#### Ahead of Remote
```
/home/developer/ragent             dev ↑                  Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

#### Behind Remote
```
/home/developer/ragent             dev ↓                  Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
```

---

## Color-Coded Examples

### Example 1: All Green (Healthy)

```
█████████████████████████████████████████████████████████████████████████
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 30% ███░░░░░░░  tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
█████████████████████████████████████████████████████████████████████████
         [GREEN]            [GREEN] [GREEN]  [GREEN] [GREEN] [GREEN]
```

---

### Example 2: Yellow Warnings

```
█████████████████████████████████████████████████████████████████████████
/home/developer/ragent             dev ◦                  ⟳ Indexing...
◔ claude │ tokens: 78% ████████░░  tasks: 2   LSP:✓ CodeIdx:⟳ AIWiki:✓
█████████████████████████████████████████████████████████████████████████
                      [YELLOW]     [YELLOW]        [YELLOW]
```

---

### Example 3: Red Alerts

```
█████████████████████████████████████████████████████████████████████████
/home/developer/ragent             main ✗                 ✗ Provider unavailable
✗ claude │ tokens: 95% ██████████  tasks: 0   LSP:✗ CodeIdx:✓ AIWiki:✓
█████████████████████████████████████████████████████████████████████████
                  [RED]  [RED]      [RED]     [RED]
```

---

### Example 4: Cyan Infos

```
█████████████████████████████████████████████████████████████████████████
/home/developer/ragent             dev ↑                  Ready
● claude │ tokens: 45% ████░░░░░░  tasks: 0   LSP:✓ CodeIdx:✓ AIWiki:✓
█████████████████████████████████████████████████████████████████████████
                 [CYAN]
```

---

## Animation Frames

### Spinning Indicator Animation

The status message animates during processing with a spinning indicator:

```
Frame 1 (0-45ms):
⟳ Refactoring (4/12)...

Frame 2 (45-90ms):
↻ Refactoring (4/12)...

Frame 3 (90-135ms):
⟲ Refactoring (4/12)...

Frame 4 (135-180ms):
↶ Refactoring (4/12)...

Frame 5 (180-225ms):
⟳ Refactoring (4/12)...  [loop back to Frame 1]
```

**Visual Effect:**
```
⟳ → ↻ → ⟲ → ↶ → ⟳ (repeating at 45ms per frame)
```

---

### Progress Bar Animation

As token usage increases, the bar fills:

```
0% → 10%:   ░░░░░░░░░░
10% → 20%:  █░░░░░░░░░
20% → 30%:  ██░░░░░░░░
30% → 40%:  ███░░░░░░░
40% → 50%:  ████░░░░░░  [Color: Green]
50% → 60%:  █████░░░░░  [Color: Yellow]
60% → 70%:  ██████░░░░  [Color: Yellow]
70% → 80%:  ███████░░░  [Color: Yellow]
80% → 90%:  ████████░░  [Color: Red]
90% → 100%: █████████░  [Color: Red]
100%:       ██████████  [Color: Red - Critical]
```

---

### State Transition Animation

#### Idle → Processing (Fade-in)

```
Time 0ms:    (no animation)
Time 50ms:   ⟳ (fading in)
Time 100ms:  ⟳ (fully visible)
Time 145ms:  ↻ (continues spinning)
```

#### Processing → Complete → Idle (Fade-out)

```
Time 0ms:    ⟳ Executing...    (processing)
Time 1000ms: ✓ Complete        (success message for 1 second)
Time 1300ms: ✓ Complete        (fading out)
Time 1500ms: Ready             (back to idle)
```

---

## Accessibility Variants

### Variant 1: Monochrome (No Colors)

For terminals that don't support colors, use bold and dim:

```
/home/developer/ragent             main [BOLD]●           Ready
[BOLD]● claude │ tokens: 45% [DIM]████░░░░░░[NORMAL]     LSP:[BOLD]✓ CodeIdx:[BOLD]✓
```

**Characteristics:**
- Bold for emphasis
- Dim for secondary info
- Icons still present
- Text always clear

---

### Variant 2: High Contrast

For accessibility, maximum contrast:

```
[BLACK BG / WHITE FG]
/home/developer/ragent             main ●                 Ready
● claude │ tokens: 45% ████░░░░    tasks: 0   LSP:✓ CodeIdx:✓
```

---

### Variant 3: Text Mode (Screen Reader Friendly)

When text-mode is enabled (`/statusbar text-mode`):

```
Status: Ready
Directory: /home/developer/ragent
Branch: main, Status: clean
Provider: anthropic/claude-sonnet-4, Health: healthy
Tokens: 2450 / 200000 (1.2%)
Tasks: 0
LSP: connected (3 servers)
CodeIndex: ready
AIWiki: enabled
```

---

### Variant 4: Colorblind-Safe

Using patterns and symbols instead of relying on color:

```
/home/developer/ragent             main ●●●              Ready
● claude │ tokens: 45% ███░░░░░░░  tasks: 0   LSP:✓✓✓ CodeIdx:✓✓✓ AIWiki:✓
```

**Symbols indicate status:**
- Single ●: Basic status
- Multiple ●●●: Active/important
- ✓: Enabled/healthy
- ✗: Disabled/error

---

## Design Review Checklist

### Visual Hierarchy
- [ ] Primary information (directory, branch, status) is visually distinct
- [ ] Secondary information (provider, tokens, services) is appropriately subordinate
- [ ] Color coding follows semantic meaning
- [ ] Text is consistently readable

### Consistency
- [ ] Column widths are consistent across all layouts
- [ ] Separators are uniform (│)
- [ ] Spacing is consistent
- [ ] Alignment rules followed

### Clarity
- [ ] Information is grouped logically
- [ ] Section purposes are clear
- [ ] Status is immediately obvious
- [ ] Error states stand out

### Responsiveness
- [ ] Full layout uses terminal width efficiently
- [ ] Compact layout remains readable
- [ ] Minimal layout preserves critical info
- [ ] No information truncation (only abbreviation)

### Aesthetics
- [ ] Professional appearance
- [ ] Clean and minimal (reduced clutter)
- [ ] Visually balanced
- [ ] Colors are pleasing and not jarring

### Accessibility
- [ ] Colorblind-friendly (patterns + color)
- [ ] High contrast when needed
- [ ] Text alternatives available
- [ ] Screen reader compatible

---

## Implementation Notes

### For Developers

These mockups show the **target final result** for each scenario. Implementation should:

1. Parse the mockup layout into a data structure
2. Implement responsive width detection
3. Apply color codes based on terminal capabilities
4. Animate transitions smoothly
5. Handle edge cases gracefully

### For Testing

Create test cases for:
- Each responsive breakpoint
- Each state variation
- Color rendering on various terminal types
- Animation smoothness and timing
- Edge cases (very long paths, special characters, etc.)

---

## Sign-Off

This mockup document represents the approved visual design for the status bar redesign.

| Role | Name | Approval | Date |
|------|------|----------|------|
| TUI Lead | [Pending] | [ ] | [TBD] |
| Design Lead | [Pending] | [ ] | [TBD] |
| Product Manager | [Pending] | [ ] | [TBD] |

---

**End of Mockups Document**

