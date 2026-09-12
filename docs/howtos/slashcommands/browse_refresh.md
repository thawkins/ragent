# /browse_refresh
> Refresh the @ file-picker project index

## Overview

`/browse_refresh` rebuilds the in-memory project-file index that the `@`
file picker uses for autocomplete suggestions. Use it when files have been
created, renamed, or deleted outside ragent and the picker is showing a stale
list. The cache is also refreshed automatically in the background, so this
command is only needed for an immediate refresh.

## Syntax

```
/browse_refresh
```

The command takes no arguments. Extra tokens are ignored.

## Options / Subcommands

| Form | Description |
|---|---|
| `/browse_refresh` | Re-collect the project file cache (up to 10,000 entries) and refresh the picker menu |

## Examples

Refresh the index after creating new files:

```
/browse_refresh
```

Status bar shows: `browse index refreshed (1234)` where the number is the
cached entry count.

## Output

- The status line reports `browse index refreshed (N)` with the number of
  cached project files.
- An info log entry records `@ picker index refreshed (N entries)` in the log
  panel.
- The next time the `@` file picker opens, it lists the refreshed set of
  project files.

## Related

- `@` file picker - the autocomplete surface this command refreshes
- `/inputdiag` - shows the browse cache state (cwd, entry count, refreshed-at)
- `/log` - open the log panel to see the refresh log entry