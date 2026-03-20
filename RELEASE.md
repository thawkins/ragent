# Release

## Current Version: 0.1.0-alpha.20

### Added
- Input changes improvements
- New OpenAI generic provider support

### Carried from 0.1.0-alpha.19
- Teams UX and lifecycle enhancements:
  - Added `/team open <name>`, `/team close`, `/team delete <name>`, and `/team clear`
  - Updated `/team tasks` to render a tabular task/status view
  - Improved team-session reliability with TeamManager lazy initialization in TUI team flows
  - Fixed slash-input cursor behavior for `/team ...` entry
- Context management improvements:
  - Added automatic pre-send context compaction near context-window limits with queued message replay
- Copilot provider enhancements:
  - Added reasoning level selection support (`low`, `medium`, `high`, `none`)
  - Added model request-cost multiplier display in model selector
  - Improved model compatibility filtering for chat-completions endpoint usage
- Stability and docs:
  - Added read tool line-range validation to prevent runtime panics
  - Added `docs/howto_teams.md` comprehensive Teams user manual
  - Expanded tests around teams slash commands and task rendering
