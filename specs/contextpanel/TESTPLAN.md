---
status: draft
---

# Context Panel Manual Test Plan

## Prerequisites

1. A working ragent build on the branch that includes the context panel work.
2. A valid `ragent.json` (or environment key) selecting a model whose context
   window is known, for example `anthropic/claude-sonnet-4` (200 K tokens) or
   an OpenAI GPT-4o model (128 K tokens).
3. A project directory with an `AGENTS.md` file present so the project-guideline
   partition is non-empty.
4. At least one skill pack or memory entry is recommended so the "Other
   partitions" row can be observed, but is not strictly required.

## Test Cases

### TC-001 — Open and close the context panel

**Title:** Toggle the context panel with Alt-C.

**Preconditions:**
- ragent TUI is running and showing the main chat screen.
- The agent is idle and the input box is visible at the bottom.

**Steps:**
1. Press `Alt-C` once.
2. Observe the right-hand side of the screen.
3. Press `Alt-C` a second time.
4. Observe the layout again.
5. Press `Alt-C` a third time.

**Test data:** none.

**Expected results:**
- After step 1, a new bordered panel titled "Context" appears on the right side
  of the screen. The main chat/log area shrinks horizontally to make room.
- After step 3, the panel disappears and the chat/log area returns to its
  previous width.
- After step 5, the panel reappears in the same place.

### TC-002 — Verify system prompt and model capacity display

**Title:** System prompt size is shown and the model capacity is reported.

**Preconditions:**
- Context panel is visible.
- An agent with a non-empty system prompt is selected (for example the default
  `coder` agent).

**Steps:**
1. Open the context panel with `Alt-C` if it is not already open.
2. Read the first two rows under the "Context" title.

**Test data:** none.

**Expected results:**
- A row labelled "System prompt" shows a non-zero token count and a percentage
  that is consistent with the selected model's reported context-window size.
- A row labelled "Model capacity" (or equivalent) shows the model's reported
  context-window in tokens, for example `200000` for Claude Sonnet 4.
- If the provider does not report a limit, the percentage column shows
  "unknown" and the model capacity row shows "unknown".

### TC-003 — Verify toolset catalog and metadata sizes

**Title:** Tool catalog and metadata sizes are displayed separately.

**Preconditions:**
- Context panel is visible.
- The current agent exposes a non-empty toolset (the default coder agent exposes
  many tools).

**Steps:**
1. Open the context panel with `Alt-C` if it is not already open.
2. Locate the rows labelled "Tool catalog" and "Tool metadata".

**Test data:** none.

**Expected results:**
- "Tool catalog" shows a token count greater than zero and a percentage share.
- "Tool metadata" shows a token count that is non-negative; for agents with
  JSON/XML tool wrappers it should be greater than zero.
- The catalog count should be smaller than or equal to the combined
  catalog-plus-metadata count when the two are added together manually.

### TC-004 — Verify conversation history size

**Title:** Message history size and message count are updated after chatting.

**Preconditions:**
- Context panel is visible.
- The conversation has at least one user message and one assistant response.

**Steps:**
1. Note the "History" row token count and message count.
2. Type `hi` into the input box and press `Enter`.
3. Wait for the assistant response to complete.
4. Read the "History" row again.

**Test data:**
- Input text: `hi`

**Expected results:**
- The "History" token count after step 4 is strictly greater than the count
  noted in step 1.
- The "History" message count after step 4 is at least two higher than in
  step 1 (one user message plus one assistant reply).

### TC-005 — Verify other partitions and total arithmetic

**Title:** Other partitions, total usage and headroom are arithmetically correct.

**Preconditions:**
- Context panel is visible.
- The session has a non-empty system prompt and toolset.

**Steps:**
1. Read every partition row and its token count.
2. Read the "Total used" and "Headroom" rows.
3. Add the partition counts manually using a calculator.

**Test data:** none.

**Expected results:**
- "Total used" equals the sum of the displayed partition counts.
- "Headroom" equals the model capacity minus the total used (or "unknown" when
  the capacity is unavailable).
- The percentage shown for "Total used" is `total / capacity * 100` rounded to
  one decimal place.

### TC-006 — Verify panel content is not sent to the model

**Title:** The context panel must not pollute the LLM context.

**Preconditions:**
- Context panel is visible.
- The session log panel or file log is accessible.

**Steps:**
1. Open the context panel with `Alt-C`.
2. Send the user message: `What is the name of the right-hand panel you can see?`
3. Wait for the assistant response.

**Test data:**
- Input text: `What is the name of the right-hand panel you can see?`

**Expected results:**
- The assistant does not claim to see a panel named "Context".
- The assistant does not reference token counts, percentages, or tool catalog
  sizes.
- The exported session (if exported) does not contain any synthetic message
  containing the word "Context" panel breakdown.

### TC-007 — Verify refresh after model switch

**Title:** Values update when the model is changed.

**Preconditions:**
- Context panel is visible.
- At least two configured models with different context-window sizes are
  available, for example a local Ollama model and an Anthropic model.

**Steps:**
1. Note the "Model capacity" value for the current model.
2. Switch to a different model using the `/model` slash command or the model
  picker.
3. Read the "Model capacity" row again.

**Test data:** none.

**Expected results:**
- The model capacity value after step 3 reflects the newly selected model.
- All percentage shares recalculate to be relative to the new capacity.
- If the new model's capacity is unknown, percentages switch to "unknown".

## Cleanup

1. Close the context panel with `Alt-C` if it is still open.
2. If any sessions were created for testing, delete or archive them via the
   session manager so they do not clutter the local database.
3. Return the model selection to the project's default if it was changed
   during TC-007.
