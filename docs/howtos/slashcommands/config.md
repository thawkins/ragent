# /config

> "Inspect, back up, and restore ragent configuration" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/config` is the in-TUI configuration inspector and backup manager. The `show`
form renders the resolved configuration without launching the agent: where the
application stores its files, which config files were found on disk, and the
final merged value of every key together with the source that supplied it. The
`save` form writes a timestamped backup of the global configuration into the
`config_dir/saves/` directory, and the `list` form opens an interactive picker
over previously saved backups so one can be restored.

Handlers live in `crates/ragent-tui/src/app/slash.rs` (the `config` arm,
approximately lines 2238-2652). All output accumulates under a `From: /config`
header in the message window.

## Syntax

```
/config show
/config save
/config list
/config help
```

Bare `/config` behaves like the unknown-args case and prints the usage line.

## Options / Subcommands

| Form | Description |
|---|---|
| `/config show` | Render application paths, discovered config files, and the fully resolved configuration table |
| `/config save` | Back up the global config to a timestamped file under `config_dir/saves/` |
| `/config list` | List saved configuration backups and open the restore picker |
| `/config help` | Show the subcommand summary |

## Examples

### Inspect resolved configuration

```
/config show
```

Output (representative, condensed):

```
From: /config show

Application Paths
  Working directory:  /home/user/Projects/ragent
  Data directory:     ~/.local/share/ragent
  Config directory:   ~/.config/ragent

Config Files
  Path                                        Exists
  /home/user/Projects/ragent/.ragent/ragent.json   yes
  /home/user/.config/ragent/ragent.json            no
  $RAGENT_CONFIG                                   (unset)

Resolved Values
  key                            | source                 | value
  ...
```

### Back up the global configuration

```
/config save
```

Output:

```
From: /config save
[ok] Configuration backed up to /home/user/.config/ragent/saves/ragent.json.20260911-101500
```

If the backup cannot be written the command reports `[err]` with
`No changes were made.` and leaves the configuration untouched.

### List and restore a saved configuration

```
/config list
```

Output (picker state):

```
From: /config list
Saved configurations (newest first):
  1. ragent.json.20260911-101500
  2. ragent.json.20260910-183022
...
```

With no saved backups present the output is:

```
From: /config list
No saved configurations found
Hint: use /config save to create a backup
```

### Show the subcommand summary

```
/config help
```

Output:

```
From: /config help
/config show | /config save | /config list | /config help
```

## Output

### `/config show` sections

- **Application Paths** - working directory, data directory, and config
  directory currently in use.
- **Config Files** - a table with exists checks for the project config
  (`<cwd>/.ragent/ragent.json`), the global config (`<config_dir>/ragent.json`),
  and the path given by the `RAGENT_CONFIG` environment variable.
- **Resolved Values** - a `key | source | value` table. Precedence is:
  `defaults < global config < project config < env file < RAGENT_CONFIG_CONTENT`.
  Every row names the source that supplied the winning value.
- **Union fields** - the merged-map fields `permission`, `instructions`,
  `skill_dirs`, `hooks`, `bash`, `hidden_tools`, `yolo`, `edit_log`, `sdd`,
  `piegap`, and `experimental` are labelled `global + project (union)` when a
  value is set in both layers.
- **Secret redaction** - keys matching `*_api_key` / `*_key`, plus the
  `openalex_email`, `gitlab`, `gmail`, and `channels` sections, are redacted
  before display.
- **Storage** - the resolved SQLite database path.
- **Code Index** - the index directory `.ragent/codeindex`.
- **Memory** - project and global memory directories.
- **Custom Agents** - the custom agent directories being scanned.

### `/config save`

Emits `[ok]` with the backup path, or `[err]` with `No changes were made.`
when the backup could not be written.

### `/config list`

Scans `<config_dir>/saves/` for files matching `ragent.json.*` (excluding
`.tmp` candidates), sorts newest-first by modification time, and opens the
`ConfigSavePickerState` interactive picker. In the picker:

- `Up` / `Down` or `k` / `j` move the selection,
- `Enter` restores the selected snapshot,
- `Esc` cancels.

An empty directory produces `No saved configurations found` plus the
`/config save` hint.

### Unknown arguments

Prints the usage line `/config show | /config save | /config list | /config help`.

## Related

- `/init config` - write a fresh default config file to disk
- `/reload` - re-read `ragent.json` into the running session
- `/skills` - view the skill registry loaded from `skill_dirs`
- [QUICKSTART.md](../../../QUICKSTART.md) - configuration examples
- [SPEC.md](../../../SPEC.md) - full configuration schema