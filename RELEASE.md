# Release

## Current Version: 0.1.0-alpha.6

### Added (since 0.1.0-alpha.5)
- TUI display summaries for `office_read`, `office_write`, `office_info`, `pdf_read`, `pdf_write` tools

### Fixed (since 0.1.0-alpha.5)
- Panic in text selection on multi-byte UTF-8 characters (●) — byte offsets now snap to char boundaries
- Office and PDF tools now show file path and line count in messages panel
