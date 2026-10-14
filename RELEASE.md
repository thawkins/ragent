# Release Notes

## v1.0.134 — MCP discovery ergonomics, ask_user timeout, security hardening

### Added

- **`/mcp config` discovery-location report.** Shows every place ragent looks for
  MCP server definitions (the ragent-native `~/.config/ragent/servers/` directory
  first, then npm global `node_modules`, `PATH`, and the Claude/Cline/legacy/XDG
  registry directories), the number of server definitions found at each location,
  and a table of the individual servers discovered (id, origin, launch command).
  Backed by the new `McpDiscoveryLocation` type and `McpClient::discover_locations`.
- **`ask_user` default/fallback option with an inactivity timeout.** The
  interactive question tool can now carry a pre-selected default that is used
  automatically if the dialog times out.
- **`/mcp discover` block-cursor panel with per-server enable toggle.** The
  discovery dialog lists every server under a block cursor (`Up`/`Down`, `k`/`j`,
  `Home`/`End`), scrolls with a scrollbar, and toggles a server's enabled state on
  `Enter` (writing a `mcp.<id>` entry to the project `.ragent/ragent.json` and the
  global enable ledger, connecting/disconnecting live).

### Security

- **Value-level redaction extended to remaining leak sites.** The server `GET
  /config` redactor and the TUI `/config show` renderer value-scan nested
  `mcp.*.env`/`headers` subtrees; the TUI key-name allowlist also masks `*_token`
  / `*_secret` keys; tool-call argument parse-failure logs redact `args`.
  `McpServerConfig`, `ProviderConfig` and `ApiConfig` carry hand-written redacting
  `Debug` impls.
- **Ad-hoc credential env reads routed through the canonical helper.** Provider
  base-URL/email reads, the OpenAlex mailto resolver, and the `RAGENT_TOKEN`
  fallback now use `ragent_config::credential_env::read_credential_env`.

### Fixed

- **`Config::load` fast-path cache now keys on `XDG_CONFIG_HOME`.**
- **MCP enable ledger writes atomically** (temp file + rename).
- **Best-effort persistence failures are logged, not swallowed.**
- **A session's working directory is always an allowed path root.**
- **A whole-repo plugin source whose only manifest is nested is now discovered.**

### Changed

- **`/simplify all` pass over the uncommitted working tree** (code-audit
  cleanups: dead parameters/branches removed, hot-path tightening, TUI
  de-duplication, and a panic removed from the question-expiry poll).

## v0.1.0-beta.25 — disable rpm packages

### Changed — CI package builds

- Disabled `.rpm` package builds in the release workflow; only the plain binary
  is now published. Both Debian and RPM packaging are temporarily disabled
  while the packaging build paths are reviewed.

## v0.1.0-beta.24 — disable debian builds

### Changed — CI package builds

- Disabled `.deb` package builds in the release workflow; the `.rpm` package and
  plain binary remain published. Debian packaging is temporarily disabled while
  the `cargo-deb` build path is reviewed.

## v0.1.0-beta.23 — CI package builds

### Added — Release workflow builds `.deb` and `.rpm` packages

- Added `.github/workflows/release.yml` that triggers on `v*` tags and builds
  `ragent` for `x86_64-unknown-linux-gnu` on `ubuntu-latest`.
- CI installs `cargo-deb` and `cargo-generate-rpm`, then runs `cargo deb` and
  `cargo generate-rpm` against the release binary.
- The release body is populated from the matching section of `CHANGELOG.md`
  (extracted via `awk`) and published with `softprops/action-gh-release@v2`.
- Assets published to the GitHub Release include:
  - `ragent-<version>-x86_64.deb`
  - `ragent-<version>-x86_64.rpm`
  - the plain `ragent` binary
- Root `Cargo.toml` now carries `[package.metadata.deb]` and
  `[package.metadata.generate-rpm]` metadata so the generated packages
  install the binary to `/usr/bin/ragent` and ship `README.md`, `LICENSE`,
  and `CHANGELOG.md` to `/usr/share/doc/ragent/`.

### Changed — OpenTelemetry updated to 0.28

- Bumped `opentelemetry`, `opentelemetry_sdk`, and `opentelemetry-otlp` to
  0.28 across `crates/ragent-telemetry` and adapted to the breaking API
  changes (new `Resource` builder, `PeriodicReader` signature,
  `InMemoryMetricExporter` relocation, `MetricReader` return types).

