# /thinking

> Switch the current thinking level: /thinking auto|off|low|medium|high

## Overview

`/thinking` reports or changes the reasoning (extended thinking) level of the
currently selected model. Bare `/thinking` prints the current level and the levels
the model supports; passing a level switches it and persists the choice. The command
is model-gated: it refuses to run before a model has been selected with `/model`, and
it validates the requested level against the levels the active model actually supports.

## Syntax

```
/thinking                       Show current level and supported levels
/thinking <level>               Set the thinking level
```

Accepted `<level>` values: `auto`, `off`, `low`, `medium`, `high`.

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/thinking` | Print the current thinking level and the supported levels |
| `/thinking auto` | Let the provider default decide thinking behaviour |
| `/thinking off` | Disable extended thinking |
| `/thinking low` | Low reasoning effort |
| `/thinking medium` | Medium reasoning effort |
| `/thinking high` | High reasoning effort |

## Behaviour

- **Model required.** With no model selected the command prints
  `[warn] No model selected - use /model to choose` and does nothing.
- **Validation.** The requested level is validated against the active model's
  supported level set. An unsupported level prints
  `[warn] Thinking level '<level>' is not supported by the active model`.
- **Thinking-incapable models.** If the active model reports no configurable
  thinking levels, only `off` is accepted; any other level prints
  `[warn] Active model does not support configurable thinking`.
- **Persistence.** The chosen level is stored against the selected model and is
  used for subsequent turns in the session.
- An invalid token (not one of the five levels) prints
  `Usage: /thinking [auto|off|low|medium|high]`.

## Examples

```
/thinking
```
Prints the current and supported levels, for example:
`Current: 'medium'` and `Supported: 'auto, low, medium, high'`.

```
/thinking high
```
Switches the model to high reasoning effort; status line becomes
`thinking: high`.

```
/thinking off
```
Turns extended thinking off for the active model.

```
/thinking turbo
```
Invalid level - prints the usage line in the status bar.

## Output

- Bare form: chat text with the current level and the supported level list,
  status line `thinking`.
- Set form: no chat text; status line `thinking: <level>`.
- Refusals: status line only (`[warn] No model selected - use /model to choose`,
  unsupported-level warning, unsupported-model warning, or the usage line).

## Related

- `/model` - select the model whose thinking level you configure
- `/provider` - change provider configuration
- Provider `thinking` section in `ragent.json` - per-provider and per-model
  thinking defaults (`enabled`, `level`, `budget_tokens`)