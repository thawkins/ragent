# /llmstats

> "Show average LLM response time and token throughput" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/llmstats` reports measured round-trip latency and token throughput for the
currently selected model, averaged over the completed LLM responses recorded
in this session. It requires both an active model and at least one completed
response; with none of those present it explains what is missing instead of
showing zeros. Use it to sanity-check that a local or remote endpoint is
responding at expected speed, or to compare throughput across models.

The handler is the `llmstats` arm in `crates/ragent-tui/src/app/slash.rs`
(lines 3468-3501).

## Syntax

```
/llmstats
```

No arguments. Additional text after the command is ignored.

## Options / Subcommands

None. `/llmstats` takes no forms.

## Examples

### Show statistics for the active model

```
/llmstats
```

Output:

```
From: /llmstats
Model: ollama/qwen2.5-coder:7b
Samples: 12
Average round-trip: 1843.7 ms
Average prompt parsing tokens/sec: 941.25
Average output tokens/sec: 52.30
```

### No model selected

```
/llmstats
```

Status line:

```
[warn] No active model selected
```

Log entry: `llmstats: no active model`. No message-window output is produced.

### No completed responses yet

```
/llmstats
```

Output:

```
From: /llmstats
No completed LLM responses yet for anthropic/claude-sonnet-4-20250514.
```

Status line: `llm stats unavailable`.

## Output

The report block, emitted under `From: /llmstats`, contains:

- **Model** - the active model reference string (provider/model),
- **Samples** - number of completed LLM responses aggregated,
- **Average round-trip** - mean elapsed milliseconds per response,
- **Average prompt parsing tokens/sec** - mean prompt-side throughput,
- **Average output tokens/sec** - mean generation-side throughput.

The status line shows `llm stats` on success.

## Related

- `/cost` - token and cost totals for the session
- `/model` - switch the active model
- `/bench` - run the full benchmark suite for repeatable measurements