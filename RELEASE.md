# Release

## Current Version: 0.1.0-alpha.3

### Added (since 0.1.0-alpha.2)
- AGENTS.md auto-loading on session start for multi-step agents with visible init greeting
- TUI tool call display: capitalized names, relative paths, result summaries with "└" prefix
- INDEX.md document index for root-level markdown files
- `/compact` history reconstruction with proper tool_result pairing

### Fixed (since 0.1.0-alpha.2)
- `/compact` slash command error (missing tool_result blocks in history)
- Read/Write tool line counts now accurate (using full content or metadata)
- Write tool filename display (full args JSON sent to TUI)
- AGENTS.md init exchange isolated from main tool call processing
