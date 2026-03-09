# Release

## Current Version: 0.1.0-alpha.2

### Added (since 0.1.0-alpha.1)
- `/provider_reset` slash command with interactive provider selection and persistent disable flag
- Clipboard copy support on Copilot device code screen (Linux-aware via `arboard`)
- Storage methods: `delete_provider_auth()`, `delete_setting()`
- Robust Copilot API base resolution with multi-source token discovery
- VS Code-compatible headers for Copilot chat API

### Fixed (since 0.1.0-alpha.1)
- Copilot "Unknown model" error — device flow token now prioritised over `gh` CLI token
- Copilot API uses plan-specific endpoint (`api.individual.githubcopilot.com`)
- Provider reset persistence across app restarts
