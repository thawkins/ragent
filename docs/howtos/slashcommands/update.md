# /update

> Check for a newer release on GitHub and install it

## Overview

`/update` checks the ragent GitHub releases for a newer version and can
replace the running binary. The check runs asynchronously: a "Checking for
updates..." line appears first, and the result arrives as an event-bus message
afterwards. `install` downloads the release binary and replaces the current
executable in place; a restart picks it up.

## Syntax

```
/update
/update install
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| (bare) | Check for a newer release; print the version and release notes |
| `install` | Download the newer release and replace the running binary |

## Examples

```
From: /update
Checking for updates...
Update available: v1.0.96
<first 500 characters of the release notes>
Run `/update install` to install.
```

```
From: /update
Checking for updates...
Already up to date (v1.0.95).
```

```
From: /update install
Updated to v1.0.96! Please restart ragent
```

```
From: /update install
(no platform binary published for the release)
A manual download is required; see github.com/thawkins/ragent/releases
```

## Output

Both forms print `Checking for updates...` immediately; results arrive
asynchronously as event-bus messages appended to the transcript. A newer
release prints the version, the first 500 characters of release notes, and the
install hint. An up-to-date install prints the restart reminder. When the
release has no binary for the current platform, the manual-download message is
shown instead.

## Related

- `/doctor`  -  flags an available update in its report
- github.com/thawkins/ragent/releases  -  manual downloads