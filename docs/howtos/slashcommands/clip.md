# /clip

> Copy the rendered message-window contents to the system clipboard

## Overview

`/clip` copies the message window to the system clipboard exactly as it was
last rendered: it joins the plain-text rows the Messages pane produced
(`message_content_lines`, the same buffer the text-selection copy feature
reads) with newlines and places that text on the clipboard. What lands on the
clipboard is therefore exactly what you see on screen, not the raw markdown
source.

If nothing has been rendered yet (empty message window), the command reports
there is nothing to copy.

## Syntax

```
/clip
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/clip` | Copy the rendered message-window contents to the system clipboard |

No arguments or subcommands.

## Examples

```
/clip
```

Success (status bar and bubble show the copied size):

```
Copied 12345 characters (312 rendered lines) to the clipboard.
```

```
/clip
```

Nothing rendered yet:

```
No rendered message content to copy yet.
```

## Output

- On success the clipboard contains the joined rendered lines; an assistant
  bubble `From: /clip` reports the character count and rendered-line count,
  and the status bar shows `clip: copied <chars> chars`.
- On an empty window: assistant bubble `No rendered message content to copy
  yet.` and status `clip: nothing to copy`.

## Related

- `/actionloop clip`  -  copy action-loop timing data to the clipboard
- `/cost`  -  session token usage summary (a common thing to clip)
- Text-selection copy reads the same rendered buffer directly