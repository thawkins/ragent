# /cost

> Show session token usage and estimated cost

## Overview

`/cost` summarises the LLM traffic recorded for the current session: number of
completed request samples, session duration, total input/output tokens, and an
estimated cost in USD. Cost is computed per sample from the resolved model's
configured per-million-token input and output rates; samples whose model
cannot be resolved contribute tokens but zero cost. The report also breaks
totals down by provider, sorted alphabetically.

If no completed LLM responses have been recorded yet, the command says so
instead of printing an empty summary.

## Syntax

```
/cost
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/cost` | Show session token usage and estimated cost |

No arguments or subcommands.

## Examples

```
/cost
```

Typical report:

```
From: /cost
Samples: 14
Session duration: 12m 31s
Total tokens: 48210 input / 3918 output
Estimated cost: $0.412370

By provider:
  - anthropic: $0.412370 (48210 in / 3918 out, 14 samples)
  - ollama: $0.000000 (2150 in / 260 out, 2 samples)
```

```
/cost
```

Nothing recorded yet:

```
No completed LLM responses yet for this session.
```

## Output

- Assistant bubble `From: /cost` with: `Samples:`, `Session duration:`,
  `Total tokens: N input / N output`, `Estimated cost: $X.XXXXXX`, and an
  optional `By provider:` block listing per-provider cost, tokens, and sample
  counts.
- Session duration is derived from the stored session's creation time
  (hours/minutes/seconds format).
- No-data case: assistant bubble `No completed LLM responses yet for this
  session.` and status `cost unavailable`.
- Normal status bar: `cost summary`.

## Related

- `/about`  -  application version info
- `/startup`  -  startup timing report
- Model costs are configured per model in `ragent.json`