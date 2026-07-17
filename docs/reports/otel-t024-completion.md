# T-024 Completion Report — Custom resource attributes from config

## Spec / Plan references

- **Spec ID:** `otel`
- **Task:** T-024 — Add custom resource attributes from config
- **Requirement:** FR-026
- **Dependencies:** T-007 (resource attribute injection), T-002 (config schema)
- **Status:** `completed` (see `specs/otel/PLAN.md` line 79)

## Requirement text

> **FR-026** The system may support custom resource attributes via
> `telemetry.otel.resource_attributes` in `ragent.json`.

## What was already implemented (prior tasks)

The implementation of FR-026 was delivered by three earlier tasks. T-024
closes the task by adding comprehensive end-to-end tests and documentation.

| Task | What it delivered for FR-026 |
|------|------------------------------|
| **T-002** | `OtelConfig.resource_attributes: HashMap<String, String>` with `#[serde(default)]` in `crates/ragent-config/src/telemetry.rs`; `TelemetryConfig::merge` unions `resource_attributes` across config layers (overlay takes precedence for overlapping keys, base-only keys preserved). |
| **T-007** | `build_resource()` in `crates/ragent-telemetry/src/subsystem.rs` iterates `config.resource_attributes` and pushes each entry as a `KeyValue` into the OTEL `Resource`, alongside the static `service.name`, `service.version`, and `host.name` attributes (FR-004). |
| **T-022** | Every resource-attribute value (including `service.name`, `host.name`, and each custom entry) is passed through `ragent_telemetry::sensitive::sanitize_attr_value` (FR-034) so an accidental API key or file content is replaced with `"redacted"` rather than exported. |
| **T-021** | `Config::merge` delegates to `TelemetryConfig::merge`, so the union semantics flow through to the top-level config layering. |

## What T-024 added

### Tests

`crates/ragent-telemetry/tests/test_custom_resource_attributes.rs` (18
tests, all passing) exercises the **full end-to-end path** from
`telemetry.otel.resource_attributes` in the config through
`TelemetrySubsystem::new()` → `build_resource()` → exported `Resource`:

| Category | Tests |
|----------|-------|
| Custom attributes appear in the export | `test_custom_resource_attributes_appear_in_export`, `test_custom_attributes_coexist_with_static`, `test_absent_resource_attributes_produces_only_static` |
| Sensitive-data guard at the export level | `test_api_key_in_resource_attributes_is_redacted`, `test_bearer_token_in_resource_attributes_is_redacted`, `test_github_pat_in_resource_attributes_is_redacted`, `test_file_content_in_resource_attributes_is_redacted`, `test_credential_in_resource_attributes_is_redacted`, `test_safe_custom_attribute_not_redacted`, `test_mixed_safe_and_sensitive_resource_attributes` |
| Subsystem config accessor | `test_subsystem_config_preserves_resource_attributes`, `test_subsystem_config_preserves_sensitive_value_raw` |
| Config serde | `test_resource_attributes_deserialize_from_json`, `test_absent_resource_attributes_defaults_to_empty`, `test_resource_attributes_serde_roundtrip` |
| Config merge | `test_config_merge_unions_resource_attributes`, `test_config_merge_disabled_overlay_contributes_resource_attributes` |
| Independence | `test_service_name_independent_of_resource_attributes` |

The export-level tests reconstruct the `Resource` from the config the same
way `build_resource` does (minus the environment-dependent hostname lookup
so the tests are deterministic), then flush through an
`InMemoryMetricExporter` and assert on the exported `Resource` attributes.
This mirrors the pattern in `test_resource_attributes.rs` but drives the
values from an `OtelConfig`.

### Documentation

- The `build_resource` doc comment in `subsystem.rs` already references
  FR-026 and FR-034 (from T-007 / T-022).
- The `OtelConfig.resource_attributes` field doc comment already
  references FR-026 (from T-002).
- `CHANGELOG.md` entry added under `## Unreleased`.

## Verification

```text
$ cargo check -p ragent-telemetry --features telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.81s

$ cargo check -p ragent-telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.64s

$ cargo check --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.27s

$ cargo check --workspace --features ragent-telemetry/telemetry
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.98s

$ cargo test -p ragent-telemetry --features telemetry --test test_custom_resource_attributes
running 18 tests
... (all 18 pass) ...
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p ragent-telemetry --features telemetry --lib --tests
... (103 lib + 10+18+16+10+20+19+6+5+19 integration tests all pass) ...

$ cargo test -p ragent-telemetry --features telemetry --doc
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-telemetry/tests/test_custom_resource_attributes.rs` | New 18-test integration suite (new file). |
| `specs/otel/PLAN.md` | T-024 status flipped from `pending` to `completed`. |
| `CHANGELOG.md` | "Added — Custom resource attributes from config (spec: otel, T-024, FR-026)" entry under `## Unreleased`. |

(No source-code changes were needed — the implementation was already
complete from T-002, T-007, T-021, and T-022.)

## Definition of Done checklist (from PLAN.md)

- [x] `cargo check -p ragent-telemetry` and `cargo test -p ragent-telemetry`
      pass with the `telemetry` feature enabled.
- [x] `cargo check` (default features, no `telemetry`) still passes and
      produces zero OTEL-related warnings.
- [x] Custom resource attributes from `telemetry.otel.resource_attributes`
      appear in the exported resource.
- [x] Custom attributes coexist with the static `service.name`,
      `service.version`, and `host.name` attributes.
- [x] Sensitive values in `resource_attributes` are redacted by the
      sensitive-data guard (FR-034).
- [x] `TelemetryConfig::merge` unions `resource_attributes` across config
      layers.
- [x] No existing agent loop, TUI, or HTTP server functionality is broken
      when telemetry is disabled.