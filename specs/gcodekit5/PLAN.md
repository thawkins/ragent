---
status: draft
spec: gcodekit5
---

# GCodeKit5 — Implementation Plan

## Summary

Build GCodeKit5 as a Rust edition 2024 Cargo workspace with a GTK4 user
interface. The workspace root hosts the binary and each domain lives in a
focused library crate under `crates/`. Work is ordered so that foundation crates
(core, settings, devicedb) land before the communication, designer, visualizer,
and UI layers that depend on them.

## Task Sequencing Notes

- Foundation first: `gcodekit5-core` must exist before every other crate.
- Communication and settings are prerequisites for the UI's Machine Control tab.
- Designer and CAM tools are prerequisites for the toolpath and stock-removal
  work in the Visualizer.
- Packaging, documentation, and i18n tasks run in parallel with late UI work and
  complete before the first release tag.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
| --- | --- | --- | --- | --- | --- | --- |
| T-001 | Create workspace skeleton with root binary and `crates/` layout | FR-001, FR-002, FR-064 | S | Critical | Pending | - |
| T-002 | Configure Rust edition 2024 and workspace lint/format tooling | FR-004, FR-057 | S | High | Pending | T-001 |
| T-003 | Implement `gcodekit5-core` types, units, constants, and error module | FR-002, FR-005, FR-006 | M | Critical | Pending | T-001 |
| T-004 | Implement core event bus (`event_bus/bus.rs`, `event_bus/events.rs`) | FR-007 | M | Critical | Pending | T-003 |
| T-005 | Implement G-Code command model, materials, tools data, and GTC import | FR-002, FR-043 | M | High | Pending | T-003 |
| T-006 | Implement `gcodekit5-settings` config, persistence, manager, controller | FR-002, FR-044 | M | High | Pending | T-003 |
| T-007 | Implement `gcodekit5-devicedb` model, manager, traits, and `devices.json` | FR-002, FR-050 | M | High | Pending | T-003 |
| T-008 | Implement communication transports (serial, TCP, WebSocket, buffered) | FR-021 | L | Critical | Pending | T-003, T-004 |
| T-009 | Implement firmware modules for GRBL, grblHAL, TinyG, g2core, Smoothieware, FluidNC | FR-022 | L | Critical | Pending | T-008 |
| T-010 | Implement serial port auto-detection and device selection | FR-023 | S | High | Pending | T-008 |
| T-011 | Implement 200 ms status polling wired to the event bus | FR-024 | M | Critical | Pending | T-008, T-004 |
| T-012 | Implement firmware capability detection and conservative defaults | FR-029, FR-052 | M | High | Pending | T-009 |
| T-013 | Implement GRBL character-counting streaming engine | FR-025, FR-026 | L | Critical | Pending | T-008, T-011 |
| T-014 | Implement streaming progress reporting (lines sent / total) | FR-027 | S | High | Pending | T-013 |
| T-015 | Implement Raw Status view for undecoded responses | FR-028 | S | Medium | Pending | T-008 |
| T-016 | Implement connection-loss handling and diagnostic reporting | FR-030 | M | High | Pending | T-011 |
| T-017 | Implement optional connection watchdog and reconnection | FR-031 | M | Medium | Pending | T-016, T-006 |
| T-018 | Implement `gcodekit5-gcodeeditor` buffer, viewport, undo manager, bridge | FR-041, FR-051 | M | Critical | Pending | T-003 |
| T-019 | Implement malformed G-Code diagnostics with line number reporting | FR-051 | S | High | Pending | T-018 |
| T-020 | Implement GTK4 main window and Notebook tab scaffold | FR-009, FR-010, FR-018 | L | Critical | Pending | T-004, T-006 |
| T-021 | Implement GResource bundle build script and resource manifest | FR-020, FR-064 | M | High | Pending | T-001 |
| T-022 | Implement Machine Control tab: DRO, jogging, Zero/WCS/Home/Unlock/EStop/overrides | FR-011, FR-012, FR-013 | L | Critical | Pending | T-020, T-013 |
| T-023 | Embed G-Code editor into the Machine Control tab with console and status bar | FR-010, FR-014 | L | Critical | Pending | T-018, T-022 |
| T-024 | Implement Pause, Resume, and Stop transport controls | FR-015, FR-016, FR-017 | M | High | Pending | T-023, T-013 |
| T-025 | Implement help system rendering English and Spanish markdown | FR-019 | M | Medium | Pending | T-020, T-021 |
| T-026 | Implement `gcodekit5-designer` shape model and parametric shapes | FR-032, FR-038 | L | Critical | Pending | T-003, T-004 |
| T-027 | Implement designer state: properties, selection, transforms, viewport, history | FR-035, FR-041 | L | Critical | Pending | T-026 |
| T-028 | Implement DXF, SVG, and raster image import plus composition | FR-033 | L | Critical | Pending | T-026 |
| T-029 | Implement per-object G-Code properties (speed, power, inversion) | FR-034, FR-035 | M | High | Pending | T-027, T-028 |
| T-030 | Implement designer canvas rendering with spatial index and render optimizer | FR-036 | L | High | Pending | T-026 |
| T-031 | Implement middle-mouse-button pan and zoom interactions | FR-036 | S | Medium | Pending | T-030 |
| T-032 | Implement Objects panel with reordering driving G-Code order | FR-037 | M | High | Pending | T-030 |
| T-033 | Implement designer G-Code generation with work-area boundary WARNING | FR-039 | M | Critical | Pending | T-027, T-032 |
| T-034 | Implement `.gckd` and `.gck4` save/load serialization | FR-040 | M | High | Pending | T-027 |
| T-035 | Implement designer toolpath generator: multipass, pocket, adaptive, v-carve | FR-042, FR-048 | L | High | Pending | T-033 |
| T-036 | Implement `gcodekit5-camtools` tabbed box, jigsaw, drill press, gerber | FR-042 | L | High | Pending | T-003, T-005 |
| T-037 | Implement CAM hatch generation, spoilboard grid, and spoilsurface | FR-042 | M | Medium | Pending | T-036 |
| T-038 | Implement CAM laser engraver, optimizer, speeds/feeds, validator | FR-042 | L | High | Pending | T-036 |
| T-039 | Implement CAM Tools panel UI in the GTK4 Notebook | FR-042 | L | Medium | Pending | T-036, T-020 |
| T-040 | Implement `gcodekit5-visualizer` G-Code parser, pipeline, and processors | FR-045, FR-048 | L | Critical | Pending | T-003 |
| T-041 | Implement 2D/3D renderer with camera, controls, and features | FR-045 | L | Critical | Pending | T-040 |
| T-042 | Implement toolpath cache for incremental re-render | FR-048 | M | Medium | Pending | T-040, T-041 |
| T-043 | Implement toolpath simulation and preview mode | FR-046 | M | High | Pending | T-041 |
| T-044 | Implement 3D stock removal against the stock envelope | FR-047 | L | High | Pending | T-041, T-035 |
| T-045 | Implement tool library UI wiring for materials and tools managers | FR-043, FR-044 | M | Medium | Pending | T-005, T-006 |
| T-046 | Implement shared UI state with `Rc<RefCell<...>>` interior mutability | NFR-003 | M | High | Pending | T-020 |
| T-047 | Ensure transport I/O runs off the GTK main thread | NFR-001, NFR-002 | M | Critical | Pending | T-013, T-020 |
| T-048 | Ship sample assets: Gerber, DXF, G-Code, designs, SVG, STL, fonts, configs | FR-049 | L | Medium | Pending | T-001 |
| T-049 | Implement internationalization catalogs and `scripts/update-po.sh` | FR-056 | M | Medium | Pending | T-020 |
| T-050 | Add Flatpak manifest, metainfo, and desktop file | FR-053 | M | Medium | Pending | T-021 |
| T-051 | Add WiX installer and Windows PowerShell build/bundle/DLL-check scripts | FR-054 | L | Medium | Pending | T-021 |
| T-052 | Add macOS bundle and DMG creation scripts | FR-055 | M | Medium | Pending | T-021 |
| T-053 | Add CI workflows: code-quality, msrv, mutation-testing, release, test-coverage | FR-058 | M | High | Pending | T-002, T-063 |
| T-054 | Add issue templates, PR template, pre-commit hook, devcontainer, VS Code, Dependabot | FR-059 | M | Medium | Pending | T-001 |
| T-055 | Write ADR set covering all ten documented decisions | FR-061 | M | High | Pending | T-001 |
| T-056 | Write root documentation files | FR-060 | L | High | Pending | T-001 |
| T-057 | Add Criterion benchmarks for core, designer, and visualizer | FR-062 | M | Low | Pending | T-035, T-041 |
| T-058 | Retain and document the legacy single-crate application under `legacy/` | FR-063 | S | Low | Pending | T-001 |
| T-059 | Add dual MIT and Apache-2.0 license files | FR-008 | S | High | Pending | T-001 |
| T-060 | Implement splash screen on start-up | FR-018 | S | Low | Pending | T-020, T-021 |
| T-061 | Add user guide documentation numbered 01–93 under `docs/user/` | FR-060 | L | Medium | Pending | T-056 |
| T-062 | Configure `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.cargo/` config and mutants | FR-057 | S | High | Pending | T-001 |
| T-063 | Establish per-crate `error.rs` and `thiserror` conventions | FR-005, NFR-005 | M | High | Pending | T-003 |
| T-064 | Implement settings/device tab for configuration editing and capability display | FR-029, FR-050 | M | Medium | Pending | T-007, T-020 |

## Milestones

| Milestone | Tasks | Exit Criteria |
| --- | --- | --- |
| M1 — Foundation | T-001 – T-007, T-059, T-062, T-063 | Workspace builds; core, settings, devicedb, and event bus usable from tests |
| M2 — Connectivity | T-008 – T-017 | A controller connects, polls status at 200 ms, and streams a file |
| M3 — Machine Control UI | T-018 – T-025, T-046, T-047, T-060 | Machine Control tab drives a live connection with editor and console |
| M4 — Designer and CAM | T-026 – T-039, T-045 | Imports, per-object G-Code properties, boundary warning, CAM generators |
| M5 — Visualizer | T-040 – T-044 | 2D/3D rendering, simulation, and stock removal work on generated toolpaths |
| M6 — Release Engineering | T-048 – T-058, T-061, T-064 | Packaging, CI, i18n, docs, licenses, and benchmarks are complete |

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| GTK4 availability differs across platforms | Build failures on macOS/Windows | Pin `gtk4-rs`; maintain Windows DLL bundling scripts and macOS bundle scripts |
| Firmware status formats differ subtly | Incorrect DRO or capability data | One module per firmware with a shared trait; conservative defaults on parse failure |
| Streaming buffer accounting errors | Lost or duplicated lines, machine faults | Dedicated streaming engine task with focused tests on the 127-byte buffer |
| Workspace crate churn | Slow incremental builds | Enforce crate boundaries per FR-003 and keep leaf crates dependency-light |
| Designer canvas complexity | Render performance regressions | Spatial index plus render optimizer, with Criterion benchmarks in T-057 |
