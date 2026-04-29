# TUI Keybindings Reference

**Document:** docs/tui-keybindings.md  
**Last Updated:** 2025-01  
**Scope:** Complete keyboard shortcut reference for ragent TUI

---

## Overview

This document provides a comprehensive reference of all keyboard shortcuts available in the ragent TUI interface. Shortcuts are organized by context and follow standardized patterns:

- **Navigation:** `↑/↓` primary, `j/k` alternatives in list contexts
- **Dismiss/Close:** `Esc` primary, `q` alternative in non-input contexts
- **Confirmation:** `Enter` always confirms, `Space` toggles/checks in multi-select

---

## Global Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Ctrl+C` | Arm quit / Copy | First press arms quit, copies if text selected |
| `Ctrl+D` | Confirm quit | Confirm quit after Ctrl+C |
| `Tab` / `Shift+Tab` | Switch agent | Cycle to next/previous agent |
| `?` | Show shortcuts | Display keyboard shortcuts help panel |
| `Alt+L` | Toggle log | Show/hide log panel |

---

## Input Area Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Enter` | Send message | Submit input as user message |
| `Shift+Enter` / `Alt+Enter` | New line | Insert literal newline |
| `↑` | History up | Recall previous input from history |
| `↓` | History down | Recall next input from history |
| `←` / `→` | Move cursor | Move cursor left/right |
| `Ctrl+←` / `Ctrl+→` | Word jump | Move cursor by word |
| `Home` / `End` | Line jump | Move to start/end of line |
| `Ctrl+A` | Select all | Select all input text |
| `Ctrl+E` | End of line | Move cursor to end |
| `Ctrl+B` / `Ctrl+F` | Char move | Move backward/forward (emacs style) |
| `Ctrl+W` | Delete word | Delete previous word |
| `Ctrl+K` | Delete to end | Delete from cursor to end of line |
| `Ctrl+X` | Cut | Cut selection to clipboard |
| `Ctrl+V` | Paste | Paste from clipboard |
| `Alt+V` | Paste image | Paste image as staged attachment |
| `Delete` | Delete char | Delete character under cursor |
| `Backspace` | Delete prev | Delete previous character |
| `Shift+←` / `Shift+→` | Select char | Extend selection by character |
| `Ctrl+Shift+←` / `Ctrl+Shift+→` | Select word | Extend selection by word |

---

## Message View Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Shift+↑` / `Shift+↓` | Scroll | Scroll message view up/down |
| `PageUp` / `PageDown` | Page scroll | Scroll by page |

---

## Log Panel Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Ctrl+PageUp` / `Ctrl+PageDown` | Log scroll | Scroll log panel up/down |

---

## Output View Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `PageUp` / `PageDown` | Page scroll | Scroll output view |
| `Ctrl+PageUp` / `Ctrl+PageDown` | Jump to edge | Jump to start/end of output |
| `Alt+↑` / `Alt+↓` | Focus teammate | Cycle focus between teammates |
| `Esc` | Close view | Close output view |

---

## Slash Menu Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate command list |
| `Enter` | Select | Select highlighted command |
| `Esc` | Cancel | Close slash menu, clear input |
| `Tab` | Complete | Accept file menu selection (when visible) |

---

## File Menu Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate file list |
| `Tab` | Accept | Accept selection (navigate into directory) |
| `Enter` | Accept | Accept file selection |
| `Esc` | Cancel | Close file menu |
| `Ctrl+\` | Toggle hidden | Show/hide hidden files |

---

## Overlay Navigation Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `k` | Up | Move selection up |
| `↓` / `j` | Down | Move selection down |
| `Enter` | Toggle | Expand/collapse selected entry |
| `PageUp` / `PageDown` | Page scroll | Scroll by 5 entries |
| `Esc` | Close | Close the active overlay or modal |

---

## Memory Browser Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Esc` | Close | Close memory browser |

---

## Permission Dialog Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `y` | Yes | Grant permission once |
| `a` | Always | Grant permission always |
| `n` | No | Deny permission |
| `Enter` | Submit | Submit question response |
| `Esc` | Cancel | Cancel question (sends dismissal) |
| `Backspace` | Delete | Delete character in question input |

---

## Plan Approval Dialog Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `←` / `→` | Toggle | Move cursor between Approve/Reject |
| `Enter` | Confirm | Confirm selected action |
| `r` | Reject | Reject plan |
| `Esc` | Cancel | Reject plan (alternative) |

---

## Force Cleanup Modal Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Enter` | Confirm | Confirm force cleanup |
| `Esc` | Cancel | Cancel force cleanup |

---

## Provider Setup Dialog Shortcuts

### Provider Selection
| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate provider list |
| `Enter` | Select | Select provider |
| `Esc` | Cancel | Close dialog |

### API Key Entry
| Shortcut | Action | Description |
|----------|--------|-------------|
| `Ctrl+V` | Paste | Paste from clipboard |
| `Tab` | Toggle field | Toggle between key and endpoint (Generic OpenAI) |
| `Enter` | Submit | Submit credentials |
| `←` / `→` | Move cursor | Move cursor left/right |
| `Home` / `End` | Jump | Jump to start/end |
| `Backspace` | Delete | Delete previous character |
| `Delete` | Delete | Delete character under cursor |

### Model Selection
| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate model list |
| `Enter` | Select | Select model |

### Device Flow
| Shortcut | Action | Description |
|----------|--------|-------------|
| `c` | Copy | Copy device code to clipboard |
| `Esc` | Cancel | Cancel device flow |

### Agent Selection
| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate agent list |
| `Enter` | Select | Select agent |

---

## MCP Discover Dialog Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate server list |
| `Enter` | Toggle | Toggle server selection |
| `s` | Select all | Select all servers |
| `c` | Clear | Clear all selections |
| `a` | Accept | Accept selected servers |
| `Esc` | Cancel | Close dialog |

---

## Context Menu Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `↑` / `↓` | Navigate | Navigate menu items |
| `Enter` | Select | Execute selected action |

---

## Processing State Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Esc` | Cancel | Cancel running agent |

---

## Navigation Standards

### Arrow Keys (Primary)
All navigation uses `↑/↓/←/→` as the primary method:
- Lists: `↑/↓` to navigate items
- Horizontal: `←/→` to move cursor or toggle options

### Vi-style Alternatives (Deprecated)
~~Some list contexts support `j/k` as alternatives~~ - **Removed** to ensure consistent navigation experience across all contexts.

### Dismissal Pattern
- **Primary:** `Esc` closes/dismisses all dialogs and modals
- **Question dialogs:** `Esc` sends dismissal message
- **Note:** `q` key dismissal is only available in specific read-only contexts (e.g., history picker)

### Confirmation Pattern
- **Primary:** `Enter` confirms selection or action
- **Toggle:** `Space` (reserved for multi-select contexts)
- **Single-key shortcuts:** `y/a/n` for permission dialogs

---

## Known Inconsistencies

All identified inconsistencies have been resolved:

| Location | Issue | Status |
|----------|-------|--------|
| Journal viewer | Used `j/k` alternatives | ✅ Fixed - now uses `↑/↓` only |
| History picker | Used `j/k` alternatives | ✅ Fixed - now uses `↑/↓` only |
| Shortcuts panel | Used `?` to dismiss | ✅ Documented - `Esc` primary, `?` alternative |

**Resolution Date:** 2025-01

---

## Implementation Notes

### File Locations
- **Main handler:** `crates/ragent-tui/src/input.rs`
- **Dialog widgets:** `crates/ragent-tui/src/widgets/`
- **State management:** `crates/ragent-tui/src/app/state.rs`

### Adding New Shortcuts
When adding new keyboard shortcuts:
1. Follow existing patterns in `handle_key()` function
2. Document in this file
3. Update shortcuts panel rendering if applicable
4. Ensure consistency with existing navigation patterns

---

## Changelog

| Date | Change |
|------|--------|
| 2025-01 | Initial documentation created |
| 2025-01 | Fixed navigation inconsistencies (j/k → ↑/↓) |
| 2025-01 | Standardized confirmation patterns |
