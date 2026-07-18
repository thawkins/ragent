# Release

## Current Version: 0.1.0-beta.5

### Added — Live telemetry reconfiguration, agent metric recording, and sudo askpass broker

- `/telemetry on|off` now reconfigures the live `TelemetrySubsystem` in place
  (shuts down the meter provider on `off`, builds a fresh one on `on`) so the
  toggle takes effect immediately instead of requiring a restart. The
  subsystem's runtime state is held behind a `parking_lot::Mutex` and the
  provider wrapped in `Arc` for safe interior mutability.
- New `ragent-agent` telemetry module (`LlmRecorder`, `SessionRecorder`,
  `ToolRecorder`) records LLM call duration, tool invocation counts/durations,
  session start/end, and agent-loop timing into the telemetry subsystem.
  `SessionProcessor` is wired to the subsystem via an `Arc<TelemetrySubsystem>`.
- New `askpass` module in `ragent-tools-core` routes `sudo` password prompts
  through ragent's interactive question dialog instead of hanging on the
  controlling tty. The bash tool now detaches stdin (`Stdio::null()`) and sets
  `SUDO_ASKPASS` environment variables when a broker is active.
- `ragent-telemetry` re-exports `LlmRecorder`, `SessionRecorder`, and
  `ToolRecorder` for cross-crate use.
- `ShutdownGuard` keeps the meter provider alive for the process lifetime and
  flushes pending metrics on normal or panic exit paths.
- Telemetry panel rendering and `/telemetry` slash-command code reformatted
  (indentation and trailing-newline fixes).

### Previous Version: 0.1.0-beta.4

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
