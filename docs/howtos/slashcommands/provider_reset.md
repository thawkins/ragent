# /provider_reset

> Reset the current provider and remove stored credentials

## Overview

`/provider_reset` opens the provider reset dialog, which lets you clear the current
provider selection and delete its stored credentials from the encrypted credential
store. Use it when switching accounts, revoking a key, or repairing a broken provider
configuration. The command takes no arguments and always opens the same dialog.

## Syntax

```
/provider_reset
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/provider_reset` | Open the reset dialog over the current provider |

There are no subcommands. Any text typed after the command word is ignored - the
dialog is opened regardless of what follows `/provider_reset`.

## Behaviour

- The command sets the provider-setup step to the reset dialog; it does not reset
  anything by itself until you confirm inside the dialog.
- Resetting removes the stored credentials for the selected provider, so the next
  connection will require the key to be entered again.
- After a reset the TUI returns to provider selection, so the next action is
  typically `/model` or `/provider` to configure a replacement provider.

## Examples

```
/provider_reset
```
Opens the reset dialog for the currently selected provider.

```
/provider_reset anthropic
```
The extra token is ignored; the same reset dialog opens.

## Output

- No chat text is appended. The TUI switches to the provider reset dialog, which
  lists the current provider and asks for confirmation before clearing it.
- The status bar reflects the dialog state while it is open.

## Related

- `/provider` - add or reconfigure providers
- `/model` - pick a model after re-configuring
- `ragent auth` - CLI provider authentication configuration
- Credentials are stored encrypted in the SQLite storage layer (see
  `ragent-storage`)