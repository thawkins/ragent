# Release

## Current Version: 0.1.0-alpha.104

### Added
- **Amazon Bedrock provider** — Full AWS Bedrock support with SigV4 request signing (no AWS SDK dependency), dual API clients (Anthropic Messages API for Claude models, Converse API for all other models), 9 default models, short alias mapping, ListFoundationModels discovery, and credential resolution chain (env vars → AWS profile INI → session tokens).
- **xAI Grok provider** — New `xai.rs` provider for the xAI Grok API, registered in the default provider registry.
- **Spec implementation commands** — `/spec impl` and `/spec implement` slash commands for spec lifecycle: generates implementation plans, tracks progress against requirements, and runs implementation tasks.

### Changed
- **Copilot provider improvements** — Updated copilot.rs with refinements to the GitHub Copilot provider.
- **HuggingFace provider improvements** — Updated huggingface.rs with refinements.
- **Provider registry** — Added Bedrock and xAI to `create_default_registry()`.
- **Spec module expanded** — New `impl_runner.rs` and `plan_parser.rs` modules in ragent-specs.

## Previous: 0.1.0-alpha.103

### Fixed
- **Bash syntax validation on Windows** — `validate_bash_syntax()` now uses the discovered shell program (`bash` on Unix, Git Bash executable path on Windows) instead of a hardcoded `sh -n -c` that fails on Windows. PowerShell syntax validation is still skipped (PowerShell self-validates at runtime).