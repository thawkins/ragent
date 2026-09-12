# /mode
> Set agent role mode: /mode architect|coder|reviewer|debugger|tester|off|help

## Overview

`/mode` switches the agent's role mode for the current session. Role modes
bias the system prompt and tool emphasis toward a named discipline:
architecture review, coding, code review, debugging, testing, or none.
The chosen mode applies to subsequent turns until cleared or changed.

Input is lowercased before matching, so `/mode ARCHITECT` works. Two forms
beyond the registry description are accepted by the dispatcher: `status`
behaves like the bare form, and `normal` is an accepted alias of `off`.

## Syntax

```
/mode                      # show the current mode
/mode status               # same as bare form
/mode architect|coder|reviewer|debugger|tester   # activate a mode
/mode off                  # clear the mode (alias: normal)
/mode help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/mode` | Show the currently active role mode |
| `/mode status` | Same as the bare form |
| `/mode architect` | Activate the architect role mode |
| `/mode coder` | Activate the coder role mode |
| `/mode reviewer` | Activate the reviewer role mode |
| `/mode debugger` | Activate the debugger role mode |
| `/mode tester` | Activate the tester role mode |
| `/mode off` | Clear the mode and return to normal |
| `/mode normal` | Accepted alias of `off` |
| `/mode help` | Print usage help |

## Examples

Check the current mode:

```
/mode
```

Switch to review work:

```
/mode reviewer
```

Switch to coding:

```
/mode coder
```

Clear the mode:

```
/mode off
```

Clear via the alias:

```
/mode normal
```

Case-insensitive selection:

```
/mode ARCHITECT
```

## Output

Activation (mode icon varies by role):

```
<icon> mode: <label>
```

Clearing:

```
[ok] Role mode cleared - back to normal mode.
```

Unknown mode (the available list is echoed):

```
Unknown mode '<x>'. Available: architect coder reviewer debugger tester off
```

Bare form and `status` show the active mode name, or the normal-mode
equivalent when none is set.

## Related

- `/agent` - switch the whole agent preset (broader than a role mode)
- `/system` - override the system prompt directly
- `/plan` - delegate a planning task to the plan agent