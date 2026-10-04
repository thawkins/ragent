---
status: draft
---
# Manual Test Plan — os_info

Spec: `specs/osinfo/SPEC.md`

## Prerequisites

- A build of ragent that includes the `os_info` tool and the `/osinfo` slash
  command: `cargo build` from the workspace root (binary at
  `target/debug/ragent`).
- A configured LLM provider (any working provider with an API key or local
  model) so the agent loop can invoke the `os_info` tool. The `/osinfo` slash
  command tests do not require a provider call.
- The manual tests are written against a Linux host where noted; TC-006 and
  TC-012 cover non-Linux behaviour.
- Optional: the `/tools` and `/help` slash commands to verify registration
  without an LLM call.

## Test Cases

### TC-001 — `os_info` appears in the tool catalogue

**Preconditions:**
- ragent TUI is open with an active session.

**Steps:**
1. Press `/` to open the slash menu.
2. Type `tools` and select `/tools`, press `Enter`.
3. Scroll the rendered tool list to the `U`/utility section.

**Expected results:**
- `os_info` is listed exactly once, adjacent to `ragent_info`.
- Its description states it reports operating system type and configuration
  and that it is read-only.
- No config change, restart, or feature flag is required for it to appear.

---

### TC-002 — LLM invocation returns the full markdown report

**Preconditions:**
- ragent TUI is open; a working provider is configured.

**Steps:**
1. In the message input, type:
   `What OS is this machine running? Use the os_info tool to find out.`
2. Press `Enter` and wait for the agent loop to complete.
3. Scroll the message window to the `os_info` tool call entry and open its
   full result.

**Test data:**
- Prompt: `What OS is this machine running? Use the os_info tool to find out.`

**Expected results:**
- The permission system does NOT prompt for approval (category `none`).
- The tool result renders as markdown with sections `Operating System`,
  `CPU`, `GPU`, `Memory`, and `Process`.
- `Operating System` shows the OS name, kernel version, hostname, and (on
  Linux) a distribution line such as `fedora / Fedora Linux 41`.
- `CPU` shows the architecture (e.g. `x86_64`), physical and logical core
  counts, brand string, and frequency in MHz.
- `GPU` shows each detected graphics adapter (or the `Adapters: unknown`
  placeholder when none is detected), with the adapter name, vendor, PCI ids,
  bound driver, and kind (e.g. `Intel Corporation TigerLake-LP GT2 [Iris Xe
  Graphics]`, `integrated`, driver `i915`).
- `Memory` shows total/available memory and swap in bytes plus GiB values
  to 2 decimal places, plus uptime in human-readable d/h/m form.
- `Process` shows the ragent pid, the current working directory, the shell
  path (e.g. `/bin/bash`), and the username.
- The agent summarises the result coherently (e.g. "You are on Fedora Linux,
  x86_64, 16 cores, 62 GiB RAM").

---

### TC-003 — JSON output format

**Preconditions:**
- Same as TC-002.

**Steps:**
1. Type:
   `Call os_info with format json and show me only the JSON it returned.`
2. Press `Enter`.
3. Inspect the tool result in the log panel.

**Test data:**
- Prompt: `Call os_info with format json and show me only the JSON it returned.`

**Expected results:**
- The tool result is a single JSON object (no markdown table).
- Keys are snake_case and include at minimum: `os_family`, `os_name`,
  `os_version`, `kernel_version`, `hostname`, `distribution`,
  `cpu_arch`, `physical_cores`, `logical_cores`, `cpu_brand`, `gpus`,
  `gpu_apis`,
  `total_memory_bytes`, `available_memory_bytes`, `total_swap_bytes`,
  `uptime_seconds`, `boot_time_utc`, `pid`, `working_directory`, `shell`,
  `username`.
- `gpus` is an array (empty when no adapter is detected); each element carries
  `name`, `vendor`, `vendor_id`, `device_id`, `driver`, `kind`, and
  `vram_bytes`.
- `gpu_apis` is an array of `{name, version}` objects, empty when none is
  detected; each `name` is drawn from `Direct3D`, `DirectX`, `Metal`, `OpenGL`,
  `OpenGL ES`, `Mesa`, `Vulkan`, `OptiX`, `CUDA`, `ROCm`, scoped to the host
  platform, and each `version` is the API version or `unknown` (FR-021).
- No key contains environment-variable dumps, file contents, or secrets.
- The call succeeds with no error text.

---

### TC-004 — Invalid format value produces a clear error

**Preconditions:**
- Same as TC-002.

**Steps:**
1. Type: `Call os_info with format set to yaml.`
2. Press `Enter` and wait for the tool result.

**Test data:**
- Prompt: `Call os_info with format set to yaml.`

**Expected results:**
- The tool returns an error message naming the invalid value and listing the
  accepted values (`text`, `json`).
- The agent reports the error to you instead of inventing OS data.
- The session remains usable; the next prompt works normally.

---

### TC-005 — Read-only guarantee

**Preconditions:**
- ragent TUI is open in a writable project directory.

**Steps:**
1. Note the current time.
2. Type: `Call os_info three times in a row and summarise the OS.`
3. Press `Enter`; let all three calls complete.
4. In a separate terminal, run `git status --short` in the project root (or
   check the newest files in the directory).

**Test data:**
- Prompt: `Call os_info three times in a row and summarise the OS.`

**Expected results:**
- No new or modified files appear in the working tree.
- The tool never prompts for permission and never invokes `bash`.
- All three results are identical except for `uptime_seconds`/`boot_time`
  consistency and available-memory drift.

---

### TC-006 — Cross-platform behaviour (non-Linux host, tool)

**Preconditions:**
- Access to a macOS or Windows machine with the same ragent build installed.

**Steps:**
1. On the non-Linux host, open the ragent TUI.
2. Type: `Use os_info to tell me what platform this is.`
3. Press `Enter`.

**Test data:**
- Prompt: `Use os_info to tell me what platform this is.`

**Expected results:**
- The call succeeds; `os_family` reflects the host (`macos` or `windows`).
- The distribution field renders as `n/a (macos)` / `n/a (windows)` rather
  than being omitted or causing an error.
- Shell resolves via `$SHELL` on macOS and `COMSPEC` on Windows (or
  `unknown` when unset), and all other fields are populated or report the
  documented `unknown`/`0` placeholders.

---

### TC-007 — Log-panel presentation

**Preconditions:**
- ragent TUI is open; the log panel is visible.

**Steps:**
1. Trigger an `os_info` call as in TC-002.
2. Observe the step-numbered tool call entry in the log panel before opening
   the full result.

**Expected results:**
- The entry shows the tool name `os_info` with a compact summary
  (`format: text` or `format: json`, or `(no parameters)` when omitted).
- The pretty-printed JSON arguments show only the `format` key at most —
  no other parameters exist.

---

### TC-008 — `/osinfo` appears in the slash menu and help index

**Preconditions:**
- ragent TUI is open with an active session.

**Steps:**
1. Press `/` to open the slash menu.
2. Type `osinfo` and confirm `/osinfo` appears as a suggestion.
3. Press `Esc` to close the menu.
4. Type `/help` and press `Enter`.
5. Scroll the help index and locate the `/osinfo` row.

**Expected results:**
- `/osinfo` appears in the autocomplete suggestions as `osinfo` is typed.
- The `/help` index lists `/osinfo` with a short description mentioning OS
  type/configuration and read-only behaviour.

---

### TC-009 — `/osinfo` with no subcommand shows help

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/osinfo` in the input line (no trailing subcommand).
2. Press `Enter`.

**Expected results:**
- An assistant message appears prefixed `From: /osinfo help`.
- The message contains a table listing the subcommands `help` and `show`
  (rendered in the message window as an ASCII box).
- The status bar reads `osinfo: help`.
- The help page states the report is read-only and invokes no external
  commands, and shows at least one worked example (e.g. `/osinfo show`).
- No OS report output and no error text appear.

---

### TC-010 — `/osinfo help` shows the help page

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/osinfo help`.
2. Press `Enter`.

**Expected results:**
- Identical output to TC-009.
- Repeat with `/osinfo --help` and `/osinfo -h`; all three aliases produce
  the same help page.

---

### TC-011 — `/osinfo show` renders the OS report

**Preconditions:**
- ragent TUI is open (no provider call needed).

**Steps:**
1. Type `/osinfo show`.
2. Press `Enter`.
3. Scroll through the rendered assistant message.

**Expected results:**
- An assistant message appears prefixed `From: /osinfo show`.
- The status bar reads `osinfo: show` once rendering completes.
- The message is the human-readable (text) markdown report with sections
  `Operating System`, `CPU`, `GPU`, `Memory`, and `Process` — the same content
  an `os_info` tool call with `format: text` returns (compare with TC-002).
- On Linux the distribution line is present; values such as hostname,
  kernel version, core counts, GPU adapters, memory totals, uptime, working
  directory, shell, and username are populated.
- No permission prompt appears and no LLM call is made (the message lands
  immediately without streaming).

---

### TC-012 — `/osinfo show` cross-platform behaviour (non-Linux host)

**Preconditions:**
- Access to a macOS or Windows machine with the same ragent build installed.

**Steps:**
1. On the non-Linux host, open the ragent TUI.
2. Type `/osinfo show`.
3. Press `Enter`.

**Expected results:**
- The report renders with `os_family` reflecting the host and the
  distribution field as `n/a (macos)` / `n/a (windows)`.
- The output matches the `os_info` tool result on the same host, field for
  field, including `unknown`/`0` placeholders for unavailable metrics.

---

### TC-013 — Unknown `/osinfo` subcommand produces a correction

**Preconditions:**
- ragent TUI is open.

**Steps:**
1. Type `/osinfo frobnicate`.
2. Press `Enter`.

**Test data:**
- Input: `/osinfo frobnicate`

**Expected results:**
- An assistant message appears prefixed `From: /osinfo`, rejecting the unknown
  subcommand and listing the valid subcommands (`show`, `help`). FR-014
  requires the correction to list the valid subcommands; it does not require
  echoing the rejected token.
- The status bar reads `osinfo: usage`.
- No OS report is rendered.
- The session remains usable; the next command works normally.

---

### TC-014 — Slash-command read-only guarantee

**Preconditions:**
- ragent TUI is open in a writable project directory.

**Steps:**
1. In a separate terminal, run `git status --short` in the project root and
   note the output.
2. In the TUI, type `/osinfo show` and press `Enter`.
3. Repeat step 1.

**Expected results:**
- `git status --short` output is unchanged — no new or modified files.
- The command never prompts for permission, never invokes `bash`, and makes
  no network request (verify by observing no spinner/streaming activity and
  no permission countdown in the UI).

---

### TC-015 — Tool and slash command agree on content

**Preconditions:**
- ragent TUI is open with a working provider configured.

**Steps:**
1. Type `/osinfo show` and press `Enter`; note the rendered values.
2. Type: `Call os_info with format text and paste the raw result.`
3. Press `Enter`; compare the tool result with step 1.

**Expected results:**
- Both outputs contain the same sections, fields, and value formatting
  (including GiB rendering and `unknown`/`n/a` placeholders); only
  time-varying values (uptime, available memory) may differ.

---

### TC-016 — Graphics-API list is platform-scoped and ordered (FR-020)

**Preconditions:**
- Same as TC-002.

**Steps:**
1. Type: `Call os_info and show only the graphics-API line from the GPU section.`
2. Press `Enter`.
3. Type: `Call os_info with format json and show only the gpu_apis array.`
4. Press `Enter`.

**Expected results:**
  - The `## GPU` section carries an `- **APIs**: ...` line (or
    `- **APIs**: none detected` when no API is detected).
  - The `gpu_apis` array elements carry `name` values drawn only from `Direct3D`,
    `DirectX`, `Metal`, `OpenGL`, `OpenGL ES`, `Mesa`, `Vulkan`, `OptiX`, `CUDA`,
    `ROCm`, in that order.
  - On a non-Windows host no `Direct3D`/`DirectX` is reported; off macOS no
    `Metal` is reported.
  - The text line and the JSON array list the same names (each rendered with its
    version per TC-018).

---

### TC-017 — `probe` confirms support with the vendor allowlist (FR-020)

**Preconditions:**
  - Same as TC-002.
  - A host with at least one installed graphics runtime (e.g. a Vulkan ICD or an
    NVIDIA driver) to observe a probe-confirmed API.

**Steps:**
    1. Type: `Call os_info with format json and show the gpu_apis array.`
    2. Press `Enter`.
    3. Type:
       `Call os_info with {"format": "json", "probe": false} and show the gpu_apis array.`
    4. Press `Enter`; compare with step 2.

**Expected results:**
    - Both calls succeed with no error text.
    - The default call (step 1) runs only the allowlisted diagnostics
      (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`, `system_profiler`), at
      most once each, and never hangs (2 s timeout per tool), so every detected
      API carries a version (FR-021).
    - An absent tool, a non-zero exit, or a timeout degrades to "no signal" rather
      than failing the call.
    - The `probe: false` call (step 3) spawns no process and reports the
      filesystem-inferred set with versions from read-only files only, which is a
      subset of (or equal to) the probed result.

### TC-018 — Graphics-API versions are reported (FR-021)

**Preconditions:**
  - Same as TC-002.
  - A host with at least one installed graphics runtime (e.g. a Vulkan ICD
    manifest, or `glxinfo`/`nvidia-smi` for a probe) to observe a version.

**Steps:**
    1. Type:
       `Call os_info with format json and show only the gpu_apis array.`
    2. Press `Enter`.
    3. Type:
       `Call os_info with {"format": "json", "probe": false} and show only the gpu_apis array.`
    4. Press `Enter`; compare with step 2.

**Expected results:**
    - Every `gpu_apis` element carries a non-empty `version` string; it is the API
      version (`1.4.354`, `4.6`, `3.2`, ...) where a source supplies it, and
      `unknown` otherwise.
    - The text `- **APIs**` line appends the version to the name
      (`Vulkan 1.4.354`) and shows the bare name when the version is `unknown`.
    - The default call (step 1) reads read-only files (Vulkan ICD `api_version`,
      CUDA/ROCm/OptiX version files) and additionally spawns the allowlisted
      diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`,
      `system_profiler`) so APIs with no read-only artefact (OpenGL, OpenGL ES,
      Mesa) still get a version.
    - The `probe: false` call (step 3) reads only read-only files and spawns no
      process; an API with no read-only version source reports `unknown`.

---

### TC-019 — Physical-hardware identity is reported (FR-022)

**Preconditions:**
  - Same as TC-002.
  - A host with a DMI/SMBIOS table (`/sys/class/dmi/id`) to observe real values;
    a host without one should instead show the `unknown` placeholders.

**Steps:**
    1. Type:
       `Call os_info with format text and show only the Hardware section.`
    2. Press `Enter`.
    3. Type:
       `Call os_info with {"format": "json"} and show system_vendor, system_model, chassis_type, board_vendor, and bios_version.`
    4. Press `Enter`.

**Expected results:**
  - The text report contains a `## Hardware` section with `System Vendor`,
    `System Model`, `System Version`, `Product Family`, `Chassis`, `Chassis
    Vendor`, `Board Vendor`, `Board Name`, `Board Version`, `BIOS Vendor`,
    `BIOS Version`, and `BIOS Date` bullets.
  - `chassis_type` is a human label (`Notebook`, `Desktop`, ...), never a bare
    SMBIOS number; on a host with no DMI table every field is `unknown`.
  - The JSON keys carry the same values as the text bullets.

### TC-020 — Physical storage devices are reported (FR-023)

**Preconditions:**
  - Same as TC-002.
  - A host with at least one whole-disk block device (`/sys/block`) to observe an
    entry; a host with only virtual devices should show the placeholder.

**Steps:**
    1. Type:
       `Call os_info with format json and show only the storage_devices array.`
    2. Press `Enter`.
    3. Compare with `lsblk -d -o NAME,MODEL,ROTA,SIZE` run outside ragent.

**Expected results:**
  - `storage_devices` is an array; each element carries `name`, `model`,
    `vendor`, `size_bytes` (a number), and `kind`.
  - Virtual devices (`ram*`, `zram*`, `loop*`, `dm-*`, `md*`, `sr*`) are absent.
  - The text report lists each device as `- **<name>**: <vendor> <model>
    (<bytes> bytes, <kind>)`, or `- **Storage**: none detected` when the list is
    empty.
  - The device set matches `lsblk -d` (partitions excluded) and the kind matches
    the rotational hint (`nvme` for NVMe, `ssd`/`hdd` for SATA).

### TC-021 — Network interfaces are reported (FR-024)

**Preconditions:**
  - Same as TC-002.
  - A host with at least one network interface.

**Steps:**
    1. Type:
       `Call os_info with format json and show only the network_interfaces array.`
    2. Press `Enter`.
    3. Compare with `ip -brief link` run outside ragent.

**Expected results:**
  - `network_interfaces` is an array; each element carries `name`,
    `mac_address`, `oper_state`, `speed_mbps` (a number), and `is_loopback`.
  - The interface set matches `ip -brief link` (including `lo`, whose
    `is_loopback` is `true`).
  - The text report lists each interface as `- **<name>**: <mac> (state
    <state>, speed <speed|unknown>, loopback <bool>)`, or `- **Network**: none
    detected` when the list is empty.

### TC-022 — Hardware identifiers never leak (FR-025)

**Preconditions:**
  - Same as TC-002.

**Steps:**
    1. Type:
       `Call os_info with format json and list every key in the result.`
    2. Press `Enter`.
    3. Inspect the text report from `os_info` and `/osinfo show`.

**Expected results:**
  - No key contains `serial`, `uuid`, or `asset_tag`.
  - The text report renders no `Serial`, `UUID`, or `Asset Tag` label.
  - The read-only DMI serial/UUID/asset-tag files
    (`product_serial`, `product_uuid`, `board_serial`, `chassis_serial`,
    `*_asset_tag`) are never read (they are root-only and are not requested by
    the collector).

---

## Cleanup

- No files, processes, or network resources are created by these tests; no
  teardown is required.
- If a test build was created solely for this plan, it may be removed with
  `cargo clean` (optional).

---

## Manual Pass Record (T-016)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, 8 logical cores.

Every test case was executed against a ragent build carrying the `os_info`
tool and the `/osinfo` slash command. The tool-side cases (TC-001..TC-007,
TC-015 tool half) were exercised by invoking the registered `os_info` tool
through the default registry with a throwaway capture harness (since no LLM
provider is configured on this host, the agent-loop prompts of TC-002/TC-003
were reproduced by calling the tool with the equivalent `format` argument and
inspecting the raw result, which is the same string the model would receive).
The slash-command cases (TC-008..TC-014, TC-015 slash half) depend on the
`/osinfo help` and `/osinfo show` handler bodies, which are delivered by tasks
T-021/T-022 and are still skeletons at the time of this pass; those cases are
marked deferred and must be re-run in the T-025 pass after T-021/T-022 land.

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered`; `os_info` appears exactly once, `permission_category() == "none"`. |
| TC-002 | pass (tool half) | Text report rendered the four sections; distribution `fedora / Fedora Linux 44 (Workstation Edition)`; CPU `x86_64`, 4 physical / 8 logical cores, brand + `4100 MHz`; memory in bytes + GiB to 2 dp; uptime `0d 3h 11m`; process pid/cwd/shell/username populated. No permission prompt (category `none`). |
| TC-003 | pass (tool half) | JSON output is one flat object; all 23 documented snake_case keys present; no secret/env dump; no error. |
| TC-004 | pass (tool half) | `format: yaml` returned `invalid 'format' value 'yaml': accepted values are text, json`; `test_os_info_invalid_format_errors_clearly`. |
| TC-005 | pass | Three back-to-back calls returned identical values except time-varying `uptime_seconds`/`available_memory_bytes`; no files changed (`git status --short` identical before/after); no `bash` invoked; no permission prompt. |
| TC-006 | deferred | Requires a macOS/Windows host; not available. Placeholder logic is covered by the `n/a (<family>)` branch in `test_os_info_text_reports_os_identity`. |
| TC-007 | pass | Compact summary `format: text` / `format: json` / `(no parameters)`, and only the `format` key exists (`test_input_summary_os_info_tool_*`, 3 tests). |
| TC-008 | pass | `osinfo` in `SLASH_COMMANDS`; autocomplete offers `show`/`help`; `/help` index lists it (`test_osinfo_registration`, 4 tests). |
| TC-009 | deferred | `handle_osinfo_help` body is a T-021 skeleton. |
| TC-010 | deferred | `handle_osinfo_help` body is a T-021 skeleton. |
| TC-011 | deferred | `handle_osinfo_show` body is a T-022 skeleton. |
| TC-012 | deferred | Requires a non-Linux host and the T-022 handler. |
| TC-013 | partial | The T-020 dispatcher rejects unknown subcommands with a correction listing `help`/`show`; covered by the T-024 slash-command tests once landed. |
| TC-014 | deferred | Depends on the T-022 `show` handler. |
| TC-015 | pass (tool half) | The `os_info` tool and `/osinfo show` share `OsInfo::collect()` + `render_text()` (FR-018); the tool half renders identically. Slash half deferred to T-022/T-025. |

Deferred cases must be re-run in the T-025 pass after T-021/T-022 are
implemented.

---

## Read-only Verification Record (T-023)

Scope: FR-017 (slash command is 100 percent read-only) and NFR-004 (no model/
LLM call). Verified statically and dynamically on the Linux build host.

**Static review**

- `crates/ragent-agent/src/tool/os_info.rs` contains no `std::process::Command`,
  no `File`/`fs` write, and no network type/`reqwest`/socket use. The only
  `std::process` reference is `std::process::id()` (read-only PID, line 536).
- `handle_osinfo_command`, `handle_osinfo_help` and `handle_osinfo_show`
  (`crates/ragent-tui/src/app/slash.rs`) call only `append_assistant_text` and
  the shared `ragent_agent::tool::os_info::{OsInfo::collect, render_text}`
  pair; they never touch `session_processor`, the provider registry, or the
  permission checker.
- `os_info` is registered with `permission_category() == "none"`, so it can
  never raise a permission prompt (`crates/ragent-agent/src/tool/mod.rs`).

**Dynamic harness** (throwaway integration test, created then deleted)

Six invocations were driven through `App::execute_slash_command`:
`/osinfo show`, `/osinfo help`, `/osinfo`, `/osinfo --help`, `/osinfo -h`,
`/osinfo bogus`. Before and after the batch, the harness asserted:

- `permission_queue` and `question_queue` both empty - no permission or
  question prompt was raised (FR-017).
- `stream_in_bytes == 0`, `stream_out_bytes == 0`, `llm_request_stats`
  empty, `token_usage == (0, 0)` - no LLM request was issued (NFR-004).
- `agent_name` and `selected_model` unchanged - no agent/model switch.
- `git status --short` output byte-identical before and after - no file was
  created, modified, or deleted by any invocation (FR-017).

Result: **pass**. No permission prompt, no LLM call, no streaming, no token
consumption, and no working-tree change for any subcommand (including the
unknown-subcommand correction path). No process or network activity is
possible on this code path: the handlers hold no `Command`, socket, or HTTP
client and perform no filesystem write. TC-014 is satisfied.

---

## Manual Pass Record (T-025)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, 4 physical / 8 logical cores,
46.76 GiB RAM.

This pass re-runs every case the T-016 record left deferred (TC-009..TC-015)
now that the `/osinfo help` and `/osinfo show` handler bodies (T-021, T-022)
and the dispatch arm (T-019/T-020) have landed, and adds the `/osinfo` slash
cases to the plan. As in T-016 the host carries no configured LLM provider, so
the cases that would otherwise drive an agent-loop prompt are exercised by
calling the shared `os_info` collector/renderer directly and by driving
`App::execute_slash_command` through a throwaway harness (created then
deleted); the slash command itself issues no LLM call, so this is the same code
path the TUI executes on `Enter`. The automated slash-command suite
(`crates/ragent-tui/tests/test_osinfo_command.rs`, 5 tests) pins the same
behaviour in CI.

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered`; `os_info` present exactly once, `permission_category() == "none"`. |
| TC-002 | pass (tool half) | Text report rendered all four sections; distribution `fedora / Fedora Linux 44 (Workstation Edition)`; CPU `x86_64`, 4 physical / 8 logical cores, brand + frequency; memory in bytes + GiB to 2 dp; uptime `0d 3h 47m`; process pid/cwd/shell/username populated. No permission prompt (category `none`). |
| TC-003 | pass (tool half) | JSON output is one flat object; all 23 documented snake_case keys present; no secret/env dump; no error. |
| TC-004 | pass (tool half) | `format: yaml` returned `invalid 'format' value 'yaml': accepted values are text, json`; `test_os_info_invalid_format_errors_clearly`. |
| TC-005 | pass | Three back-to-back calls returned identical values except time-varying `uptime_seconds`/`available_memory_bytes`; no files changed (`git status --short` identical before/after); no `bash` invoked; no permission prompt. |
| TC-006 | deferred | Requires a macOS/Windows host; not available. The `n/a (<family>)` branch is covered by `test_os_info_text_reports_os_identity`. |
| TC-007 | pass | Compact summary `format: text` / `format: json` / `(no parameters)`, only the `format` key (`test_input_summary_os_info_tool_*`, 3 tests). |
| TC-008 | pass | `osinfo` in `SLASH_COMMANDS`; autocomplete offers `show`/`help`; `/help` index lists it (`test_osinfo_registration`, 4 tests). |
| TC-009 | pass | Bare `/osinfo` emitted `From: /osinfo help`, an ASCII subcommand table with the `show`/`help` rows, the read-only/no-external-commands note, and an Examples section; status `osinfo: help`; no report, no error (`test_osinfo_help_page_renders_and_sets_status`). |
| TC-010 | pass | `/osinfo help`, `/osinfo --help`, `/osinfo -h` (and the bare form) produced byte-identical help pages and status `osinfo: help` (`test_osinfo_help_aliases_render_identically`). |
| TC-011 | pass | `/osinfo show` emitted `From: /osinfo show`, the four sections `Operating System`/`CPU`/`Memory`/`Process` with populated hostname, kernel, core counts, memory totals (GiB), uptime, working directory, shell, username; status `osinfo: show`; message landed immediately with no streaming and no permission prompt (`test_osinfo_show_renders_report_and_sets_status`). |
| TC-012 | deferred | Requires a non-Linux host; not available. The `/osinfo show` body delegates to the same collector that the TC-006 placeholder path is unit-tested against, so no separate slash-command branch exists to vary by platform. |
| TC-013 | pass | `/osinfo frobnicate` emitted `From: /osinfo` with the correction `Unknown subcommand. Usage: \`/osinfo show\|help\``, status `osinfo: usage`, and no report (`test_osinfo_unknown_subcommand_lists_valid_and_sets_usage`). The token itself is not echoed; FR-014 requires the valid-subcommand list, not the rejected token. |
| TC-014 | pass | Re-confirmed: `permission_queue`/`question_queue` empty, `token_usage == (0, 0)`, `llm_request_stats` empty, `stream_in_bytes == 0`, `stream_out_bytes == 0`, agent/model unchanged, and `git status --short` byte-identical before and after driving `/osinfo show`, `/osinfo help`, `/osinfo`, `/osinfo --help`, `/osinfo -h`, `/osinfo bogus`. See also the read-only verification record above. |
| TC-015 | pass | FR-018 confirmed: the `/osinfo show` body, after normalising markdown (`trim`, drop blanks, `-` -> `*` bullet fold) and ignoring only volatile lines (uptime, available memory, CPU frequency), is set-equal to `ragent_agent::tool::os_info::render_text(&OsInfo::collect())` (`test_osinfo_show_matches_tool_renderer`). Both render the same sections, fields, GiB formatting, and `unknown`/`n/a` placeholders. |

Summary: 13 pass, 2 deferred (TC-006, TC-012 - both require a non-Linux host and
are covered by the platform-agnostic unit tests on the placeholder branch). No
case failed. The earlier T-016 "deferred" and "partial" rows for TC-009..TC-015
are superseded by this record.

---

## Manual Pass Record (T-030 — GPU section, FR-019)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, Intel integrated graphics
(`/sys/class/drm/card1` -> `0000:00:02.0`, driver `i915`).

Re-runs the cases affected by the new `## GPU` section (FR-019) against the same
harness as the T-025 record. The host has one DRM card, an Intel iGPU.

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered` still green; no registration change. |
| TC-002 | pass (tool half) | Text report now renders five sections including `## GPU`, listing `Intel Corporation TigerLake-LP GT2 [Iris Xe Graphics]`, vendor `Intel Corporation`, PCI ids `8086:9a49`, driver `i915`, kind `integrated`. |
| TC-003 | pass (tool half) | JSON output carries the new `gpus` array; the single element has `name`, `vendor`, `vendor_id`, `device_id`, `driver`, `kind`, `vram_bytes` keys (`test_os_info_json_format_is_structured`). |
| TC-011 | pass | `/osinfo show` now contains the `GPU` section with the same adapter data (`test_osinfo_show_renders_report_and_sets_status`). |
| TC-015 | pass | `test_osinfo_show_matches_tool_renderer` remains set-equal to the tool renderer, confirming the GPU section is shared rather than duplicated (FR-018). |

Summary: all re-run cases pass. The GPU section is populated on this host
(integrated Intel graphics); the empty-list `Adapters: unknown` path is pinned by
the `gpus` array-type and placeholder assertions in `test_os_info_tool.rs`.

---

## Manual Pass Record (T-033 — graphics-API support, FR-020)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, Intel integrated graphics.

Re-runs the cases affected by the new graphics-API list (FR-020).

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered` still green; the schema now also carries the optional `probe` boolean (`test_os_info_schema_only_exposes_format`). |
| TC-002 | pass (tool half) | The `## GPU` section now renders an `- **APIs**: ...` line before the adapter bullets; on this host the inferred list is `OpenGL, OpenGL ES, Mesa, Vulkan` (`libGL.so.1`, `libGLESv2.so.2`, `libGLX_mesa.so.0` and a non-empty `/usr/share/vulkan/icd.d` are present), with no `Metal`/`Direct3D`/`DirectX` (platform-scoped) and no `CUDA`/`ROCm`/`OptiX` (runtimes absent). |
| TC-003 | pass (tool half) | JSON output carries the new `gpu_apis` array; `test_os_info_json_format_is_structured` asserts it is always an array. |
| TC-011 | pass | `/osinfo show` renders the same `APIs` line (shared renderer, FR-018). |
| TC-015 | pass | `test_osinfo_show_matches_tool_renderer` remains set-equal to the tool renderer, confirming the API line is shared rather than duplicated. |
| TC-016 (new) | pass | `test_os_info_gpu_apis_are_platform_scoped` confirms the vocabulary, the fixed order, and platform scoping (no `Metal` off macOS, no `Direct3D`/`DirectX` off Windows), and that the text and JSON surfaces agree. |
| TC-017 (new) | pass | `{"probe": true}` is accepted; the default path spawns no process (filesystem inference only). The probe allowlist is exercised by the `probe: true` branch; a missing tool degrades to "no signal". |

Summary: all re-run cases pass, plus two new cases for FR-020. The graphics-API
list is populated on this host from filesystem inference alone; the empty-list
`- **APIs**: none detected` path is pinned by `test_os_info_gpu_apis_are_platform_scoped`.

---

## Manual Pass Record (T-034 — graphics-API versions, FR-021)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, Intel integrated graphics
(Mesa 26.2.3; `vulkaninfo` and `glxinfo` installed; no CUDA/ROCm/OptiX runtime).

Re-runs the cases affected by the new graphics-API version report (FR-021).

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered` still green; no registration or schema change. |
| TC-002 | pass (tool half) | The `## GPU` `- **APIs**` line now renders `OpenGL, OpenGL ES, Mesa, Vulkan 1.4.354` by default: the Vulkan version comes from the read-only ICD manifest `/usr/share/vulkan/icd.d/intel_icd.x86_64.json` (`api_version` `1.4.354`); the remaining APIs render bare because only a probe supplies their versions. |
| TC-003 | pass (tool half) | JSON `gpu_apis` elements are now objects carrying `name` and `version`; `test_os_info_json_format_is_structured` asserts both keys are present strings. |
| TC-011 | pass | `/osinfo show` renders the same versioned `APIs` line (shared renderer, FR-018). |
| TC-015 | pass | `test_osinfo_show_matches_tool_renderer` remains set-equal to the tool renderer. |
| TC-016 | pass | `test_os_info_gpu_apis_are_platform_scoped` now reads the `name` field of each object; vocabulary, order, and platform scoping unchanged. |
| TC-017 | pass | The probe allowlist now also lists `glxinfo -B`; `{"probe": true}` still spawns no process on the default path. |
| TC-018 (new) | pass | Default call: `Vulkan 1.4.354` (ICD manifest), other APIs bare. `probe: true` call: `OpenGL 4.6, OpenGL ES 3.2, Mesa 26.2.3, Vulkan 1.4.354` from `glxinfo -B`; `test_os_info_gpu_api_versions_are_reported` asserts every `version` is non-empty and any known version renders in the text line. |

Summary: all re-run cases pass, plus one new case for FR-021. Version evidence is
read-only on the default path (Vulkan ICD manifest) and probe-supplied on the
`probe: true` path (`glxinfo -B`); a host with no version source reports
`unknown` for the affected API and renders the bare name.

---

## Manual Pass Record (T-036 — probe default-on, FR-020/FR-021)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, Intel integrated graphics
(Mesa 26.2.3; `vulkaninfo` and `glxinfo` installed; no CUDA/ROCm/OptiX runtime).

Re-runs the cases affected by making the graphics diagnostics run by default.

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered` still green; `test_os_info_schema_only_exposes_format` confirms the schema still exposes only `format` and `probe`. |
| TC-002 | pass (tool half) | Default call now renders `- **APIs**: OpenGL 4.6, OpenGL ES 3.2, Mesa 26.2.3, Vulkan 1.4.354` - OpenGL/GLES/Mesa from `glxinfo -B`, Vulkan from the ICD manifest `/usr/share/vulkan/icd.d/intel_icd.x86_64.json`. |
| TC-003 | pass (tool half) | `gpu_apis` elements carry `name` and `version`; the new `test_os_info_probe_false_selects_process_free_pass` checks the `probe: false` shape and every version is non-empty. |
| TC-011 | pass | `/osinfo show` renders the same versioned `APIs` line (shared renderer, FR-018). |
| TC-015 | pass | `test_osinfo_show_matches_tool_renderer` remains set-equal to the probed tool renderer; the new `test_osinfo_show_no_probe_matches_process_free_tool_renderer` is set-equal to the `probe: false` renderer. |
| TC-016 | pass | `test_os_info_gpu_apis_are_platform_scoped` reads the `name` field; vocabulary, order, and platform scoping unchanged. |
| TC-017 | pass | Default call runs the allowlist (each tool at most once, 2 s timeout); `probe: false` spawns no process. |
| TC-018 | pass | Default: `OpenGL 4.6, OpenGL ES 3.2, Mesa 26.2.3, Vulkan 1.4.354`. `probe: false`: `OpenGL, OpenGL ES, Mesa, Vulkan 1.4.354` (only Vulkan has a read-only version file). `test_os_info_gpu_api_versions_are_reported` and `test_os_info_probe_false_selects_process_free_pass` assert every `version` is non-empty and any known version renders in the text line. |

Summary: all re-run cases pass. The default path now probes, so OpenGL, OpenGL
ES, and Mesa acquire a version; `{"probe": false}` (tool) and
`/osinfo show --no-probe` (slash command) stay process-free with
`Vulkan 1.4.354` from the ICD manifest as the only version.

---

## Manual Pass Record (T-041 — physical-hardware section, FR-022..FR-025)

Host: Linux (Fedora Linux 44), x86_64, i7-1165G7, LENOVO ThinkPad T14 Gen 2i
(`20W1S20H00`), Intel integrated graphics.

Re-runs the cases affected by the new `## Hardware` section (FR-022..FR-025)
against the same harness as the T-036 record.

| Case | Result | Evidence |
| ---- | ------ | -------- |
| TC-001 | pass | `test_os_info_is_registered` still green; no registration or schema change. |
| TC-002 | pass (tool half) | Text report now renders six sections including `## Hardware`: system vendor `LENOVO`, model `20W1S20H00`, version/family `ThinkPad T14 Gen 2i`, chassis `Notebook`, board `LENOVO 20W1S20H00 SDK0J40709 WIN`, BIOS `LENOVO N34ET71W (1.71 ) 05/09/2026`, plus storage `nvme0n1 SOLIDIGM SSDPFKNU020TZ` and `sda Seagate Fast SSD` and the `lo`/`enp0s31f6`/`wlp0s20f3` network interfaces. |
| TC-003 | pass (tool half) | JSON output adds the identity keys (`system_vendor`..`bios_date`), the `storage_devices` array, and the `network_interfaces` array (`test_os_info_json_format_is_structured`). |
| TC-011 | pass | `/osinfo show` renders the same `Hardware` section (shared renderer, FR-018). |
| TC-015 | pass | `test_osinfo_show_matches_tool_renderer` remains set-equal to the tool renderer, confirming the hardware section is shared rather than duplicated. |
| TC-019 (new) | pass | `test_os_info_text_reports_hardware_section` confirms the section and identity labels; `chassis_type` renders `Notebook` (SMBIOS code `10`), never the raw number. |
| TC-020 (new) | pass | Two physical devices detected (`nvme0n1`, `sda`); `zram0` correctly excluded. Each entry carries `name`/`model`/`vendor`/`size_bytes`/`kind` (`nvme`, `ssd`). |
| TC-021 (new) | pass | Three interfaces detected (`enp0s31f6`, `lo`, `wlp0s20f3`); `lo` carries `is_loopback: true`; each entry carries `name`/`mac_address`/`oper_state`/`speed_mbps`/`is_loopback`. |
| TC-022 (new) | pass | `test_os_info_hardware_excludes_sensitive_identifiers` confirms no `serial`/`uuid`/`asset_tag` key in JSON and no `Serial`/`UUID`/`Asset Tag` label in text; the root-only DMI serial/UUID/asset-tag files are never requested. |

Summary: all re-run cases pass, plus four new cases for FR-022..FR-025. The
hardware section is populated on this host (LENOVO ThinkPad T14 Gen 2i, one NVMe
and one SATA SSD, three network interfaces); the empty-list `none detected`
placeholders are pinned by the array-type and placeholder assertions in
`test_os_info_tool.rs`.
