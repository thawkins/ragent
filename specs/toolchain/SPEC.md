---
status: draft
audit:
  - { time: 1789032067, from: "none", to: "draft", actor: "system" }
---
# /toolchain — Runtime Toolchain Presence Report

## Overview

The code index (`ragent-codeindex::scanner`) maintains `SUPPORTED_LANGUAGES`, a
50-entry list of language identifiers it can parse (50 canonical entries
including dialect/header aliases such as `tsx`, `jsx`, `c_header`,
`cpp_header`). The entries split naturally into two groups:

- **Application languages** (30 entries) — languages whose source is compiled
  or interpreted by a runtime toolchain (e.g. `rust` needs `cargo`/`rustc`,
  `python` needs `python3`, `go` needs `go`).
- **Data/format entries** (20 entries) — pure data, markup, or build-DSL
  formats that have no source runtime (`toml`, `yaml`, `json`, `xml`, `html`,
  `css`, `scss`, `sql`, `markdown`, `protobuf`, `verilog`, `vhdl`,
  `terraform`, `openscad`, `cmake`, `gradle`, `gradle_kts`, `maven`, `nix`,
  `hcl`).

Today there is no way to see, from inside ragent, which of these runtimes are
actually installed on the local machine and at what version. This spec defines
a new read-only slash command, `/toolchain`, that walks
`SUPPORTED_LANGUAGES`, and for each non-data entry determines the runtime
support required, probes the local system for it, and reports presence and
version. A `help` subcommand documents the surface.

## Scope

In scope:

- `/toolchain help` (and `/toolchain` with no subcommand) — usage reference.
- `/toolchain list` — full report over every non-data entry of
  `SUPPORTED_LANGUAGES`: language, required runtime command(s),
  installed/absent status, and installed version.
- `/toolchain list <lang>` — filtered report for one language id.
- `/toolchain list --json` — machine-readable JSON rendering of the report.
- Classification of every `SUPPORTED_LANGUAGES` entry as application or
  data/format, held in a single mapping table.
- Non-blocking execution so the TUI never stalls on version probes.

Out of scope:

- Installing, upgrading, or configuring any runtime (reporting only).
- Probing toolchains for data/format entries (skipped by design).
- HTTP/REST endpoints or CLI subcommand surfaces (TUI-only; may be added later).
- Detecting project-local toolchains (`.tool-versions`, `mise`, nix shells, venvs).

## Background

Key types and APIs the implementation will use:

- `ragent_codeindex::scanner::SUPPORTED_LANGUAGES` — the canonical
  `&[&str]` list of 50 language ids. The toolchain report must derive its
  language set from this const (no second hard-coded list), so the two can
  never drift.
- New module `crates/ragent-tui/src/app/toolchain.rs` (or equivalent) holding:
  - the language → runtime mapping const,
  - the probe helpers (`presence` via PATH lookup, `version` via
    `<tool> <version-flag>` with a timeout),
  - the report renderer (text table + JSON).
- Slash-command plumbing in `crates/ragent-tui/src/app/slash.rs`:
  - a `"toolchain" => self.handle_toolchain_command(args)` arm in
    `execute_slash_command_inner`,
  - a `"toolchain"` entry in `get_command_suggestions` for autocomplete.
- The TUI is a single-threaded UI loop; blocking probes (process spawn +
  wait) must run on a blocking thread (`tokio::task::spawn_blocking`) with
  the UI status set to `[wait]` until the report is ready, following the
  existing async-slash-command pattern.

Version probing notes (captured so implementation does not re-derive them):

- Most tools report version on stdout with `--version`; notable exceptions
  that must carry a per-tool flag: `java` (`-version`, stderr), `erl`
  (`-version`, stderr), `R` (`--version`, first line), `node`
  (`--version`, prints `vX.Y.Z`).
- `dotnet` is probed with `--list-sdks` and `--list-runtimes` (not
  `--version`, which reports only the active SDK and fails on runtime-only
  installs); the first line of each listing is joined into one version
  string.
- Probes should capture both stdout and stderr and take the first
  non-empty line, trimmed.
- Presence is checked with a PATH lookup (`command -v <tool>` semantics via
  `std::process::Command` on `sh -c`, or an equivalent pure-Rust PATH walk).
  No shell injection risk: tool names are compile-time constants.

## Requirements

### FR-001 — Slash command registration (Ubiquitous)

The system SHALL register a `toolchain` slash command in the TUI
`SLASH_COMMANDS` table and in the autocomplete suggestion map, so that
`/toolchain` is recognised by `execute_slash_command_inner` and appears in the
slash menu and the `/help` command index.

### FR-002 — Dispatch routing (Event-driven)

WHEN the user submits `/toolchain <args>`, the system SHALL route to a
`handle_toolchain_command` dispatcher that matches the first whitespace-
separated token (lowercased): empty string and `help`/`--help`/`-h` SHALL
render the help page (FR-003); `list` SHALL render the toolchain report
(FR-004–FR-009); any other token SHALL render the unknown-subcommand
correction message (FR-014).

### FR-003 — Help page (Ubiquitous)

The system SHALL provide a `/toolchain help` page that documents every
subcommand in a table (`help`, `list`), the `list` argument forms
(`[language]`, `--json`), states that the report is read-only, and shows at
least one worked example. The page SHALL be prefixed `From: /toolchain help`
and the status bar SHALL read `toolchain: help`.

### FR-004 — Walk the supported-language list (Event-driven)

WHEN the user submits `/toolchain list`, the system SHALL iterate
`ragent_codeindex::scanner::SUPPORTED_LANGUAGES` in list order and emit one
report row per entry. The language set SHALL be derived from the const at
runtime; the implementation SHALL NOT maintain a second hard-coded language
list.

### FR-005 — Non-data classification (Ubiquitous)

The system SHALL classify every `SUPPORTED_LANGUAGES` entry via a single
mapping table as either an application language (runtime probed) or a
data/format entry (runtime skipped). The mapping SHALL cover all 50 canonical
ids and SHALL be exhaustive — every entry in `SUPPORTED_LANGUAGES` MUST have
a mapping row, so adding a language to the scanner const without adding a
mapping is a compile/CI-detectable gap. The initial partition SHALL be:

- **Application (30):** `rust`, `python`, `typescript`, `tsx`, `javascript`,
  `jsx`, `go`, `c`, `c_header`, `cpp`, `cpp_header`, `java`, `kotlin`,
  `ruby`, `swift`, `csharp`, `lua`, `shell`, `zsh`, `fish`, `zig`, `nim`,
  `elixir`, `erlang`, `haskell`, `ocaml`, `r`, `dart`, `php`, `perl`.
- **Data/format (20):** `toml`, `yaml`, `json`, `xml`, `html`, `css`,
  `scss`, `sql`, `markdown`, `protobuf`, `verilog`, `vhdl`, `terraform`,
  `openscad`, `cmake`, `gradle`, `gradle_kts`, `maven`, `nix`, `hcl`.

Data/format rows SHALL still appear in the full `list` report, marked
`(data format — runtime n/a)`, so the user can see the full language surface;
only application rows carry presence/version columns.

### FR-006 — Runtime mapping (Ubiquitous)

For each application language the system SHALL specify the runtime command(s)
that must be present. The initial mapping SHALL be:

| Language id(s) | Runtime probe(s) |
|---|---|
| `rust` | `cargo`, `rustc` |
| `python` | `python3` |
| `typescript`, `tsx` | `node`, `tsc` |
| `javascript`, `jsx` | `node` |
| `go` | `go` |
| `c`, `c_header` | `cc` (fallback `gcc`, then `clang`) |
| `cpp`, `cpp_header` | `c++` (fallback `g++`, then `clang++`) |
| `java` | `java`, `javac` |
| `kotlin` | `kotlinc` |
| `ruby` | `ruby` |
| `swift` | `swift` |
| `csharp` | `dotnet` |
| `lua` | `lua` (fallback `luajit`) |
| `shell` | `bash` |
| `zsh` | `zsh` |
| `fish` | `fish` |
| `zig` | `zig` |
| `nim` | `nim` |
| `elixir` | `elixir` |
| `erlang` | `erl` |
| `haskell` | `ghc` (fallback `runghc`) |
| `ocaml` | `ocaml` |
| `r` | `R` |
| `dart` | `dart` |
| `php` | `php` |
| `perl` | `perl` |

A language row SHALL report **installed** only when the primary probe (or a
fallback, when declared) is found; multi-runtime rows (`rust`, `java`,
`typescript`/`tsx`) SHALL list the per-command status.

### FR-007 — Presence probe (Event-driven)

WHEN a runtime command is probed, the system SHALL determine presence by
resolving the command name against the `PATH` environment variable (same
semantics as `command -v`), without spawning the tool. The probe result SHALL
be `installed` (binary found) or `not installed` (not found), and MUST NOT
depend on network access.

### FR-008 — Version capture (Event-driven)

WHEN a runtime command is found installed, the system SHALL execute
`<command> <version-flag>` (per-tool flag from the version-probing notes;
default `--version`), capture the first non-empty line from stdout or
stderr, and report it as the version string, trimmed and rendered on a
single line. WHERE a tool requires multiple probes (`dotnet`), the first
line of each probe SHALL be joined into one `, `-separated version string.
The version probe MUST be bounded by a per-probe timeout (FR-012).

### FR-009 — Report rendering (Event-driven)

WHEN the report is produced, the system SHALL render a markdown table in the
message window with columns **Language**, **Runtime**, **Status**, **Version**
(`Status`: `installed` / `not installed`; `Version`: version string or
`-`), prefixed `From: /toolchain list`, plus a summary line (`<n>/<m>`
application runtimes installed). The status bar SHALL read
`toolchain: list`. Data-format rows SHALL use the FR-005 marker in place of
presence/version.

### FR-010 — Absent runtime handling (State-driven)

WHEN a runtime command is not found on `PATH`, the system SHALL report
`not installed` with a `-` version placeholder and SHALL continue probing the
remaining languages — a missing runtime MUST NOT abort or truncate the report.

### FR-011 — Language filter (Optional)

WHERE the user supplies a language id (`/toolchain list <lang>`), the system
SHALL report only that language's row, matched case-insensitively against the
`SUPPORTED_LANGUAGES` ids. WHEN the supplied id is not in
`SUPPORTED_LANGUAGES`, the system SHALL warn that the id is unknown, echo the
valid id list, and produce no report table.

### FR-012 — Probe failure containment (Unwanted)

IF a version probe fails to start, exits non-zero without usable version
text, or exceeds the per-probe timeout (2 seconds), THEN the system SHALL
report the runtime as `installed` with version `unknown` (or `timeout`) and
continue — no probe may block the TUI or crash the command.

### FR-013 — Read-only guarantee (Ubiquitous)

The system SHALL perform the entire `/toolchain` surface without writing to
the filesystem, mutating configuration, spawning installers, or altering the
code index. The only child processes executed are the version probes of
FR-008 and the `PATH` resolution of FR-007, which must not modify any state.

### FR-014 — Unknown subcommand (Unwanted)

IF the user submits `/toolchain <unknown>`, THEN the system SHALL render a
usage correction message (`From: /toolchain`, usage line, pointer to
`/toolchain help`), set the status bar to `toolchain: usage`, and NOT
dispatch any probe.

### FR-015 — JSON output (Optional)

WHERE the `--json` flag is supplied (`/toolchain list --json`, combinable
with a language filter), the system SHALL emit the report as a JSON document
(`{ "languages": [ { "id", "runtime", "status", "version" } ... ],
"installed", "total" }`) instead of the markdown table, suitable for piping
or LLM consumption.

### FR-016 — Non-blocking execution (Event-driven)

WHEN `/toolchain list` runs, the system SHALL execute the PATH walks and
version probes on a blocking thread and set the status bar to a wait
indicator (`[wait] toolchain`) until the report is ready, so the TUI event
loop remains responsive throughout the (potentially dozens of) child-process
invocations.

### FR-017 — Fixed-width table columns

The `/toolchain list` ASCII table SHALL use fixed column widths of 10
(Language), 10 (Runtime), 10 (Status), and 50 (Version) characters, so the
table stays compact. Language and Runtime cells wider than their column SHALL
be clipped to the column width; the Status and Version columns SHALL word-wrap
overflow text onto continuation grid lines (blank Language and Runtime cells,
and a blank Status cell on Version-only continuation lines) so no status or
version text is lost.

## Non-Functional Requirements

### NFR-001 — Performance

A full `list` report SHALL complete within 10 seconds on a typical Linux
workstation with the per-probe timeout of FR-012, even when several
toolchains are absent (absent runtimes cost one PATH walk each, no spawn).

### NFR-002 — Determinism

The report SHALL list languages in `SUPPORTED_LANGUAGES` order, and repeated
invocations in the same environment SHALL produce identical status/version
columns (barring concurrent PATH changes).

### NFR-003 — No shell injection surface

All spawned commands SHALL be compile-time constants from the FR-006 mapping;
user input is used only for the language filter match (FR-011) and never
interpolated into a spawned command line.

## Acceptance Criteria

- `/toolchain` and `/toolchain help` render the help page.
- `/toolchain list` renders the full 50-row table with correct
  installed/not-installed statuses matching a manual `command -v`/`--version`
  spot-check on the test machine.
- `/toolchain list rust` renders only the rust row.
- `/toolchain list nosuchlang` warns and lists valid ids.
- `/toolchain list --json` emits valid JSON with an entry per application
  language.
- No ragent state is modified by any `/toolchain` invocation.