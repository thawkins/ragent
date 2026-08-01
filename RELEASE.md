# Release Notes

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

