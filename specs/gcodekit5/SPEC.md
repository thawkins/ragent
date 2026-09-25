---
status: draft
---

# GCodeKit5 — Cross-Platform G-Code Sender and CNC/Laser Controller

## Overview

GCodeKit5 is a GTK4 desktop application written in Rust that acts as a modern,
cross-platform G-Code sender and controller for Laser and CNC machines. It is an
open-source alternative to Universal G-Code Sender (UGS) that connects to
controllers over serial, TCP, and WebSocket, supports multiple firmware families
through a unified interface, controls up to 6 axes, and provides visual design,
CAM, and toolpath visualization tooling.

The application is delivered as a Cargo workspace so that only changed crates
recompile, with the binary at the workspace root and all library crates under
`crates/`.

## Technology Stack

- Language: Rust, edition 2024
- UI framework: GTK4 via `gtk4-rs` (documented in ADR-001)
- Targets: Linux, macOS, Windows
- Crate type: Cargo workspace with the binary at the root and libraries under `crates/`

## Scope

### In Scope

- Machine control (jogging, DRO, WCS, homing, unlock, e-stop, overrides)
- Device management (auto-detect, status polling, settings, capabilities)
- G-Code editor embedded in the Machine Control tab
- GRBL character-counting G-Code streaming with progress reporting
- Designer canvas (shapes, DXF/SVG/raster import, per-object G-code properties,
  boundary checking, object ordering)
- CAM tools (tabbed box, jigsaw, drill press, gerber, hatch, spoilboard,
  laser engraving, v-carve, adaptive/pocket/multipass)
- 2D/3D Visualizer with toolpath preview and stock removal
- Settings, device profile database, tool database, and GTC import
- Cross-platform packaging (Flatpak, WiX, macOS bundle/DMG)
- Internationalization (English and Spanish markdown help; `.po`/`.pot` catalogs)
- Documentation and ADR set

### Out of Scope

- No browser-based UI
- No TUI interface
- No separate G-Code Editing tab (the editor is embedded in Machine Control)
- No echoed G-Code in the console output (streaming speed must not be reduced)

## Definitions

| Term | Meaning |
| ---- | ------- |
| DRO | Digital Readout — live display of axis positions |
| WCS | Work Coordinate System (G54–G59) |
| GRBL character-counting protocol | Flow control where the sender tracks the receiver's free RX buffer bytes |
| Firmware family | One of GRBL, grblHAL, TinyG, g2core, Smoothieware, FluidNC |
| GResource bundle | GLib-compiled resource archive built by `gcodekit5-ui/build.rs` |

## Requirements

### Workspace and Architecture

**FR-001 (Ubiquitous)** The system shall be organized as a Cargo workspace with
the binary at the workspace root and all library crates under `crates/`.

**FR-002 (Ubiquitous)** The system shall provide the library crates
`gcodekit5-core`, `gcodekit5-camtools`, `gcodekit5-designer`,
`gcodekit5-gcodeeditor`, `gcodekit5-communication`, `gcodekit5-settings`,
`gcodekit5-devicedb`, `gcodekit5-visualizer`, and `gcodekit5-ui`.

**FR-003 (Ubiquitous)** When a crate's source changes, the build shall recompile
only the changed crate and its dependents, and shall not recompile unrelated crates.

**FR-004 (Ubiquitous)** The system shall use Rust edition 2024 and the GTK4
binding `gtk4-rs` for all user interface code.

**FR-005 (Ubiquitous)** Each library crate shall define a dedicated `error.rs`
module using `thiserror` for its error types.

**FR-006 (Ubiquitous)** The system shall represent dimensional values internally
in millimetres and shall convert to display or controller units only at
boundaries, consistent with ADR-008.

**FR-007 (Ubiquitous)** The system shall use an event bus for communication
between the user interface and core services rather than direct cross-crate calls.

**FR-008 (Ubiquitous)** The system shall be dual-licensed under MIT and
Apache-2.0, providing `LICENSE-MIT` and `LICENSE-APACHE`.

### User Interface

**FR-009 (Ubiquitous)** The system shall present a GTK4 main window containing a
Notebook with the tabs Machine Control, Designer, Visualizer, CAM Tools, and
Settings/Device.

**FR-010 (Ubiquitous)** The G-Code editor shall be embedded within the Machine
Control tab and the system shall not present a separate G-Code Editing tab.

**FR-011 (Ubiquitous)** The Machine Control tab shall display a digital readout
of all configured axes with a displayed resolution of 0.001 mm.

**FR-012 (Ubiquitous)** The Machine Control tab shall provide jogging controls
with selectable step sizes of 0.1, 1, 10, and 100 mm for linear axes and of
equivalent degree steps for rotary axes.

**FR-013 (Ubiquitous)** The Machine Control tab shall provide Zero X/Y/Z
controls that emit G92 commands, a WCS selector offering G54 through G59, a Home
control that emits `$H`, an Unlock control that emits `$X`, an Emergency Stop
control, and feed rate, rapid rate, and spindle speed override controls.

**FR-014 (Ubiquitous)** The Machine Control tab shall present a live console
panel and a status bar containing a progress bar together with Stop, Pause, and
Resume buttons.

**FR-015 (Event-driven)** When the user activates the Pause control during
streaming, the system shall transmit the GRBL feed-hold command `!`.

**FR-016 (Event-driven)** When the user activates the Resume control during a
feed hold, the system shall transmit the GRBL cycle-start command `~`.

**FR-017 (Event-driven)** When the user activates the Stop control during
streaming, the system shall terminate transmission immediately and update the
status bar to a stopped state.

**FR-018 (Ubiquitous)** The system shall display a splash screen at application
start-up.

**FR-019 (Ubiquitous)** The system shall render bundled help content from
markdown resources in English and Spanish.

**FR-020 (Ubiquitous)** The system shall load application icons and images from a
GResource bundle produced by `gcodekit5-ui/build.rs` from
`crates/gcodekit5-ui/resources/gresources.xml`.

### Communication and Streaming

**FR-021 (Ubiquitous)** The system shall support serial, TCP, and WebSocket
transports behind a single communication abstraction.

**FR-022 (Ubiquitous)** The system shall provide a unified firmware interface
with one implementation module for each of GRBL, grblHAL, TinyG, g2core,
Smoothieware, and FluidNC.

**FR-023 (Event-driven)** When the application starts or the user requests a port
scan, the system shall auto-detect available serial ports and present them for
selection.

**FR-024 (State-driven)** While a device connection is established, the system
shall poll machine status and update the DRO and status bar every 200 ms.

**FR-025 (Ubiquitous)** The system shall stream G-Code using the GRBL
character-counting protocol, tracking a free RX buffer of 127 bytes, sending at
most 5 lines per cycle, and consuming `ok` acknowledgements before releasing
tracked buffer bytes.

**FR-026 (Ubiquitous)** The streaming engine shall filter comment lines and blank
lines before transmission.

**FR-027 (State-driven)** While a G-Code stream is in progress, the system shall
display progress as lines sent against total lines in the status bar.

**FR-028 (Ubiquitous)** The system shall provide a Raw Status view that displays
undecoded controller status responses.

**FR-029 (Ubiquitous)** The system shall report detected firmware capabilities
including arc support (G2/G3), variable spindle or PWM control, homing cycle
support, probing, laser mode, multi-axis support for 4 to 6 axes, safety door,
coolant control, and tool change support.

**FR-030 (Event-driven)** When a controller connection is lost, the system shall
enter an alarm or disconnected state, halt streaming, and present a diagnostic
message to the user.

**FR-031 (Optional)** Where a device profile is configured with connection
watchdog parameters, the system shall monitor connection health and initiate
reconnection according to those parameters.

### Designer

**FR-032 (Ubiquitous)** The Designer shall provide shape tooling for line,
polyline, rectangle, circle, ellipse, polygon, triangle, slot, gear, sprocket,
timing pulley, L support, U support, and text.

**FR-033 (Ubiquitous)** The Designer shall support importing DXF files, SVG
files, and raster images, and shall allow raster images and vector objects to be
composed on a single canvas.

**FR-034 (Event-driven)** When a raster image is imported, the system shall allow
the user to configure that image's G-Code generation parameters, including speed,
power, and inversion, independently of other objects.

**FR-035 (Ubiquitous)** The Designer shall support a global G-Code property set
for the canvas and per-object property overrides, so that different objects may
use different speed and power settings for engraving and cutting.

**FR-036 (Event-driven)** When the user activates the pan gesture with the middle
mouse button on the Designer canvas, the system shall pan the canvas view.

**FR-037 (Ubiquitous)** The Designer shall provide an Objects panel that allows
the user to reorder canvas objects and that determines the G-Code generation
order.

**FR-038 (Event-driven)** When a chain sprocket is configured, the system shall
accept only the pitch and tooth count and shall derive the roller diameter
according to ANSI/ISO standard values.

**FR-039 (Event-driven)** When G-Code is generated and any toolpath point lies
outside the configured work area, the system shall emit a WARNING comment in the
generated G-Code identifying the out-of-bounds condition.

**FR-040 (Ubiquitous)** The Designer shall support save and load of designs in the
`.gckd` and `.gck4` formats.

**FR-041 (Ubiquitous)** The Designer shall record edit operations in an undo and
redo history.

### CAM Tools

**FR-042 (Ubiquitous)** The CAM Tools tab shall provide tabbed-box generation,
jigsaw puzzle generation, drill press operations, Gerber processing, hatch
generation, spoilboard grid generation, spoilboard surfacing, laser engraving,
v-carve, and adaptive, pocket, and multipass toolpath operations.

**FR-043 (Ubiquitous)** The system shall support importing GTC tool databases for
the tool library.

**FR-044 (Ubiquitous)** The system shall provide a tool library persisted with the
application settings and device profiles.

### Visualizer

**FR-045 (Ubiquitous)** The Visualizer shall provide 2D and 3D rendering of
parsed toolpaths with camera, zoom, and rotation controls.

**FR-046 (Ubiquitous)** The Visualizer shall support a toolpath simulation or
preview mode that advances through the parsed G-Code.

**FR-047 (Ubiquitous)** The Visualizer shall model 3D stock removal against the
configured stock envelope.

**FR-048 (Ubiquitous)** The Visualizer shall cache parsed toolpaths so that
re-rendering a previously parsed file does not require a full reparse.

### Data, Files, and Assets

**FR-049 (Ubiquitous)** The system shall ship sample assets including Gerber
board sets, DXF files, G-Code samples for boxes, lines, circles, and malformed
G-Code error cases, designs in `.gckd` and `.gck4`, SVG art, STL files, Fira Code
fonts, tool database files, GRBL configuration JSON samples, and a G-Code
language specification.

**FR-050 (Ubiquitous)** The system shall store device profiles in `devices.json`
and shall manage them through the `gcodekit5-devicedb` crate.

**FR-051 (Unwanted)** If the user opens a malformed G-Code file, then the system
shall report the offending line number and shall not crash or corrupt the buffer.

**FR-052 (Unwanted)** If a firmware capability query returns an unrecognized or
empty response, then the system shall default to a conservative capability set
and shall not enable unsupported features.

### Packaging, Tooling, and Documentation

**FR-053 (Ubiquitous)** The system shall provide a Flatpak manifest, metainfo, and
desktop file under `flatpak/`.

**FR-054 (Ubiquitous)** The system shall provide a WiX installer source under
`wix/` and PowerShell build, GTK DLL bundling, and DLL dependency check scripts
under `scripts/`.

**FR-055 (Ubiquitous)** The system shall provide macOS bundle and DMG creation
scripts under `scripts/`.

**FR-056 (Ubiquitous)** The system shall provide internationalization catalogs
under `po/` and an `scripts/update-po.sh` helper.

**FR-057 (Ubiquitous)** The repository shall provide `rustfmt.toml`,
`clippy.toml`, `deny.toml`, `.cargo/config.toml`, and `.cargo/mutants.toml` at the
workspace root.

**FR-058 (Ubiquitous)** The repository shall provide continuous integration
workflows for code quality, MSRV, mutation testing, release, and test coverage
under `.github/workflows/`.

**FR-059 (Ubiquitous)** The repository shall provide issue templates for bug,
feature, task, and change, a pull request template, a pre-commit hook, a
devcontainer definition, VS Code settings, and Dependabot configuration.

**FR-060 (Ubiquitous)** The repository shall maintain root documentation files
`README.md`, `SPEC.md`, `ARCHITECTURE.md`, `CHANGELOG.md`, `CONTRIBUTING.md`,
`DEVELOPMENT.md`, `TOOLCHAIN.md`, `TESTPLAN.md`, `STATS.md`, `RELEASE.md`,
`AGENTS.md`, and `GTK4.md`.

**FR-061 (Ubiquitous)** The repository shall maintain Architecture Decision
Records covering the GTK4 UI framework, the coordinate system, modular crates,
interior mutability, error handling, the event bus, paned layout, the units
system, the communication protocol, and dependency management.

**FR-062 (Ubiquitous)** The repository shall provide Criterion benchmarks in
`crates/gcodekit5-core/benches/`, `crates/gcodekit5-designer/benches/`, and
`crates/gcodekit5-visualizer/benches/`.

**FR-063 (Ubiquitous)** The repository shall retain the original single-crate
application under `legacy/` for reference and shall document it in
`LEGACY_CODE.md`.

**FR-064 (Ubiquitous)** The workspace root and the `gcodekit5-ui` crate shall each
provide a `build.rs` build script.

## Non-Functional Requirements

**NFR-001** The UI shall remain responsive during G-Code streaming; all transport
reads and writes shall occur off the GTK main thread.

**NFR-002** Status polling shall not exceed 200 ms intervals to keep the DRO
current.

**NFR-003** Shared user interface state shall use interior mutability
(`Rc<RefCell<...>>`) consistent with ADR-004.

**NFR-004** The application shall build on Linux, macOS, and Windows from the same
source tree.

**NFR-005** No `unwrap()` shall be used on user-facing paths; errors shall be
propagated or reported through the per-crate error types.

## Acceptance Criteria

1. All crates in FR-002 exist and the workspace builds with `cargo build`.
2. The Notebook tabs in FR-009 are present and the editor is inside Machine
   Control (FR-010).
3. A serial connection can be established, auto-detected (FR-023), and status
   updates appear at 200 ms cadence (FR-024).
4. A sample G-Code file streams successfully under the character-counting
   protocol with progress displayed (FR-025, FR-027).
5. Each firmware module in FR-022 can parse its own status response format.
6. Designer imports DXF, SVG, and a raster image and generates G-Code with a
   boundary WARNING for out-of-area geometry (FR-033, FR-039).
7. Visualizer renders a toolpath in 2D and 3D and performs stock removal
   (FR-045, FR-047).
8. All EARS requirement templates (ubiquitous, event-driven, state-driven,
   optional, unwanted) are represented in the requirement set.

## Traceability

| Area | Requirements |
| ---- | ------------ |
| Workspace and architecture | FR-001 – FR-008 |
| User interface | FR-009 – FR-020 |
| Communication and streaming | FR-021 – FR-031 |
| Designer | FR-032 – FR-041 |
| CAM tools | FR-042 – FR-044 |
| Visualizer | FR-045 – FR-048 |
| Data, files, assets | FR-049 – FR-052 |
| Packaging, tooling, documentation | FR-053 – FR-064 |
