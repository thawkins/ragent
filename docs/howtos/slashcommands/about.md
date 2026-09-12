# /about

> Show application info, version, and authors

## Overview

`/about` prints a short informational banner describing ragent: what it is, the
compiled version number (`CARGO_PKG_VERSION`), the build timestamp, the source
repository URL, the license, and the author list. It takes no arguments and
performs no configuration lookups  -  everything shown is baked in at compile
time except the build date, which is stamped when the command runs.

The output is appended to the message window as an assistant bubble prefixed
with `From: /about`, and the status bar is set to `about`.

## Syntax

```
/about
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/about` | Show the application info banner |

No subcommands or arguments are supported; extra text after the trigger is
ignored.

## Examples

```
/about
```

Display the banner:

```
  ragent - AI Coding Agent

  An interactive TUI-based AI coding agent
  supporting multiple LLM providers.

  Version:     1.0.95
  Built:       2026-09-11 08:14:02 UTC
  Repository:  https://github.com/thawkins/ragent
  License:     MIT

  Authors:
    Tim Hawkins <tim.thawkins@gmail.com>
```

## Output

- An assistant message bubble titled `From: /about` containing the info block.
- The version line comes from the crate version compiled into the binary.
- The `Built:` line reflects the wall-clock time the command was run
  (UTC), not a compile-time build date.
- Status bar shows `about`.

## Related

- `/help`  -  list all available slash commands
- `/cost`  -  session token usage and estimated cost
- `/startup`  -  startup timing report