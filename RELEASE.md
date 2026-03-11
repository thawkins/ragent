# Release

## Current Version: 0.1.0-alpha.5

### Added (since 0.1.0-alpha.4)
- `create` tool — create/overwrite a file with content, creating parent directories as needed (22 tools total)
- Slash command output headers — `From: /<command>` prefix on all slash command output
- Each slash command output appears as a separate message block with its own indicator

### Fixed (since 0.1.0-alpha.4)
- Slash command output truncation — scroll calculation now accounts for word-wrapped lines via `Paragraph::line_count()`
- Slash command viewport not scrolling to bottom — `scroll_offset` resets on every slash command
- `ratatui` `unstable-rendered-line-info` feature enabled for accurate line measurement
