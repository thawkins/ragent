# /bug-report

> Generate a bug report file with session diagnostics, logs, and transcript

## Overview

`/bug-report` writes a self-contained markdown bug report under the project
working directory so it can be attached to an issue. The file is created at
`log/bug-report-<YYYYMMDD>-<HHMMSS>.md` (the `log/` directory is created if
missing). Sensitive content is redacted: log entries and the transcript pass
through `redact_secrets`, and transcript messages are truncated.

There are no subcommands; the command runs immediately.

## Syntax

```
/bug-report
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| (bare) | Write the bug report and print its path |

## Report contents

| Section | What it contains |
|---------|------------------|
| Session Information | Session id, title, directory, created/updated timestamps |
| Agent Configuration | Active agent, its description, selected model |
| Tool Configuration | Visible tool count and per-family visibility (office, github, gitlab, teams, agents, plan, codeindex) |
| Token Usage | Input, output, and total token counts for the session |
| Recent Log Entries | Last 50 log lines, `redact_secrets` applied |
| Session Transcript | Each message, redacted, truncated to 501 characters |

## Examples

```
From: /bug-report
Bug report written to log/bug-report-20260911-141530.md

The report includes session info, agent and tool configuration, token usage,
the last 50 log entries, and the session transcript.
Please review the report for sensitive information before sharing.
```

```
From: /bug-report
(In a session with 3 messages and one tool error)
The transcript section shows each message redacted and truncated to 501
characters; the log section shows the tool error with secrets redacted.
```

## Output

One success block in the message window: the written file path, a summary of
what the report includes, and a warning to review the file for sensitive
information before sharing it. Failures (unwritable path) print a `[warn]`
error line.

## Related

- `/doctor`  -  live environment diagnostics instead of a file report
- `/debug`  -  reads session debug logs in-session
- Log directory `log/` also holds sub-agent reports