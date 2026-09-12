# /toolchain

> Language toolchain report: /toolchain list [lang] [--json] | /toolchain help

## Overview

`/toolchain` reports the presence of language toolchains detected on this
machine: which runtimes and compilers are installed, their versions, and their
readiness status. It is strictly read-only - it never installs, upgrades, or
configures anything; it only probes. For example, `/toolchain list rust`
probes `cargo` and `rustc` and reports what was found.

## Syntax

```
/toolchain
/toolchain help
/toolchain list
/toolchain list <language>
/toolchain list <language> --json
/toolchain list --json
```

Notes:

- The bare form is equivalent to `/toolchain help`.
- `--json` is position-independent: it may appear before or after the language
  filter.
- The language filter is case-insensitive.
- Probing is blocking; while it runs the status line shows
  `[wait] toolchain status`.

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `/toolchain` (or `/toolchain help`) | Print the help page. |
| `/toolchain list` | Probe and report every known language toolchain. |
| `/toolchain list <language>` | Probe and report only the given language (case-insensitive id). |
| `/toolchain list <language> --json` | Same report as a bare JSON document. |
| unknown language id | `Unknown language id `{filter}`. Valid ids: ...` |
| unknown subcommand | `Unknown subcommand `{sub}`. Usage: /toolchain [list [language] [--json] | help]. Run /toolchain help for details.` |

## Examples

Probe everything and show the plain table:

```
/toolchain list
```

Probe only the Rust toolchain (cargo + rustc):

```
/toolchain list rust
```

Machine-readable report for scripts or other tools:

```
/toolchain list rust --json
```

Flag position does not matter:

```
/toolchain list --json rust
```

Unknown subcommand prints the usage correction:

```
/toolchain install rust
```

## Output

### Table form

A fixed-width table (constant 93-column width) with 10/10/10/50 character
columns:

| Column | Width | Overflow |
| ------ | ----- | -------- |
| Language | 10 | clipped |
| Runtime | 10 | clipped |
| Status | 10 | word-wrapped |
| Version | 50 | word-wrapped |

Wrapped rows continue on grid lines, so multi-word statuses and long versions
remain readable inside the fixed grid.

### JSON form

`--json` renders a bare JSON document (no table) with the same probe results,
one entry per toolchain, suitable for piping into other tools.

### Errors and status

- While probing: status `[wait] toolchain status`.
- Unknown language filter: `Unknown language id `{filter}`. Valid ids: ...`
- Unknown subcommand: `Unknown subcommand `{sub}`. Usage: /toolchain [list
  [language] [--json] | help]. Run /toolchain help for details.`

The command never installs or upgrades toolchains; missing toolchains simply
report as not found.

## Related

- Toolchain manual: `docs/howtos/toolchain.md`.
- README v1.0.95 notes the fixed-width `/toolchain list` table columns.
- `/new` scaffolding detects the runtime stack it needs; `/doctor` and
  `/update` cover other environment checks.