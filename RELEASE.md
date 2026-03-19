# Release

## Current Version: 0.1.0-alpha.19

### Added (since 0.1.0-alpha.14)
- **Image attachment support (Alt+V)** — paste images from clipboard or file URIs to include screenshots and visuals in conversations with the LLM
  - New `MessagePart::Image` variant stores image path and MIME type
  - Clipboard raw image data (RGBA pixels) encoded as PNG and saved to temp file
  - File URIs (`file:///...`) with image extensions are recognized and staged
  - Pending attachments displayed in input widget before sending
- **Keybindings help panel (`?`)** — type `?` on empty input to view all keyboard shortcuts
- **Right-click context menu** — Cut/Copy/Paste context menu for input and message panels
- **Context-window utilisation display** — status bar shows percentage of context window used (Copilot provider)
- **Session-prefixed step numbers** — tool call logs now show `[sid:step]` format for easier cross-session correlation

### Changed (since 0.1.0-alpha.14)
- Provider layer extended with `supports_images()` capability check
- Anthropic provider supports image content blocks in message assembly
- Copilot provider supports vision-capable models with base64 image URLs
- Step map now stores `(short_session_id, step_number)` tuples

### Carried from 0.1.0-alpha.14
- MCP server auto-discovery — `/mcp discover` command
- TUI MCP discovery panel (F9)
- LSP test prompts — 5 test prompts for LSP server integration testing
- Office 365 test prompts — 5 test prompts for Office document read/write testing
- LSP workspace folders support
- OpenSkills support for extended skill file formats
- Output file support for `/simplify` skill
- Skills system fully implemented (SPEC §3.19) — 10 phases complete
- 781+ tests passing
