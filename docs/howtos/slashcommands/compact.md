# /compact
> Summarise and compact the conversation history

## Overview

`/compact` replaces the current session's message history with an LLM-generated
summary plus the most recent turns. The session processor compacts the stored
history, the in-memory transcript is swapped for the result, and the context
window measurement drops immediately. Use it when token usage approaches the
model's context limit or output starts to degrade.

Compaction is asynchronous: the command starts a background compaction task,
shows a `compacting...` status, and updates the message window when the summary
arrives. The same engine also runs automatic compaction (configured in the
`compaction` section of `ragent.json`), and manual compaction inherits the
post-compaction queued-prompt drain so a prompt submitted during compaction is
sent after the history swap completes.

`/compress` is a deprecated alias for `/compact` and forwards to the identical
code path; it additionally logs a deprecation notice. Do not create new
content that uses `/compress`.

## Syntax

```
/compact
```

No arguments, options, or subcommands. The command operates on the current
session only.

Alias:

```
/compress        # deprecated alias, identical behaviour
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/compact` | Start manual compaction of the current session history |
| `/compress` | Deprecated alias of `/compact`; logs a deprecation line then behaves identically |

## Examples

Compact the current session:

```
/compact
```

Same result via the deprecated alias (avoid in new sessions):

```
/compress
```

Combine with a cost check first, to decide whether compaction is worthwhile:

```
/cost
/compact
```

Check token headroom after compaction:

```
/compact
/context
```

Queue the next prompt while compaction runs; it is sent automatically when the
history swap completes:

```
/compact
continue with the next test file
```

## Output

What appears in the message window and status bar:

- Status `compacting...` while the summary is being generated (manual
  compaction; auto compaction shows `compacting before send...`).
- On success the conversation history is replaced with a compaction summary
  followed by the retained recent messages; the status returns to `ready`, a
  log line records that session history was replaced with the summary, and the
  context-snapshot refresh is scheduled.
- On failure the status shows `[warn] compact failed: <error>` and the failure
  is logged.
- Guard messages when compaction cannot start or continue:
  - No active session: `[warn] No active session to compact`
  - Empty history: `[warn] No messages to compact`
  - No model selected: `[warn] No model selected - use /model to choose`

## Related

- `/cost` - token usage and estimated cost for the session
- `/context` - context cache management and window measurement
- `/model` - switch the model used for compaction and chatting
- `/clear` - wipe history entirely (no summary retained)
- `compaction` config section in `ragent.json` - automatic compaction,
  threshold, buffer, summary token cap, and optional dedicated compaction model