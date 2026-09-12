# /editlog

> Edit-operation logging: /editlog on|off|status|show|analyse|clear

## Overview

`/editlog` manages the edit-operation log, a record of every file-edit tool
call (read, write, edit, multiedit, patch, apply_patch, ...) and whether it
succeeded. Use it to diagnose why edits keep failing: `show` summarises
per-tool success rates and `analyse` inspects the failed `old_str` values to
characterise why they missed.

## Syntax

```
/editlog            Print the help table (same as /editlog help)
/editlog on         Enable edit-operation logging
/editlog off        Disable edit-operation logging
/editlog status     Show enabled state and log directory
/editlog show       Per-tool statistics table
/editlog analyse    Failed old_str analysis
/editlog clear      Empty the edit-log files (files kept)
/editlog help       Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `on` | Enable edit-operation logging via `persist_edit_log`. |
| `off` | Disable edit-operation logging. |
| `status` | Show whether logging is enabled and where the log lives (`log/`). |
| `show` | Render a per-tool table: tool name, total calls, successes, failures, success percentage, plus the recorded failure reasons. |
| `analyse` | Characterise the failed `old_str` values (risk traits, per-tool failure ratios, most common failure combinations, sample examples). |
| `clear` | Empty the edit-log files without deleting them; reports the number of cleared entries. |
| `help` | Print the help table. |

`--help` and `-h` are accepted as aliases of `help`.

## Examples

```
/editlog on
```
Starts recording every edit operation.

```
/editlog status
```
Confirms logging is enabled and prints the `log/` directory used.

```
/editlog show
```
Shows a table such as:

```
Tool        Calls  OK  Fail   Pct
edit          142  130   12    92
multiedit      38   33    5    87
```

```
/editlog analyse
```
Reports the risk characteristics of failed `old_str` blocks: per-tool failure
ratios, the most common failing combinations (top 10), and concrete examples.

```
/editlog clear
```
Empties the log files and prints the cleared-entry count; the files remain so
future stats continue to accumulate.

```
/editlog help
```
Prints the table above.

## Output

`on`/`off` confirm the state change; a failed persist is reported as a
`[warn]` line. `status` prints enabled/disabled plus the directory. `show`
prints a fixed-width per-tool stats table with success percentages and the
failure reasons seen. `analyse` prints a breakdown of failed `old_str`
characteristics, per-tool ratios, top combinations, and examples. `clear`
prints how many entries were emptied.

## Related

- `/editlog` statistics are cleared by `/log clear editlog`.
- `/log` -- log panel and directory clearing.
- `log/editlog/` -- where the edit log is written.