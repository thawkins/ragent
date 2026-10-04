# /osinfo

> Report host OS and hardware info: /osinfo show [--no-probe]|help

## Overview

`/osinfo` prints a read-only report about the host machine ragent is running
on: OS identity (family, name, version, kernel, hostname, and the Linux
distribution), CPU (architecture, physical/logical cores, vendor, brand,
frequency), graphics adapters (integrated and discrete, with vendor, PCI ids,
driver, class, and VRAM where exposed) and the graphics-API versions the host
supports (Direct3D, DirectX, Metal, OpenGL, OpenGL ES, Mesa, Vulkan, OptiX,
CUDA, ROCm), physical hardware (system vendor/model/version and family, chassis
type and vendor, motherboard, BIOS vendor/version/date, storage devices, and
network interfaces), memory and swap, uptime and boot time, and the ragent
process environment (pid, working directory, shell, username).

The report is produced by the `os_info` tool's own collector and text renderer,
so the slash command and the tool always agree. It writes no file, makes no
network request, consults no provider, and never reports serial numbers, UUIDs,
or asset tags - a report can be shared safely.

## Syntax

```
/osinfo
/osinfo show
/osinfo show --no-probe
/osinfo help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/osinfo` or `/osinfo help` | Print the `/osinfo` help page (also `--help` / `-h`). |
| `/osinfo show` | Render the host OS and hardware report. Runs the bounded graphics diagnostics by default. |
| `/osinfo show --no-probe` | Render the report without spawning any external command (fully process-free). `--read-only` / `--no-diagnostics` are accepted aliases. |

## Collection modes

OpenGL, OpenGL ES, and Mesa expose no read-only version artefact, so they can
only be read from a live diagnostic. By default (`probe` = `true`) `/osinfo
show` runs a fixed, timeout-bounded allowlist of the host's own vendor
diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`,
`system_profiler`), each at most once per report under a hard timeout, so every
graphics API is reported with its version. The diagnostics only *read* host
state (no writes, no network).

Pass `--no-probe` for a fully process-free pass that runs no external command
and relies only on read-only files and installed runtimes; APIs whose version
only a live diagnostic reports are then omitted.

## Examples

```
/osinfo show
```

Renders the report, including each graphics API's version, into the message
window:

```
## OS
- Family: linux
- Name: Linux
- Version: 6.11.0-19-generic
- Kernel: 6.11.0-19-generic
...
## GPU
- Adapters:
  - Intel Corporation TigerLake-LP GT2 [Iris Xe Graphics] (driver i915, integrated)
- APIs: OpenGL 4.6, OpenGL ES 3.2, Mesa 26.2.3, Vulkan 1.4.354
```

```
/osinfo show --no-probe
```

Prints the same report without spawning any diagnostics process.

## Output

- An assistant message bubble titled `From: /osinfo show`.
- Status bar shows `osinfo: show` (or `osinfo: help` for the help page).
- No permission prompt is raised and no LLM call is made.

## Related

- `os_info` - the tool that produces the same report for the agent
- `/about` - ragent binary version and build information
- `/doctor` - provider, git, ripgrep, MCP, and memory diagnostics
