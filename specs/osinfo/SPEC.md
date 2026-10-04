---
status: draft
audit:
  - { time: 1791089848, from: "none", to: "draft", actor: "system" }
---
# os_info — Operating System Type and Configuration Introspection Tool

## Overview

ragent already ships `ragent_info` (build/version introspection about the
ragent binary itself), but neither the LLM nor the user has a way to learn
about the *host environment* ragent is running on: the operating system type,
kernel, distribution, architecture, CPU, graphics adapters, memory, or shell
environment. Answering
a question such as "are we on Linux or macOS, and how much RAM does this
machine have?" currently requires running a `bash` command, which goes through
the 7-layer bash security gate and is often overkill for a purely informational
lookup.

This spec defines a new built-in tool, **`os_info`**, that reports extensive,
read-only information about the operating system and hardware configuration of
the machine running ragent, plus a companion **`/osinfo` slash command**
(`help`, `show`) for viewing the same information directly. The tool executes
no external commands, writes nothing, and always succeeds; the slash command
renders the tool's text report into the message window without an LLM call.

## Scope

In scope:

- A new `os_info` tool in `crates/ragent-agent/src/tool/os_info.rs` registered
  in the standard agent session registry, following the `ragent_info.rs`
  pattern (static `Tool` impl, `format` parameter with `text` (default,
  human-readable markdown) or `json` output).
- OS identity: `family`/`type` (`linux`, `macos`, `windows`, `freebsd`, …),
  kernel/OS version string, long OS version, and Linux distribution id and
  name when applicable (e.g. `ubuntu`, `fedora`), plus hostname.
- Architecture and CPU: CPU architecture (`x86_64`, `aarch64`, …), physical
  and logical core counts, CPU vendor and brand string, and CPU frequency.
- Graphics adapters (GPU): every detected adapter, integrated or discrete, with
  its PCI vendor/device ids, the bound kernel driver, a coarse
  integrated/discrete/virtual/unknown classification, and dedicated VRAM where
  the host exposes it. Linux reads the read-only DRM sysfs tree; other platforms
  report an empty adapter list (FR-019).
- Graphics-API support and versions: each detected API (from the fixed
  vocabulary) with a best-effort version number, inferred from read-only runtime
  artefacts and refined by the opt-in `probe` diagnostics (FR-020, FR-021).
- Physical hardware: system (product) vendor, model, version and family;
  chassis type (SMBIOS label) and vendor; motherboard vendor, name, and
  version; firmware (BIOS) vendor, version, and release date; the detected
  physical storage devices (name, model, vendor, capacity, NVMe/SSD/HDD class);
  and the network interfaces (name, MAC address, operational state, link speed,
  loopback flag). On Linux all values come from the read-only DMI sysfs tree
  (`/sys/class/dmi/id`), `/sys/block`, and `/sys/class/net`; other platforms
  report `unknown` placeholders and empty device lists (FR-022, FR-023, FR-024).
  Serial numbers, UUIDs, and asset tags are never read (FR-025).
- Memory and swap: total and available physical memory, total and free swap,
  reported in bytes (exact) with human-readable renderings in the text format.
  Uptime and boot time.
- Process environment: the ragent process id, the user shell (e.g. `$SHELL`
  on Unix, `COMSPEC` on Windows), the current username, and the current
  working directory of the ragent process.
- Tool description, JSON parameters schema, `none` permission category
  (read-only, always allowed), registration in `mod.rs`, and help/summary
  presentation so tool calls render compactly in the TUI log panel.
- A `/osinfo` slash command (`help`, `show`) that renders a displayable
  version of the same information directly into the message window, reusing
  the tool's collector and text renderer so both surfaces stay in lockstep.

Out of scope:

- Detailed GPU metrics beyond identity: utilisation, temperature, clock, and
  per-process memory (leave to a future `sys_info` tool if desired).
- Filesystem/mount listing beyond whole-disk identity, partition enumeration,
  storage capacity in use, network traffic counters, IP-address assignment,
  Wi-Fi SSID/band, battery/sensor data, and peripheral/USB/PCI device
  enumeration beyond the graphics adapters of FR-019.
- Sensitive hardware identifiers: serial numbers, UUIDs, and asset tags are
  explicitly excluded (FR-025).
- Additional `/osinfo` subcommands beyond `help` and `show` (e.g. a `--json`
  flag) — displayable text output only.
- Environment-variable dumps (sensitive; explicitly excluded).
- Platform-specific deep probing (e.g. Windows registry, macOS
  `sysctl` internals beyond what the support library exposes).

## Background

`ragent_info` (`crates/ragent-agent/src/tool/ragent_info.rs`) is the closest
existing analogue: a zero-parameter (plus `format`) read-only introspection
tool with `permission_category() == "none"`. `os_info` will mirror its
structure one-for-one: module docblock, a `struct OsInfoTool`, an internal
`#[derive(Serialize)] struct OsInfo`, and a `Tool` impl whose `execute` either
serialises the struct to JSON or renders a markdown table.

Runtime OS data is best sourced from `sysinfo` (pure-Rust, cross-platform,
no shell-out) plus `std::env` for process-level values such as the working
directory, shell, and username. `sysinfo` exposes no GPU data, so graphics
adapters are enumerated from the read-only DRM sysfs tree under
`/sys/class/drm` on Linux (each `cardN` node's PCI vendor/device ids and bound
driver), with adapter display names resolved from the read-only system
`pci.ids` database when present. Non-Linux platforms report an empty adapter
list. The tool's read-only guarantee means it takes no user-supplied parameters
other than `format`, never opens files for writing, never runs external
commands, and never mutates state.

To keep both surfaces reporting identical values, the `os_info` module exposes
its collector and text renderer as `pub(crate)` items; the `/osinfo` slash
command (registered in the TUI `SLASH_COMMANDS` table and dispatched via
`handle_osinfo_command`) calls into those same functions, adds only the
`From: /osinfo …` prefix and status-bar bookkeeping, and performs no LLM
call.

## Requirements

### FR-001 — Tool registration (Ubiquitous)

The system SHALL register a tool named `os_info` in
`crates/ragent-agent/src/tool/mod.rs` (a new `pub mod os_info;` and a
`registry.register(Arc::new(os_info::OsInfoTool));` line in the session
registry builder), so that `os_info` appears in the tool catalogue for every
agent session and in the `/tools` listing without further configuration.

### FR-002 — OS identity report (Ubiquitous)

The system SHALL report the operating system identity of the host: OS
family/type (`linux`, `macos`, `windows`, and other values exposed by the
supporting library), a human-readable OS name and version string, the kernel
version, and the hostname. Each value SHALL be reported as a string; when a
value cannot be determined on the host platform the tool SHALL report `unknown`
rather than fail or omit the field.

### FR-003 — Linux distribution detail (State-driven)

WHILE the host OS family is Linux, the system SHALL additionally report the
distribution identifier and display name (e.g. `fedora`, `Fedora Linux 41
(Workstation Edition)`). On non-Linux platforms the distribution fields SHALL
be reported as `n/a (`<family>`)` so the output schema is stable across
platforms.

### FR-004 — Invocation with output-format selection (Event-driven)

WHEN the agent calls `os_info` with no arguments, or with
`{"format": "text"}`, the system SHALL render a human-readable markdown
report with a section per category (`Operating System`, `CPU`, `GPU`, `Memory`,
`Process`); WHEN the agent calls `os_info` with `{"format": "json"}`, the
system SHALL return the same information as a single structured JSON object
with stable snake_case keys. Any other `format` value SHALL produce an error
message listing the accepted values.

### FR-005 — CPU report (Ubiquitous)

The system SHALL report the CPU architecture (e.g. `x86_64`, `aarch64`), the
physical core count, the logical core count, the CPU vendor id, the CPU brand
string, and the current CPU frequency in MHz. Values that cannot be obtained
on the host SHALL be reported as `unknown` or `0` with a `units` label rather
than failing the call.

### FR-006 — Memory and uptime report (Ubiquitous)

The system SHALL report total physical memory, available physical memory,
total swap, free swap (all in bytes, rendered additionally as GiB in the text
format to 2 decimal places), system uptime in seconds (rendered additionally
as days/hours/minutes in the text format), and boot time as a UTC timestamp.

### FR-007 — Process environment report (Ubiquitous)

The system SHALL report the ragent process id, the current working directory
of the ragent process, the resolved user shell (`$SHELL` on Unix, `COMSPEC`
on Windows, `unknown` when unset), and the username of the user running
ragent (`unknown` when it cannot be determined).

### FR-008 — Sensitive-data exclusion (Unwanted)

The system SHALL NOT include environment variables (other than the resolved
shell path), filesystem contents, registry values, or any credential material
in the report. IF a data source could expose secrets (e.g. reading the full
environment block), THEN the tool SHALL exclude it from the output by design,
and the tool description SHALL state the exclusion.

### FR-009 — Read-only guarantee (Ubiquitous)

The tool SHALL be read-only: it SHALL NOT create, modify, or delete any file;
SHALL NOT make network requests; and SHALL NOT mutate session or global state.
It SHALL declare `permission_category() == "none"` and an empty
`path_pattern()` so the permission engine never prompts the user for an
`os_info` call. The only external commands it may run SHALL be the fixed
graphics-diagnostics allowlist of FR-020; every other data source SHALL be
`sysinfo`, `std::env`, or a read-only file. When the graphics diagnostics are
skipped (FR-020 `probe: false`) the tool spawns no process at all.

### FR-010 — No user-controlled input paths (Ubiquitous)

The tool SHALL accept only the optional `format` parameter and the optional
boolean `probe` parameter; it SHALL NOT accept any parameter that selects
files, commands, hosts, or the individual diagnostics to run, so there is no
user-controlled input surface beyond output formatting and whether the fixed
diagnostics allowlist runs. The parameters schema SHALL set
`additionalProperties: false`.

### FR-011 — Failure resilience (Optional)

WHERE the underlying platform query library fails to retrieve a specific
metric on an exotic or restricted platform, the tool SHALL still succeed and
report that individual field as `unknown` (or the documented placeholder), so
`os_info` never errors out on a partial-information host.

### FR-012 — TUI presentation (Ubiquitous)

The system SHALL display `os_info` tool calls in the TUI log panel with the
tool name and, for `input_summary`, a compact note such as
`format: text`/`format: json` (or `(no parameters)` when omitted), consistent
with the existing `ragent_info` rendering tests.

### FR-013 — `/osinfo` slash command registration (Ubiquitous)

The system SHALL register an `osinfo` slash command in the TUI
`SLASH_COMMANDS` table, in the autocomplete suggestion map, and in the
`/help` command index, so that `/osinfo` is recognised by
`execute_slash_command_inner` and appears in the slash menu alongside the
other read-only introspection commands.

### FR-014 — `/osinfo` dispatch routing (Event-driven)

WHEN the user submits `/osinfo <args>`, the system SHALL route to a
`handle_osinfo_command` dispatcher that matches the first whitespace-separated
token (lowercased): empty string and `help`/`--help`/`-h` SHALL render the
help page (FR-015); `show` SHALL render the OS information report (FR-016),
honouring a following `--no-probe` (or `--read-only`) flag by taking the
process-free collection path; any other token SHALL render an
unknown-subcommand correction message that lists the valid subcommands
(`help`, `show`).

### FR-015 — `/osinfo help` page (Ubiquitous)

The system SHALL provide a `/osinfo help` page that documents every
subcommand in a table (`help`, `show`), states that the report is read-only,
explains that `show` runs the host's bounded graphics-diagnostics allowlist by
default so every detected graphics API carries its version (FR-021) and that
`--no-probe` skips it for a fully process-free report, and shows at least one
worked example. The page SHALL be prefixed `From: /osinfo help` and the status
bar SHALL read `osinfo: help`.

### FR-016 — `/osinfo show` report (Event-driven)

WHEN the user submits `/osinfo show`, the system SHALL render, as an
assistant message in the message window, a displayable version of exactly the
same information the `os_info` tool collects — OS identity (including the
Linux distribution detail of FR-003), CPU, graphics adapters and graphics-API
versions (FR-019, FR-020, FR-021), memory/uptime, and process
environment — using the tool's own data collection and text renderer rather
than a parallel reimplementation. The message SHALL be prefixed
`From: /osinfo show` and the status bar SHALL read `osinfo: show` on
completion. The report SHALL render the text (human-readable) form by
default. `show` SHALL run the graphics diagnostics by default so every
detected API carries its version (FR-021); a trailing `--no-probe` flag SHALL
force the process-free collection path (`probe: false`) instead.

### FR-017 — Slash-command read-only guarantee (Ubiquitous)

The `/osinfo` command SHALL be read-only: it SHALL NOT create, modify, or
delete any file; SHALL NOT make network requests; SHALL NOT mutate session,
global, or configuration state; and SHALL NOT trigger a permission prompt. The
only external commands it may run SHALL be the fixed graphics-diagnostics
allowlist of FR-020 (skipped entirely by `/osinfo show --no-probe`), which
only read host state. The same sensitive-data exclusions as FR-008 SHALL apply
to the rendered report.

### FR-018 — Single source of truth (Unwanted)

The system SHALL NOT maintain a second copy of the OS data-collection or
text-rendering logic for the slash command. IF the slash command and the
`os_info` tool were to diverge in collected fields or formatting, THEN the
defect SHALL be prevented by construction: `/osinfo show` SHALL call into the
same collector/renderer used by `OsInfoTool`, and field placeholders
(`unknown`, `n/a (<family>)`) SHALL render identically on both paths.

### FR-019 — Graphics adapter (GPU) report (State-driven)

The system SHALL report every detectable graphics adapter, including integrated
graphics, as a list. Each adapter SHALL report a best-effort human-readable name
(e.g. `Intel Corporation TigerLake-LP GT2 [Iris Xe Graphics]`), the vendor name,
the PCI vendor id and device id as four lowercase hex digits, the bound kernel
driver (e.g. `i915`), a coarse kind classification (`integrated`, `discrete`,
`virtual`, or `unknown`), and dedicated video memory in bytes (`0` when the host
exposes none).

WHILE the host OS family is Linux, adapters SHALL be enumerated from the
read-only DRM sysfs tree under `/sys/class/drm` (each `cardN` node), with names
resolved from the read-only system `pci.ids` database when it is present. On
non-Linux platforms, or on a Linux host with no DRM card nodes, the adapter list
SHALL be empty. Values that cannot be obtained SHALL be reported as `unknown`
(or `0`) rather than failing the call (FR-011).

The text format SHALL render the adapters under a `## GPU` section that is
always present: one bullet group per adapter, or a single
`- **Adapters**: unknown` line when the list is empty. The JSON format SHALL
expose a `gpus` array (empty when no adapter is detected) whose elements carry
the keys `name`, `vendor`, `vendor_id`, `device_id`, `driver`, `kind`, and
`vram_bytes`. GPU enumeration SHALL remain read-only: it reads only existing
`/sys` and `pci.ids` files and SHALL NOT spawn a process, write a file, or make
a network request (FR-009).

### FR-020 — Graphics-API support report (State-driven)

The system SHALL report the graphics programming APIs the host can support, as a
list drawn from the fixed vocabulary `Direct3D`, `DirectX`, `Metal`, `OpenGL`,
`OpenGL ES`, `Mesa`, `Vulkan`, `OptiX`, `CUDA`, and `ROCm`, emitted in that
order. The list SHALL be scoped to the host platform: `Direct3D` and `DirectX`
SHALL be reported only on Windows, `Metal` only on macOS, and every other API on
any platform.

Support SHALL first be inferred read-only from installed runtimes (runtime
libraries, Vulkan ICD manifests, or vendor tools under well-known locations)
without writing a file or making a network request (FR-009). In addition, and
by default, the system SHALL run a fixed, timeout-bounded allowlist of vendor
diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`,
`system_profiler`) to confirm support; each diagnostic SHALL be run at most
once per report and SHALL degrade to "no signal" on a missing tool, a non-zero
exit, or a timeout rather than failing the call (FR-011). WHEN the agent calls
`os_info` with `{"probe": false}` (or runs `/osinfo show --no-probe`) the
system SHALL skip those diagnostics and report from the read-only inference
alone, spawning no process. The tool description SHALL state that the default
runs the diagnostics and that `probe: false` avoids spawning any process.

The text format SHALL render the APIs under the `## GPU` section as a single
`- **APIs**: <comma-separated list>` line (or `- **APIs**: none detected` when
empty), where each entry is the API name followed by its version when known
(FR-021), e.g. `Vulkan 1.4.354`, and the bare name when the version is unknown.
The JSON format SHALL expose a `gpu_apis` array, empty when none is detected,
whose elements are objects carrying `name` and `version` (FR-021).

### FR-021 — Graphics-API version report (State-driven)

Each detected graphics API SHALL carry a best-effort version string, reported as
the `version` field of the `gpu_apis` JSON element and appended to the API name
in the text `- **APIs**` line (`Vulkan 1.4.354`). The version SHALL be the API's
own version where the host exposes it (e.g. Vulkan `1.4.354`, OpenGL `4.6`,
OpenGL ES `3.2`), and SHALL be the `unknown` placeholder when no source carries
one (FR-011); a known version SHALL never be empty and SHALL never render the
`unknown` token next to the name.

The version SHALL be read from read-only artefacts first, without writing a
file or making a network request (FR-009): the Vulkan ICD manifest
`api_version`, the CUDA `version.json`/`version.txt` or `cuda.h`, the OptiX
`optix.h` header, the ROCm `.info/version` file, or the macOS Metal framework
`Info.plist`. In addition, and by default, the version from any successful
vendor diagnostic SHALL be used where the read-only artefacts carry none:
`glxinfo -B` for OpenGL, OpenGL ES, and Mesa (which expose no read-only
version file, so the diagnostic is their only source), `vulkaninfo` for Vulkan,
`nvidia-smi` for CUDA, and vendor tools for the remaining APIs. WHEN the agent
calls `os_info` with `{"probe": false}` (or runs `/osinfo show --no-probe`) the
read-only artefacts SHALL be the only source and an API with none SHALL report
`unknown`. Probe output SHALL be captured once per report and SHALL degrade to
"no signal" on a missing tool, a non-zero exit, or a timeout (FR-011).

### FR-022 — Physical-hardware identity report (State-driven)

WHILE the host exposes a DMI/SMBIOS table (Linux `/sys/class/dmi/id`), the
system SHALL report the system (product) vendor, model, version and family; the
chassis type rendered from its SMBIOS code as a human label (e.g. `10` ->
`Notebook`) and the chassis vendor; the motherboard (baseboard) vendor, name and
version; and the firmware (BIOS) vendor, version and release date. On a host
without a DMI table (or a non-Linux platform) each field SHALL report the
`unknown` placeholder so the output schema is stable across platforms (FR-011).

### FR-023 — Physical storage-device report (State-driven)

WHILE the host exposes block-device sysfs (Linux `/sys/block`), the system SHALL
report the list of physical storage devices, one entry per non-virtual block
device, each with its kernel name (e.g. `nvme0n1`, `sda`), best-effort vendor and
model, capacity in bytes, and a coarse kind (`nvme`, `ssd`, `hdd`, or
`unknown`; `nvme` is identified by the kernel-name prefix, `hdd`/`ssd` from the
`queue/rotational` hint, and `unknown` when it is absent). Virtual and
removable pseudo-devices (RAM disks, zram/swap,
loopback, device-mapper, software RAID, and optical drives) SHALL be excluded.
Devices SHALL be ordered by kernel name so the list is stable across calls. A
host with no readable block-device tree SHALL report an empty list rather than
failing the call (FR-011).

### FR-024 — Network-interface report (State-driven)

WHILE the host exposes network sysfs (Linux `/sys/class/net`), the system SHALL
report the list of network interfaces, one entry per interface, each with its
name, MAC address, operational state (e.g. `up`, `down`, `unknown`), link speed
in Mbit/s (`0` when the host does not expose one), and a boolean loopback flag.
Interfaces SHALL be ordered by name so the list is stable across calls. A host
with no readable network tree SHALL report an empty list rather than failing the
call (FR-011).

### FR-025 — Sensitive hardware identifiers excluded (Unwanted)

The system SHALL NOT read or report hardware serial numbers, product/chassis/
board UUIDs, or asset tags (e.g. DMI `product_serial`, `product_uuid`,
`board_serial`, `chassis_serial`, `*_asset_tag`), so a report can be shared
without leaking uniquely identifying hardware material. IF such a data source
exists on the host, THEN the collector SHALL not request it by design, and the
tool description SHALL state the exclusion.

## Non-Functional Requirements

- **NFR-001 — Performance:** a `probe: false` call SHALL complete in under
  250 ms on a typical workstation (single `sysinfo` refresh of the metrics it
  reports; no repeated or sampling-style refreshes; graphics-API inference is a
  bounded set of read-only path checks). The default call runs the timeout-bounded
  vendor diagnostics and is exempt from the 250 ms budget.
- **NFR-002 — Dependencies:** runtime OS data SHALL come from `sysinfo`
  (cross-platform, pure Rust, no shell-out); process data SHALL come from
  `std::env`. GPU identity SHALL come from read-only `/sys/class/drm` sysfs
  files with `pci.ids` name resolution on Linux, the physical-hardware identity,
  storage, and network fields from read-only `/sys/class/dmi/id`, `/sys/block`,
  and `/sys/class/net` on Linux, and graphics-API inference from
  read-only runtime file checks; none of these SHALL add a new crate dependency (the
  opt-in `probe` diagnostics use `std::process`). The `sysinfo` dependency is
  added to `ragent-agent`'s `Cargo.toml` with pinned features (`system` only) to
  keep compile cost low.
- **NFR-003 — Documentation:** the tool description SHALL enumerate the
  reported categories, state that the tool is read-only, makes no network or
  filesystem writes, sets `additionalProperties: false`, and that the tool
  always succeeds. The description SHALL also state that `probe: true` spawns the
  vendor diagnostics.
- **NFR-004 — Shared rendering performance:** `/osinfo show` SHALL reuse the
  tool's collector and text renderer (NFR-002 dependencies) and SHALL respect
  the same sub-250 ms budget as an `os_info` tool call (NFR-001); it SHALL
  NOT trigger a model/LLM call.

## Acceptance Criteria

1. `os_info` appears in the registered-tool list and `/tools` output.
2. Calling `os_info` (no args) returns a markdown report containing OS name,
   OS version, kernel version, hostname, CPU arch/cores/brand, a GPU section
   (graphics-API list plus adapter list, or the `unknown` placeholder), a
   Hardware section (system/chassis/board/BIOS identity plus a storage list and a
   network list, or their `none detected` placeholders), memory
   totals,
   uptime, working directory, shell, and username — with the Linux
   distribution section present on Linux.
3. `{"format": "json"}` returns a JSON object with stable keys for all of the
   above (including a `gpus` array, a `gpu_apis` array whose elements carry
   `name` and `version`, and `storage_devices`/`network_interfaces` arrays whose
   elements carry the documented keys), and never a serial/UUID/asset-tag key
   (FR-025).
4. An `os_info` call with no `probe` argument (or `probe: false`) creates no
   file, spawns no process, and never prompts the permission engine; only
   `{"probe": true}` runs the allowlisted, timeout-bounded vendor diagnostics.
5. On non-Linux hosts the distribution fields render as `n/a` and the call
   still succeeds.
6. `/osinfo` appears in the slash menu and `/help` index; `/osinfo` (no
   subcommand) and `/osinfo help` render the help page prefixed
   `From: /osinfo help`.
7. `/osinfo show` renders the same text report as an `os_info`
   `format: text` call, prefixed `From: /osinfo show`, with no LLM call, no
   writes, and no permission prompt.
