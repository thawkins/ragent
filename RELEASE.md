# Release

## Current Version: 0.1.0-alpha.10

### Added (since 0.1.0-alpha.7)
- Step numbers `[#N]` in both message and log panels for tool call cross-referencing
- Pretty-printed JSON for tool call parameters in log panel
- Restored session tool calls now appear in log panel with status icon and `(restored)` suffix
- Event bus lag warning logged when broadcast events are dropped

### Changed (since 0.1.0-alpha.7)
- Message/log panel split: 60/40 (was 70/30)
- Event bus capacity: 2048 (was 256)
- TUI event loop drains all pending events per cycle via `try_recv()`
- Log panel scroll calculation uses wrapped line count for correct auto-scroll

### Fixed (since 0.1.0-alpha.7)
- Log panel missing entries due to incorrect scroll calculation with wrapped lines
- Silent event bus lag dropping tool call log entries during bursts
- Resumed sessions not populating log panel with restored tool calls
- 148 build warnings resolved (missing docs, unused variables, dead code)
