# /model

> Switch the active model, or show metadata with /model show (/model help for usage)

## Overview

`/model` manages the active LLM model selection in the TUI. With no arguments it opens
the model picker for the currently configured provider; when no provider is configured
it falls back to the provider picker. `/model show` prints a metadata report for the
active model into the message window. Switching models happens through the interactive
picker dialogs rather than by typing a model name on the command line.

## Syntax

```
/model              Open the model picker for the configured provider
/model show         Print the active model's metadata report
/model help         Show the help table
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/model` | Open the model picker for the configured provider (or the provider picker when none is configured) |
| `/model show` | Print the active model's metadata report into the chat |
| `/model help` | Show the help table |

Any other argument produces the usage line `Usage: /model [show]`.

## Bare-form branches

The bare `/model` form takes three branches depending on the configured provider:

1. **Normal provider configured** - the model list for that provider is fetched
   (the status line shows a model-discovery progress state) and the model picker
   opens with the discovered entries.
2. **`azure_resource` provider configured** - an Azure resource entry picker opens
   instead of the model list. The previously selected resource is pre-highlighted
   from the `azure_resource_last_selection` storage setting.
3. **`router` provider configured** - the model-router setup dialog opens, seeded
   from the configured concrete providers. If no concrete providers are configured,
   the provider picker opens instead.

If nothing is configured at all, branch 1 becomes the provider picker so the first
`/model` call walks you straight into provider setup.

## Examples

```
/model
```
Opens the model picker for the configured provider.

```
/model show
```
Prints the metadata report of the currently active model (context window,
token limits, capabilities) into the message window.

```
/model help
```
Prints the three-row subcommand table shown above.

```
/model list
```
Not a supported form - prints `Usage: /model [show]` in the status bar.

## Output

- Bare form: no chat text; the TUI switches to a model-discovery status and then
  the picker dialog (`LoadingModels` step while the provider's model list is
  fetched, then the selection list).
- `/model show`: the metadata report appended to the message window, with the
  status line `active model metadata`. If no model is active, the status line
  shows `[warn] No active model selected` instead.
- `/model help`: the help table appended to the message window with status
  `model: help`.
- Unknown argument: status line `Usage: /model [show]`.

## Related

- `/thinking` - switch the thinking level of the selected model
- `/provider` - change or show provider configuration
- `/provider_reset` - reset the current provider and stored credentials
- `/router` - manage the model-router cluster
- `ragent models` - CLI equivalent that lists available models