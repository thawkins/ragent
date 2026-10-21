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

## Dialog layout

The pickers share one composition (the same calm-slate chrome the plugin and
connector catalogue panels wear):

```
┌ Select Model  Anthropic ────────────────────────────────────┐
│2 models                                        Enter selects│
│  Model               Context   Cost        Thinking   Features│
│> Claude Sonnet 4        200K   Premium 1x  Auto/...   R,V,T   │
│  Claude Haiku 3.5       200K   Low 0.2x     Auto/...   R,V,T   │
│  ^/v navigate  Enter select  Esc cancel  |  R reasoning ...  │
└──────────────────────────────────────────────────────────────┘
```

- The frame title names the step and the provider; the heading strip below it
  carries the model count on the left and the confirm key on the right.
- A pinned black-on-grey band labels each column above the rows, so the columns
  stay identified while the list scrolls.
- The selected row is a filled accent bar; the model name leads in white and the
  supporting facts (context window, cost, thinking levels, feature flags) step
  back into a muted tone. `Context` and `Cost` are right-aligned so digits line up.
- The footer spells out the terse feature codes (`R reasoning`, `V vision`,
  `T tools`) and, when the list is windowed, leads with `Showing a-b of N` so a
  clipped list is never mistaken for the whole set.

The router tier picker (`/provider router` -> assign a model) uses the same table
and the same footer legend, with `Enter assign` in place of `Enter select`.

## Scrolling

Every selector in the provider/model dialogs windows around its cursor, so a list
longer than the dialog always keeps the highlighted row on screen and moves the
painted slice one entry at a time as you navigate:

- the provider picker, the model picker and the router tier picker;
- the thinking-level selector (each level is two lines tall, so it windows two
  lines at a time);
- the Azure resource picker;
- the router setup's provider palette (its block title states the window,
  `Providers (*) 11-20 of 30`) and each tier bucket (windowed by the selected
  model's own group of lines, so its retained properties stay with it).

Whenever the list is clipped the legend (or the block title) says
`Showing a-b of N`.

The mouse wheel scrolls the same way: with a setup dialog open, one notch over it
moves the cursor one entry (identical to `Up`/`Down`, because it goes through the
same handler), and a notch anywhere else is swallowed, so the chat and side panels
behind the modal never scroll under it. Non-list steps (the API-key and GitLab
forms, the loading spinner, the success card) swallow the notch rather than treating
it as a keystroke.

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