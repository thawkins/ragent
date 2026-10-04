# Implementation Plan — os_info Operating System Introspection Tool

Spec: `specs/osinfo/SPEC.md`

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `sysinfo` (features = ["system"]) dependency to `crates/ragent-agent/Cargo.toml` | FR-005, FR-006, NFR-002 | S | High | completed | — |
| T-002 | Create `os_info` module with module docblock, `OsInfoTool` struct and internal `OsInfo` serde struct | FR-001, FR-002 | S | High | completed | T-001 |
| T-003 | Implement OS identity collection (family, name, version, kernel, hostname, distro) | FR-002, FR-003 | M | Critical | completed | T-002 |
| T-004 | Implement CPU collection (arch, physical/logical cores, vendor, brand, frequency) with `unknown`/`0` placeholders | FR-005, FR-011 | M | High | completed | T-002 |
| T-005 | Implement memory/swap/uptime/boot-time collection (bytes + UTC boot timestamp) | FR-006, FR-011 | M | High | completed | T-002 |
| T-006 | Implement process environment collection (pid, cwd, shell, username) with `unknown` fallbacks | FR-007, FR-011 | S | High | completed | T-002 |
| T-007 | Implement text (markdown) renderer with sections OS/CPU/Memory/Process and human-readable GiB + uptime | FR-004, FR-006 | M | High | completed | T-003, T-004, T-005, T-006 |
| T-008 | Implement JSON renderer with stable snake_case keys | FR-004 | S | Medium | completed | T-003, T-004, T-005, T-006 |
| T-009 | Implement `Tool` trait (name, description, parameters schema with `additionalProperties: false`, `permission_category() == "none"`, `execute`) | FR-001, FR-004, FR-008, FR-009, FR-010, NFR-003 | M | Critical | completed | T-002 |
| T-010 | Verify read-only guarantee: no writes, no process spawn, no network, always succeeds | FR-008, FR-009, FR-011 | S | High | completed | T-009 |
| T-011 | Register `os_info` in `crates/ragent-agent/src/tool/mod.rs` (module + registry) | FR-001 | S | Critical | completed | T-009 |
| T-012 | Add TUI `input_summary` compact rendering for `os_info` calls | FR-012 | S | Medium | completed | T-011 |
| T-013 | Add external tests in `crates/ragent-agent/tests/test_os_info_tool.rs` (registration, text content, JSON structure, invalid-format error) | FR-001, FR-002, FR-003, FR-004, FR-010 | M | High | completed | T-011 |
| T-014 | Add TUI tool-display test for `os_info` `input_summary` | FR-012 | S | Low | completed | T-012 |
| T-015 | Run `cargo fmt`, `cargo clippy`, and crate-scoped tests; confirm zero warnings | all | S | High | completed | T-013, T-014 |
| T-016 | Manual test pass against `TESTPLAN.md` | all | S | Medium | completed | T-015 |
| T-017 | Expose `pub(crate)` collector + text renderer from `os_info` module for reuse by the TUI | FR-016, FR-018 | S | Critical | completed | T-009 |
| T-018 | Register `osinfo` in TUI `SLASH_COMMANDS` table, help index, and autocomplete suggestions | FR-013 | S | High | completed | T-017 |
| T-019 | Add `osinfo` dispatch arm in `execute_slash_command_inner` routing to `handle_osinfo_command` | FR-014 | S | High | completed | T-018 |
| T-020 | Implement `handle_osinfo_command` dispatcher (empty/help aliases, unknown-subcommand correction) | FR-014, FR-015 | S | High | completed | T-019 |
| T-021 | Implement `/osinfo help` page (subcommand table, read-only note, worked example; `From:` prefix + status bar) | FR-015 | S | Medium | completed | T-020 |
| T-022 | Implement `/osinfo show` rendering via shared collector/renderer; `From:` prefix + status bar | FR-016, FR-017, FR-018, NFR-004 | M | Critical | completed | T-017, T-020 |
| T-023 | Verify slash command triggers no writes, no processes, no network, no permission prompt, no LLM call | FR-017, NFR-004 | S | High | completed | T-022 |
| T-024 | Add TUI slash-command tests (`show` output content, help page, unknown subcommand) in `crates/ragent-tui/tests/` | FR-013, FR-014, FR-015, FR-016 | M | High | completed | T-022 |
| T-025 | Update `TESTPLAN.md` with `/osinfo` manual test cases and re-run manual pass | FR-013–FR-018 | S | Medium | completed | T-024 |
| T-026 | Add GPU section collection (`GpuSection`/`GpuAdapter`) reading `/sys/class/drm` + `pci.ids`; classify integrated/discrete/virtual, with `unknown`/`0` fallbacks | FR-019, FR-011 | M | High | completed | T-004 |
| T-027 | Render GPU adapters in the text report (`## GPU` section, empty-list placeholder) and JSON (`gpus` array) | FR-004, FR-019 | S | High | completed | T-026 |
| T-028 | Extend tool description and `/osinfo help` page to mention graphics adapters | FR-019, NFR-003 | S | Medium | completed | T-027 |
| T-029 | Add GPU assertions to `test_os_info_tool.rs` (section present, `gpus` array shape, per-adapter keys) and TUI section checks | FR-019 | S | High | completed | T-027 |
| T-030 | Run `cargo fmt`, `cargo clippy`, and crate-scoped tests; confirm zero new warnings | all | S | High | completed | T-029 |
| T-031 | Add graphics-API detection (`API_ORDER`, platform scoping, read-only runtime inference) to `GpuSection`; render `APIs` line (text) and `gpu_apis` array (JSON) | FR-020, FR-019, FR-011 | M | High | completed | T-027 |
| T-032 | Add opt-in `probe` parameter: `probe_api_supported` + timeout-bounded allowlist (`vulkaninfo`, `nvidia-smi`, `rocminfo`, `system_profiler`), schema entry, and tool-description update | FR-020, FR-004, NFR-003 | M | High | completed | T-031 |
| T-033 | Add tests (platform-scoped `gpu_apis`, `APIs` text line, schema `probe`, TUI `input_summary` probe rendering) and run `cargo fmt`, `cargo clippy`, crate-scoped tests | FR-020, FR-012, FR-019 | M | High | completed | T-032 |
| T-034 | Add graphics-API version capture (FR-021): read-only version files (Vulkan ICD `api_version`, CUDA/ROCm/OptiX files, Metal plist), probe-output capture (`glxinfo -B` for OpenGL/GLES/Mesa; `vulkaninfo`/`nvidia-smi`), `GpuApi { name, version }` model, and `gpu_apis` object rendering + text `Name Version` line | FR-021, FR-020, FR-011 | M | High | completed | T-033 |
| T-035 | Update `SPEC.md` (FR-020/FR-021), `TESTPLAN.md` (TC-018) and tests (`test_os_info_gpu_api_versions_are_reported`, JSON/text shape) and run `cargo fmt`, `cargo clippy`, crate-scoped tests | FR-021, FR-020 | M | High | completed | T-034 |
| T-036 | Flip graphics-API probing to default-on: `probe` defaults to true (tool `execute`, `OsInfo::collect`), `/osinfo show` probes with a `--no-probe` opt-out, spec FR-009/FR-010/FR-014/FR-015/FR-016/FR-017/FR-020/FR-021 updated, tests updated, `cargo fmt`/`cargo clippy`/crate-scoped tests | FR-020, FR-021, FR-009, FR-010, FR-014, FR-015, FR-016, FR-017 | M | High | completed | T-035 |
| T-037 | Add `HardwareSection` (system/chassis/board/BIOS identity from read-only `/sys/class/dmi/id`, storage devices from `/sys/block`, network interfaces from `/sys/class/net`) with `unknown`/empty-list fallbacks and SMBIOS chassis-type labelling; exclude serial/UUID/asset-tag files | FR-022, FR-023, FR-024, FR-025, FR-011 | M | High | completed | T-004 |
| T-038 | Render the `## Hardware` section in the text report (identity bullets + storage/network lists with `none detected` placeholders) and the `storage_devices`/`network_interfaces` arrays plus identity keys in JSON | FR-004, FR-022, FR-023, FR-024 | S | High | completed | T-037 |
| T-039 | Extend the tool description and `/osinfo help` page to mention the physical-hardware section and the sensitive-identifier exclusion | FR-022, FR-025, NFR-003 | S | Medium | completed | T-038 |
| T-040 | Add hardware assertions to `test_os_info_tool.rs` (section present, JSON key set, per-device keys, sensitive-identifier exclusion) and update the documented-section and key-set checks | FR-022, FR-023, FR-024, FR-025 | S | High | completed | T-038 |
| T-041 | Run `cargo fmt`, `cargo clippy`, and crate-scoped tests; confirm zero new warnings | all | S | High | completed | T-040 |

## Notes

- The tool portion of the feature lives in a single new source file
  `crates/ragent-agent/src/tool/os_info.rs`, modelled one-for-one on
  `ragent_info.rs`. Registration is one line in `mod.rs` beside
  `registry.register(Arc::new(ragent_info::RagentInfoTool));`.
- `sysinfo` is used for OS/CPU/memory/uptime values; `std::env` covers shell,
  username, pid, and cwd. Physical-hardware identity, storage, and network data
  come from read-only `/sys` files on Linux (DMI sysfs, `/sys/block`,
  `/sys/class/net`) and degrade to `unknown`/empty lists elsewhere (FR-022..FR-025).
  No external commands are spawned, preserving the
  read-only and sub-250 ms guarantees.
- The `/osinfo` slash command reuses the tool's collector and text renderer
  (exposed as `pub(crate)` items by T-017) so both surfaces report identical
  fields and placeholders (FR-018); the TUI adds only registration, dispatch,
  and `From:`/status-bar plumbing in `crates/ragent-tui/src/app/`
  (`state.rs` for `SLASH_COMMANDS`, `slash.rs` for dispatch and handlers).
- Per workspace rules, tests live in `crates/ragent-agent/tests/` and
  `crates/ragent-tui/tests/`, not inline in the source files.
- `/osinfo show` performs no LLM call; it renders directly into the message
  window as an assistant message.