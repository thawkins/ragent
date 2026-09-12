# /triggers

> Manage trigger rules: /triggers [list|enable|disable|remove|status|help]

## Overview

`/triggers` manages trigger rules: conditions that fire an action
automatically (for example, running a command when a file appears). Rules are
not created by this command -- they are created by natural language elsewhere
(e.g. "when $HOME/build.done exists, run cargo test") -- so `/triggers` lists,
toggles, removes, and reports metrics for the existing rule set.

## Syntax

```
/triggers                List trigger rules (same as /triggers list)
/triggers list           List trigger rules
/triggers enable <rule_id>    Enable a rule
/triggers disable <rule_id>   Disable a rule
/triggers remove <rule_id>    Remove a rule
/triggers status         Show runtime metrics
/triggers help           Print the usage table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| `list` (or bare) | Print a table of rules: ID (8-character prefix), condition, action, mode (`once` or `repeat`), and status (`active`, `disabled`, or `fired`). With no rules defined, prints the empty-state message explaining that rules are created by natural language. |
| `enable <rule_id>` | Enable the rule with the given ID prefix. |
| `disable <rule_id>` | Disable the rule so it no longer fires. |
| `remove <rule_id>` | Delete the rule. |
| `status` | Print runtime metrics: total/active/disabled/fired counts, dedup cache entries, cycle trackers, dedup window seconds, and max cycles. |
| `help` | Print the usage table. |

Unknown subcommands print a warning.

## Examples

```
/triggers list
```
Shows the rule table (ID, condition, action, mode, status) or the
empty-state explainer if none exist yet.

```
/triggers enable 3f9a2c
```
Re-enables the rule whose ID prefix is `3f9a2c`.

```
/triggers disable 3f9a2c
```
Stops the rule from firing without deleting it.

```
/triggers remove 3f9a2c
```
Deletes the rule.

```
/triggers status
```
Prints the metrics table: total, active, disabled, fired, dedup cache
entries, cycle trackers, dedup window secs, max cycles.

```
/triggers help
```
Prints the usage table.

## Output

`list` prints the fixed-width rule table, or the empty-state note that rules
are created by natural language. `enable`/`disable`/`remove` confirm the
change for the given ID prefix. `status` prints the runtime metrics table.
`help` prints the usage table; unknown subcommands print a warning line.

## Related

- Natural-language rule creation (rules are made by asking the agent, not by
  a slash-command form).
- `/status` -- unrelated; shows status-message history.