# Status Bar Visual Design Specification

**Document:** STATUSBAR_VISUAL_SPEC.md  
**Version:** 1.0  
**Status:** Final  
**Created:** 2025-01-16  
**Phase:** 1 (Design & Planning)

---

## Table of Contents

1. [Overview](#overview)
2. [Visual Design Specifications](#visual-design-specifications)
3. [Color Palette](#color-palette)
4. [Typography & Spacing](#typography--spacing)
5. [Indicators & Icons](#indicators--icons)
6. [Animation Specifications](#animation-specifications)
7. [Accessibility Guidelines](#accessibility-guidelines)
8. [Responsive Breakpoints](#responsive-breakpoints)
9. [Visual Examples](#visual-examples)
10. [Component Specifications](#component-specifications)

---

## Overview

This document provides detailed visual specifications for the redesigned status bar, including exact measurements, colors, spacing, indicators, and animations.

### Design Principles

1. **Hierarchy** — Clear visual priority between sections
2. **Consistency** — Uniform styling across all components
3. **Clarity** — Easy-to-understand information grouping
4. **Minimalism** — Remove visual noise, keep essential info
5. **Responsiveness** — Adapt gracefully to terminal size changes
6. **Accessibility** — Support for colorblind users and assistive tech

---

## Visual Design Specifications

### Line 1: Context & Status

#### Layout Structure
```
┌──────────────────────────┬────────────────────┬───────────────────────────┐
│  Section 1: Working Dir  │ Section 2: Git     │  Section 3: Status Msg    │
│      (Left)              │    (Center)        │      (Right)              │
├──────────────────────────┼────────────────────┼───────────────────────────┤
│ /home/user/project       │ main ●             │ Ready                     │
│ ~25 chars (truncated)    │ ~12-15 chars       │ Flexible, right-aligned   │
└──────────────────────────┴────────────────────┴───────────────────────────┘
```

#### Column 1: Working Directory
- **Width**: 25 characters (fixed width)
- **Alignment**: Left-aligned, padded right
- **Separator**: Light gray column divider (│) after
- **Truncation**: Middle truncation with ellipsis (…)
  - Example: `/home/user/proj…/src` (for paths > 25 chars)
- **Home indicator**: `~` for `/home/username` paths
- **Read-only indicator**: ⚠ suffix for read-only directories

**Examples:**
```
/home/user/project       →   /home/user/project
/home/username/very/long →   /home/user…/long
~/.config/ragent         →   ~/.config/ragent
/var/log (read-only)     →   /var/log ⚠
```

#### Column 2: Git Branch & Status
- **Width**: 15 characters (flexible, min 12)
- **Alignment**: Center-aligned with padding
- **Format**: `<branch-name> <status-indicator>`
- **Branch name**: Green color, max 10 chars
- **Status indicator**: Single character with color
  - `●` (●) = Clean/ready (Green, `#00FF00`)
  - `◦` (◦) = Uncommitted changes (Yellow, `#FFFF00`)
  - `✗` (✗) = Merge conflict (Red, `#FF0000`)
  - `↓` (↓) = Behind remote (Cyan, `#00FFFF`)
  - `↑` (↑) = Ahead of remote (Cyan, `#00FFFF`)
- **Fallback**: Just branch name if git not available

**Examples:**
```
main ●               →   Clean, tracking upstream
feature/auth ◦       →   Has uncommitted changes
develop ↑            →   Commits not pushed
merge-pr ✗           →   Merge conflict
(no git) ─           →   Git not initialized
```

#### Column 3: Session Status Message
- **Width**: Remaining space (flexible)
- **Alignment**: Right-aligned
- **Text color**: White (#FFFFFF) default
- **Status colors**: 
  - Green for "Ready"
  - Yellow for "Processing…", "Working…"
  - Red for errors
  - Cyan for special states

**Examples:**
```
Ready                                →   Idle, no activity
⟳ Executing...                       →   Tool running
⟳ Refactoring (3/12)...              →   Showing progress
⟳ Indexing...                        →   Long operation
✓ Task completed                     →   Success
✗ Error: token limit exceeded        →   Error state
```

---

### Line 2: System & Resources

#### Layout Structure
```
┌──────────────────────────┬────────────────────┬───────────────────────────┐
│ Section 1: LLM Provider  │ Section 2: Resource│  Section 3: Services      │
│      (Left)              │    (Center)        │      (Right)              │
├──────────────────────────┼────────────────────┼───────────────────────────┤
│ ● claude │ 4K tokens     │ tasks: 1, mem: 2M  │ LSP:✓ CodeIdx:✓ AIWiki:✓ │
│ ~25 chars (fixed)        │ ~25 chars (fixed)  │ Flexible, right-aligned   │
└──────────────────────────┴────────────────────┴───────────────────────────┘
```

#### Column 1: LLM Provider
- **Width**: 25 characters (fixed width)
- **Format**: `<health-dot> <provider> │ <context-window>`
- **Health indicator**: Single character
  - `●` (●) = Healthy (Green, `#00FF00`)
  - `◔` (◔) = Slow/degraded (Yellow, `#FFFF00`)
  - `✗` (✗) = Error/unreachable (Red, `#FF0000`)
  - `◎` (◎) = Connecting (Cyan, `#00FFFF`)
- **Provider name**: Max 10 chars, shortened
  - `claude` (Anthropic)
  - `gpt-4o` (OpenAI)
  - `gemini` (Google)
  - `llama` (Ollama)
- **Context window**: Humanized format
  - `4K` (4,096 tokens)
  - `128K` (128,000 tokens)
  - `200K` (200,000+ tokens)
- **Separator**: Light gray │ after provider

**Examples:**
```
● claude │ 200K tokens      →   Healthy, Claude, 200K context
◔ gpt-4o │ 128K tokens      →   Slow, GPT-4o
✗ llama │ --               →   Error, unavailable
```

#### Column 2: Resources & Metrics
- **Width**: 25 characters (fixed width)
- **Primary metric**: Token usage (always shown)
  - Format: `tokens: XX% <bar>` or `tokens: X.XK/X.XK`
  - Bar: `████░░░░░░` (10 chars, 10% each)
  - Color bar:
    - Green (`#00FF00`) for 0-50%
    - Yellow (`#FFFF00`) for 50-80%
    - Red (`#FF0000`) for 80-100%
  - Percentage: `12%`, `45%`, `87%`
- **Secondary metrics** (space-permitting):
  - `tasks: 1` (if > 0, else hidden)
  - `mem: 128M` (if > 100MB, else hidden)
- **Spacing**: Metrics separated by commas and spaces

**Examples:**
```
tokens: 45% ████░░░░░░      →   45% token usage
tokens: 89% ██████████      →   Critical (red bar)
tokens: 2.4K/8K (45%)        →   Detailed format
tasks: 1, mem: 256M          →   With secondary metrics
```

#### Column 3: Service Status
- **Width**: Remaining space (flexible, min 30 chars)
- **Format**: Compact indicators `<service>:<status>`
- **Indicators**:
  - `LSP:✓` (Green if connected/active, Red if error)
  - `CodeIdx:✓` (Green if ready, Yellow if indexing)
  - `AIWiki:✓` (Green if enabled, Red if disabled, Cyan if syncing)
- **Separator**: Spaces between indicators
- **Optional**: Log count shown only if verbose or on demand
  - `Log: 234` (shown in verbose mode)

**Examples:**
```
LSP:✓ CodeIdx:✓ AIWiki:✓     →   All services healthy
LSP:✗ CodeIdx:✓ AIWiki:✓     →   LSP error
LSP:✓ CodeIdx:⟳ AIWiki:✓     →   CodeIdx indexing
LSP:✓ CodeIdx:✓ AIWiki:✓ Log:234  →   Verbose mode
```

---

## Color Palette

### Semantic Colors

| Color | Hex Code | RGB | Purpose | Usage |
|-------|----------|-----|---------|-------|
| **Green** | `#00FF00` | 0,255,0 | Ready, healthy, enabled | ●, ✓, clean status |
| **Yellow** | `#FFFF00` | 255,255,0 | Warning, processing | ◔, changes, indexing |
| **Red** | `#FF0000` | 255,0,0 | Error, failed, disabled | ✗, conflict, error |
| **Cyan** | `#00FFFF` | 0,255,255 | In progress, changed, info | ↑↓, syncing, updates |
| **White** | `#FFFFFF` | 255,255,255 | Text, default | All text content |
| **DarkGray** | `#808080` | 128,128,128 | Labels, separators | │, labels, secondary text |
| **Black** | `#000000` | 0,0,0 | Background | Terminal background |

### Color Application Rules

#### Status Indicators (Always Colored)
- Health dots: Always use semantic color
- Checkmarks/X marks: Always use semantic color
- Direction arrows: Always use semantic color

#### Text Content (Generally White)
- Labels: DarkGray for secondary importance
- Values: White for primary importance
- Messages: Color-coded by severity

#### Progressive Filling (Token Bar)
```
0-50%:   ████░░░░░░  (Green)
50-80%:  ████████░░  (Yellow)
80-100%: ██████████  (Red)
```

---

## Typography & Spacing

### Font Settings

| Setting | Value | Notes |
|---------|-------|-------|
| **Font Family** | Monospace | Standard terminal font |
| **Font Size** | Terminal default | Don't override |
| **Line Height** | 1.0 | No extra spacing |
| **Weight** | Regular | Normal terminal weight |

### Spacing Rules

#### Horizontal Spacing
- **Column divider**: 1 space before + 1 space after (`│`)
- **Element spacing**: 1 space between elements
- **Padding left**: 1 space at start of line
- **Padding right**: 1 space at end of line

**Example spacing:**
```
[1sp] /home/user/project [1sp] │ [1sp] main ● [1sp] │ [1sp] Ready [1sp]
```

#### Character Widths (Monospace)
- Each character = 1 unit width
- Each space = 1 unit width
- Special chars (●, ✓, ✗) = 1 unit width (even if multi-byte UTF-8)

### Text Alignment

| Section | Alignment | Method |
|---------|-----------|--------|
| Working Dir | Left | Pad right with spaces |
| Branch | Center | Pad left and right |
| Status Msg | Right | Pad left with spaces |
| Provider | Left | Pad right with spaces |
| Resources | Left | Pad right with spaces |
| Services | Right | Pad left with spaces |

---

## Indicators & Icons

### Status Indicator Symbols

| Symbol | Unicode | Name | Colors | Usage |
|--------|---------|------|--------|-------|
| `●` | U+25CF | Black Circle | Green/Yellow/Red/Cyan | Health/status |
| `◔` | U+25D4 | Circle with Right Half Black | Yellow | Warning/slow |
| `✗` | U+2717 | Ballot X | Red | Error/disabled |
| `✓` | U+2713 | Check Mark | Green | Success/enabled |
| `◦` | U+25E6 | White Bullet | Yellow | Changes |
| `↑` | U+2191 | Upwards Arrow | Cyan | Ahead |
| `↓` | U+2193 | Downwards Arrow | Cyan | Behind |
| `⟳` | U+27F3 | Clockwise Gapped Circle Arrow | Cyan | Spinning/processing |
| `⊙` | U+2299 | Circled Dot Operator | Yellow | Alternative spinner |
| `─` | U+2500 | Box Drawings Light Horizontal | DarkGray | Separator |
| `│` | U+2502 | Box Drawings Light Vertical | DarkGray | Column divider |

### Animation Indicators

#### Spinning Indicator
For long-running operations, use spinning animation:
```
⟳ → ↻ → ⟲ → ↶ → ⟳  (repeat)
```

Frame cycle:
1. ⟳ (45ms)
2. ↻ (45ms)
3. ⟲ (45ms)
4. ↶ (45ms)
5. Repeat

Alternative (if Unicode issues):
```
- → \ → | → / → - (repeat, 45ms each)
```

#### Progress Bars

10-character bar for token usage:
```
████░░░░░░  (0%  - 10%)
█████░░░░░  (10% - 20%)
██████░░░░  (20% - 30%)
███████░░░  (30% - 40%)
████████░░  (40% - 50%)
████████░░  (50% - 60%)  ← Color changes Yellow here
█████████░  (60% - 70%)
██████████  (70% - 80%)
██████████  (80% - 90%)
██████████  (90% - 100%) ← Color changes Red here
```

---

## Animation Specifications

### Idle State
```
No animation, static display
- Status message: "Ready"
- All indicators stable
- No visual changes
```

### Processing State
```
Spinning indicator animation
Duration: Continuous until operation complete
Frame rate: 45ms per frame
Animation: ⟳ ↻ ⟲ ↶ (repeating)

Status message: ⟳ <operation name>...
Example: ⟳ Executing...
Example: ⟳ Refactoring (3/12)...
```

### Completed State
```
Instant change with optional fade
Duration: Show for 1-2 seconds, then fade to "Ready"
Display: ✓ <operation result>
Color: Green for success, Red for error
Example: ✓ Task completed
Example: ✗ Error: token limit exceeded
```

### Transitions

#### State Change: Idle → Processing
- Animation: Fade-in spinner (200ms)
- Message change: Instant
- Color change: Instant

#### State Change: Processing → Complete
- Animation: Spinner stop, instant message change
- Result display: 1-2 seconds visible
- Fade to idle: Gradual (300ms)

### Animation Performance
- Frame rate: 45ms per frame (≈22 fps)
- No frame drops allowed
- CPU-efficient spinner implementation
- Stop animation immediately on state change

---

## Accessibility Guidelines

### Color Blindness Support

The design must be usable by people with:
- Deuteranopia (red-green colorblindness)
- Protanopia (red-green colorblindness)
- Tritanopia (blue-yellow colorblindness)
- Monochromia (complete colorblindness)

#### Strategy: Pattern + Color

Use patterns in addition to colors:

| Status | Color + Pattern |
|--------|-----------------|
| Healthy | Green + `●` |
| Warning | Yellow + `◔` |
| Error | Red + `✗` |
| Info | Cyan + `↑/↓` |

#### Text for Critical Info

Always include text labels in addition to colors:
```
✓ Health indicator (text always present)
tokens: 45% (percentage value always shown)
LSP:✓ (service status with symbol and text)
```

### Screen Reader Support

Provide accessible alternative text via:
- ARIA labels (for web/accessible terminals)
- Alt text fallback mode: `/statusbar text-mode`
- Complete status via `/status` command

### Terminal Compatibility

Test with:
- 16-color terminals (fallback colors)
- 256-color terminals (full color palette)
- True color terminals (24-bit RGB)
- Monochrome terminals (bold/dim patterns)

---

## Responsive Breakpoints

### Full Layout (≥120 characters)

```
Terminal width: 120+ chars
Usage: Large monitors, wide terminals

┌──────────────────────────┬────────────────────┬───────────────────────────┐
│ /home/user/project       │ main ●             │ Ready                     │
├──────────────────────────┼────────────────────┼───────────────────────────┤
│ ● claude │ tokens: 45%   │ tasks: 1           │ LSP:✓ CodeIdx:✓ AIWiki:✓ │
│ ████░░░░░░              │ mem: 128M          │ Log: 234                  │
└──────────────────────────┴────────────────────┴───────────────────────────┘

Features:
- Full directory path
- Complete metrics shown
- Service indicators + log count
- Maximum information density
```

### Compact Layout (80-120 characters)

```
Terminal width: 80-120 chars
Usage: Standard terminals, laptop screens

┌─────────────────┬──────────────────┬──────────────────────┐
│ ~/project       │ main ●           │ Ready                │
├─────────────────┼──────────────────┼──────────────────────┤
│ claude          │ tok: 45% ████░░  │ LSP:✓ CodeIdx:✓     │
└─────────────────┴──────────────────┴──────────────────────┘

Features:
- Shortened path with home tilde
- Abbreviated metric labels (tok: instead of tokens:)
- Service indicators only
- Slightly reduced information density
```

### Minimal Layout (<80 characters)

```
Terminal width: <80 chars
Usage: Very small terminals, mobile

┌──────────────┬──────────┬────────────┐
│ ~/proj       │ main ●   │ Ready      │
├──────────────┼──────────┼────────────┤
│ claude       │ tok:45%  │ LSP:✓      │
└──────────────┴──────────┴────────────┘

Features:
- Minimal path
- No progress bar
- Only critical services (LSP)
- Maximum truncation
- Detailed info via /status command
```

---

## Visual Examples

### Example 1: Healthy System

```
/home/developer/ragent         develop ●           Ready
● claude │ tokens: 45% ████░   tasks: 0            LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Interpretation:**
- Working in `/home/developer/ragent` directory
- On `develop` branch, clean (no changes)
- Ready to accept commands
- Claude provider healthy, 45% token budget used
- No active tasks
- All services operational

---

### Example 2: Active Operation

```
/home/developer/ragent         develop ●           ⟳ Refactoring (4/12)...
● claude │ tokens: 62% ███████ tasks: 2          LSP:✓ CodeIdx:⟳ AIWiki:✓
```

**Interpretation:**
- Working in ragent repository
- Currently executing a refactoring task (step 4 of 12)
- Token usage at 62% (yellow bar)
- 2 active background tasks
- Code index is currently re-indexing
- Processing indicated by spinning animation

---

### Example 3: Error State

```
/home/developer/ragent         main ✗              ✗ Merge conflict detected
✗ claude │ tokens: 91% █████   tasks: 1           LSP:✓ CodeIdx:✓ AIWiki:✓
```

**Interpretation:**
- Critical merge conflict on main branch
- Token budget nearly exhausted (91%, red bar)
- Provider error (unreachable or error state)
- Visual alerts with red ✗ symbols

---

### Example 4: Compact Terminal

```
~/proj             main ●             Ready
claude tok:45% ████ tasks:0          LSP:✓ CodeIdx:✓
```

**Interpretation:**
- Same as Example 1 but on 90-char terminal
- Abbreviated labels and paths
- Less visual density

---

### Example 5: Minimal Terminal

```
~/proj        main ●
claude tok:45% LSP:✓
```

**Interpretation:**
- Same information on <80 char terminal
- Absolute minimum for essential info
- User must use `/status` for full details

---

## Component Specifications

### Column Divider
- **Character**: `│` (U+2502, Box Drawings Light Vertical)
- **Color**: DarkGray (`#808080`)
- **Spacing**: 1 space before, 1 space after
- **Usage**: Between all 3 sections per line

### Status Dot
- **Character**: `●` (U+25CF, Black Circle)
- **Size**: 1 character width
- **Colors**: Green/Yellow/Red/Cyan
- **Usage**: Health indicator, git status, provider status

### Check Mark
- **Character**: `✓` (U+2713, Check Mark)
- **Color**: Green (`#00FF00`)
- **Usage**: Service enabled, operation success

### Cross Mark
- **Character**: `✗` (U+2717, Ballot X)
- **Color**: Red (`#FF0000`)
- **Usage**: Service disabled, error state, conflict

### Progress Bar
- **Filled**: `█` (U+2588, Full Block)
- **Empty**: `░` (U+2591, Light Shade)
- **Length**: 10 characters
- **Color**: Dynamic (Green → Yellow → Red)

### Spinner
- **Characters**: `⟳ ↻ ⟲ ↶` (rotational arrows)
- **Color**: Cyan (`#00FFFF`)
- **Frame time**: 45ms each
- **Total cycle**: 180ms

---

## Design Sign-Off

| Role | Name | Status | Date |
|------|------|--------|------|
| TUI Lead | [TBD] | Pending | [TBD] |
| Design Lead | [TBD] | Pending | [TBD] |
| Product Manager | [TBD] | Pending | [TBD] |
| Accessibility Review | [TBD] | Pending | [TBD] |

---

## Document Versioning

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-01-16 | Initial comprehensive specification |
| [Draft] | [TBD] | [Pending feedback] |
| [Final] | [TBD] | [Approved by stakeholders] |

---

**End of Document**

