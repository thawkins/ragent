# /provider

> Change provider, show config, or configure model router: /provider [show|router|help]

## Overview

`/provider` is the entry point for provider configuration in the TUI. The bare form
opens the interactive provider setup dialog, `/provider show` opens a viewer over the
configured providers, and `/provider router` opens the model-router setup dialog
seeded from the providers you have already configured. Provider setup covers API key
entry, model selection, and provider-specific options; `/model` then picks a model
from the configured provider.

## Syntax

```
/provider               Open the interactive provider setup dialog
/provider show          Show configured providers
/provider router        Open the model-router setup dialog
/provider help          Show the help table
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/provider` | Open the interactive provider setup dialog |
| `/provider show` | Show configured providers (and the router virtual provider when configured) |
| `/provider router` | Open the model-router setup dialog seeded from configured providers |
| `/provider help` | Show the help table |

Any other argument prints a one-line usage reminder in the chat:
`/provider` | `/provider show` | `/provider router` | `/provider help`.

## Behaviour

- **Bare form.** Opens the provider selection step with API key entry forced
  (`force_key_entry: true`), so it can be used both for first setup and for adding
  another provider.
- **`show`.** Lists all configured providers from storage in a selection dialog.
  When a saved router configuration exists, a virtual `router` provider
  ("Model Router") is appended to the list so the cluster can be viewed inline.
  With no configured providers it reports `[warn] No configured providers`.
- **`router`.** Requires at least one concrete provider; without one it reports
  `[warn] No concrete providers - configure one first`. The router setup dialog is
  seeded with the concrete providers so tiers can be assigned to them.
- `/provider help` prints a four-row subcommand table into the chat.

## Scrolling

Every list in the provider dialogs windows around its cursor, so a long list never
hides the highlighted row: `Up`/`Down` move the painted slice one entry at a time
and a clipped list says so (`Showing a-b of N`, or a window in the pane title such
as `Providers (*) 11-20 of 30`). The router setup's provider palette and each tier
bucket scroll the same way, the bucket window following the selected model's own
group of lines so its properties stay with it. The mouse wheel over an open setup
dialog moves the cursor one entry per notch and is swallowed anywhere else, so the
chat behind the modal never scrolls.

## Examples

```
/provider
```
Opens the provider setup dialog to add or reconfigure a provider.

```
/provider show
```
Opens the provider-config viewer listing every configured provider, including
the Model Router virtual entry when router config has been saved.

```
/provider router
```
Opens the router setup dialog seeded from the configured providers for tier
assignment.

```
/provider help
```
Prints the subcommand table with status line `provider: help`.

```
/provider models
```
Unknown subcommand - prints the usage reminder line.

## Dialog layout

Every step of provider setup shares one composition (the calm-slate chrome the
plugin and connector catalogue panels wear):

```
┌ Provider Setup ─────────────────────────────────────────────┐
│Select a Provider                          2 of 14 configured  │
│Provider                Id             Kind     Status        │
│Ollama (Local)          ollama         [local]                │
│Anthropic (Claude)      anthropic                  [ok]       │
│          ^/v navigate  Enter select  Esc cancel               │
└──────────────────────────────────────────────────────────────┘
```

- The frame title names the step and, where there is one, its subject (the provider
  name in `Enter API Key`, the provider and target tier in the router assignment).
- The heading strip carries the step name on the left and a live count on the right
  (`N of 14 configured`, `N configured`, `N available  * = in use`).
- Steps that are lists carry a pinned black-on-grey column band above the rows, so
  the columns stay identified while the list scrolls: provider name / id / kind
  (`[local]`) / status (`[ok]` when a credential is stored).
- The selected row is a filled accent bar, rows alternate a near-black stripe, and
  the key-hint footer is muted. `Provider Reset` keeps its yellow warning frame and
  yellow cursor bar.
- `Switch Provider` (`/model` with several configured providers) names where each
  key came from (`stored`, `env`, `auto`) beside its `[ok]`.
- The model list steps out of the API-key dialog is the shared model picker; see
  `/model` for its column layout.

## Output

- Bare and `show` forms: no chat text; a dialog state is set and the TUI renders
  the interactive provider UI.
- `router` form: dialog state when providers exist; otherwise the
  `[warn] No concrete providers - configure one first` status line plus a log entry.
- `help` form: help table in the message window with status `provider: help`.
- Unknown form: usage line in chat with status `provider: usage`.

## Related

- `/model` - pick a model from the configured provider
- `/provider_reset` - remove the current provider and its stored credentials
- `/router` - manage router runtime state (enable/disable, test, reload)
- `ragent config` - CLI view of the resolved configuration
- `ragent auth` - CLI provider authentication setup