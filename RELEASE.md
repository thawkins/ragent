# Release

## Current Version: 0.1.0-beta.4

### Added — Telemetry panel styling and release tooling

- Telemetry metric type labels now render in bold blue for better visual
  distinction in the ALT-O Telemetry panel.
- Automated release skill increments workspace version, updates release notes,
  and tags the repository.

### Previous Version: 0.1.0-beta.3

### Added — Context-window compaction and `/config save`

- New context-window compaction pipeline replacing the Headroom-based compression
  scheme. Includes `compaction` config block, `/compact` slash command (with
  `/compress` alias), `CompactionStarted/CompactionFinished` events, and
  Unicode-safe truncation.
- `/config save` and `/config list` slash commands for backing up and restoring
  global `ragent.json`.
- Updates to telemetry counters and TUI wiring.

### Removed — Headroom dependency, CCR store, and compression pipeline

- Dropped the `headroom-core` git dependency, deleted the `compression` modules,
  removed CCR markers and the `headroom_retrieve` bridge, and added a legacy
  `compression` → `compaction` config alias.

- Workspace version bumped from `0.1.0-beta.2` to `0.1.0-beta.3`.
- `cargo check` passes cleanly with the new version.

## Previous Version: 0.1.0-beta.2

### Added — Telemetry (OTEL) and ALT-O Telemetry panel

- OpenTelemetry metrics export (`/telemetry` slash command family: `help`, `on`, `off`, `setup`, `counters`) for managing OTLP endpoints, protocol, export interval, timeout, and an internal Prometheus port.
- TUI **ALT-O Telemetry panel** for live OpenTelemetry metrics and counter inspection.
- Configuration schema and TUI wiring for telemetry settings in `ragent.json`.

## Previous Version: 0.1.0-beta.1

### Changed — Transition to beta channel

- Workspace version bumped from `0.1.0-alpha.147` to `0.1.0-beta.1`, marking
  the transition from the alpha pre-release channel to the beta pre-release
  channel.
