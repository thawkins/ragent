# /yolo

> Toggle YOLO mode: skip all permission prompts (/yolo on|off|status)

## Overview

`/yolo` toggles YOLO mode, which disables the permission system for trusted
environments. The state is persisted, so the mode survives TUI restarts until
you toggle it off again. YOLO mode bypasses far more checks than the on-screen
enable message lists; the full bypass surface is documented below.

## Syntax

```
/yolo            Toggle YOLO mode on or off
/yolo on         Enable YOLO mode
/yolo off        Disable YOLO mode
/yolo status     Show whether YOLO mode is enabled
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/yolo` | Toggles the current YOLO state |
| `/yolo on` | Enables YOLO mode |
| `/yolo off` | Disables YOLO mode |
| `/yolo status` | Prints the current state |

The flag is stored as a persistent runtime flag (`config.yolo`), so it
persists across restarts.

## Bypass surface

When YOLO mode is enabled the following checks are skipped:

- Banned bash commands from the 7-layer bash security whitelist
- Denied command names in the bash security layer
- User deny patterns configured through `/bash add deny`
- Obfuscation detection for shell commands
- Dynamic context and skill allowlist validation
- MCP configuration validation
- Interactive permission prompts for all tools (auto-allow), except forced
  destructive-action checkpoints, which still require confirmation
- The default block on packet-capture tools (`tcpdump`, `wireshark`)

The enable message printed by the TUI lists only a subset of these items, so
treat the list above as the authoritative bypass surface.

## Examples

```
/yolo on
```
Enables YOLO mode; all subsequent tool calls run without permission prompts
(aside from forced destructive-action checkpoints).

```
/yolo
```
Toggles the current state; if YOLO was off it is now on, and vice versa.

```
/yolo status
```
Prints whether YOLO mode is currently enabled.

```
/yolo off
```
Disables YOLO mode and restores normal permission gating.

## Output

- Toggling prints a confirmation message with the new state. The enable
  message enumerates a subset of the bypassed checks only.
- `status` prints the current on/off state.

## Related

- `/autopilot` - autonomous runs with permission auto-approval for the run duration
- `/bash` - the bash tool whose security layers are relaxed by YOLO mode
- `/mcp` - MCP server management, whose validation is skipped under YOLO
- Permission rules in `ragent.json` - allow/deny rules are bypassed while YOLO is on