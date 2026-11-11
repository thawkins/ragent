## 1. Think Before Coding

VERY VERY IMPORTANT: All disscusion, thoughts and other output should be in English.

CRITICAL: whilst thinking is a good thing, overthinking is not, be concise, dont repeat thinking about small impact items, proceed to action when possible. Overthinking burns tokens, pushes costs up and significantly impacts performance and ROI.

CRITICAL: there is no such thing as a "pre-existing" test failure or lint or doctest failure. If you find one, fix it even if your changes did not cause it, but in a separate commit and say so in your report; otherwise touch only what the request needs (see 3. Surgical Changes).

So less thinking, more doing.

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## Response style (ADHD mode)

### Persistence

These rules apply to every response for the rest of the session, not only this one. They do not expire after a few turns and they do not lapse when the topic changes. If you are unsure whether they still apply, they do.

Turn them off only when the reader says "stop adhd mode" or "normal mode". Confirm in one line, then return to your default style.

What ADHD changes about reading. Five facts drive every rule below:

- Working memory is small. Anything not on screen is forgotten. Do not ask the reader to "keep in mind X."
- Knowing the answer is not doing the answer. The friction between "got it" and "done it" is where work dies.
- Starting is the hardest step. The first action must be obvious, small, and doable now.
- Time estimates feel uniform. "A bit of work" and "a few hours" register the same. Vague estimates fail.
- Dopamine is scarce. Visible progress matters. Buried wins do not register.

### Rules

**Rule 1. Lead with the next action.** The first line is something the reader can do. Not context. Not a plan. The action.
Bad: "Let's think about this. Your auth flow has a few moving pieces..." Good: "Run npm install jsonwebtoken, then edit src/auth.ts:42."
If the answer is a command, path, or snippet, it goes first. Prose comes after, if at all.

**Rule 2. Number multi-step tasks.** If the work takes more than one step, write a numbered list. Each step is one bounded action. No step contains "and then" twice.
Use the fewest steps that still work. Cut any step the reader does not need, and fold trivial steps into the one before. A short path finished beats a complete path abandoned.
Bad: "First open the file, find the function, swap it out, then run the tests."
Good:

1. Open `src/auth.ts`
2. Replace `verifyToken` (lines 42 to 58) with the snippet below
3. Run `npm test -- auth.spec.ts`

**Rule 3. End with one concrete next action.** If anything is left open, name ONE thing the reader can do in under two minutes. Even "open the file" counts.
Bad: "Hope that helps. Let me know if you want to dig deeper." Good: "Next: run npm test and paste the first failing line."

**Rule 4. Suppress tangents.** If a second issue exists, finish the first, then offer the second as a separate question.
Bad: "Here's the fix. By the way, your dependency is also stale, and your README is out of date, and..." Good: "Here's the fix. Separately: there is also a stale dependency. Want me to handle that next?"
A question that comes up mid-work is not a tangent: answer it yourself if you can and fold the result in. If it still needs the reader, surface it once, at the end.

**Rule 5. Restate state every turn.** The reader cannot hold "we are on step 3 of 5" between messages. Restate it.
Bad: "Done. Ready for the next part?" Good: "Step 3 of 5 done: schema updated. Next: backfill the new column. Run the script?"
If the harness has a task or plan tool, use it for multi-step work: one item per step, one in progress at a time. The checklist does the restating; do not also narrate the full plan as prose.

**Rule 6. Give specific time estimates.** Vague estimates fail. Ballpark in concrete units.
Bad: "This will take some work." Good: "About 15 minutes if tests already cover this. An afternoon if not."

**Rule 7. Make completed work visible.** Show what now works, in concrete terms. Do not bury wins in a recap.
Bad: "I've made some changes to the auth flow. Among other things..." Good: "Login now works with magic links. Try: npm run dev, open /login."

**Rule 8. Matter-of-fact tone for errors.** Never use "Uh oh," "Oh no," or "There seems to be a problem." State cause and fix.
Bad: "Uh oh, the test is failing. There seems to be an issue..." Good: "Test fails at auth.spec.ts:42: expected 200, got 401. Cause: missing auth header. Fix: add Authorization: Bearer ${token} to the request."

**Rule 9. Cap lists at 5 items.** If a list grows past five, split into "do now" vs "later," or "must" vs "nice to have." Five items ranked beats ten unranked.

**Rule 10. No preamble, no recap, no closing pleasantries.**
Forbidden openers: "Great question," "Let me...", "I'll...", "Sure!", "Looking at your...", "To answer your question..."
Forbidden recaps after a completed task: "I've now done X, Y, and Z, which means..."
Forbidden closers: "Let me know if you need anything else," "Hope this helps," "Happy to clarify," "Feel free to ask."
Start with the answer. End when the answer is done.

### When to break the rules

Override the defaults when:

- User asks to "explain" or "walk me through." Explain fully. Still no preamble, still no closer, but the body runs as long as the topic needs. Add headers so the reader can skim back.
- Destructive action ahead (rm -rf, force push, schema migration, dropping a table). Confirm before acting. Safety wins over brevity.
- Debug spiral. If the last three turns have been "still broken," stop iterating on code. Name the assumption that might be wrong. Ask one diagnostic question.
- Real ambiguity in the request. One short clarifying question beats guessing and rewriting.
- A rule fights the task. When a rule would delete the answer itself, the task wins; the shape stays. Example: "what are my options" gets 2 to 4 ranked options with one-line trade-offs, recommendation first, not one path. The options are the answer.
- A rule fights the harness. Inside an agent harness, the system prompt outranks these rules: announce a tool call when the harness requires it, do the work instead of asking "want me to," point time estimates at whoever executes the steps. Same principle as Rule 5: the constraint wins, the shape stays.

### Pre-send check

Before sending, delete:

- The first sentence if it announces what you are about to do.
- The last sentence if it asks "anything else?" or recaps what just happened.
- Any "by the way" sidebar.
- Any hedging adverb adding no information ("perhaps," "might," "could possibly"). Keep a hedge that carries real uncertainty; deleting it manufactures confidence.
- Any idiom or figurative phrase ("circle back," "get the ball rolling," "on the same page"). Replace with the literal action.

Then verify: if the reader reads only the first line and the last line, do they know (a) what to do next, and (b) what just happened?

If yes, send.

## Tool notes

- `Bash` is the canonical shell tool. Its `timeout` defaults to 120 seconds; pass an explicit `timeout` (in seconds, e.g. `1500`) for `cargo build`, `cargo nextest run` and other long commands, or they are killed at 120 s.
- There is no `search` or `search_in_repo` tool: use `Grep` (its `pattern` parameter is required) for text, and the `codeindex_*` tools for code symbols when the index is active.

@AGENTS-LANGUAGE.md

## GitHub Access

- When asked to "push to remote", update the SPEC.md, README.md, STATS.md, QUICKSTART.md and CHANGELOG.md files with all recent activity and spec changes, construct a suitable commit message based on recent activity, commit all changes and push the changes to the remote repository.
- When asked to "push release to remote", update the release number, and then follow the "push to remote" process. **Commit Message Rule**: Do not use "chore: bump version to ...", instead use "Version: <version_number>".
- When initializing a new repo, add BUG, FEATURE, TASK and CHANGE issue templates only do this once.
- **CRITICAL — NEVER push without explicit instruction**: Do not push changes to remote unless the user explicitly says words like "push to remote", "push to github", "push these changes", or "commit and push". This is a strict, non-negotiable rule. Even if you have modified files and the user says "looks good" or "that works", you still MUST NOT push until the user gives an explicit push command.
- Do not tag releases unless specifically told to.
- DO NOT use "git checkout" to rewind files, this ALWAYS results in lost work.

## Changelog Management

- **CHANGELOG.md**: Maintain a changelog in the root directory documenting all changes before each push to remote.
- **Format**: Follow Keep a Changelog format (https://keepachangelog.com/)
- **Update Timing**: Update CHANGELOG.md before each push to remote with the latest changes, features, fixes, and improvements.
- **Version**: Use semantic versioning (major.minor.patch-prerelease)
- Whenever a new feature or function is added ensure that SPEC.md and QUICKSTART.md is updated if relevant.

## Test Layout

The canonical test-layout rules (one `tests/test_*.rs` file per unit, one `all`
binary per crate via `autotests = false` + `tests/all.rs`, `cargo nextest`
filters, the binary-wide `global_env_lock`, nextest test groups) live in
"Test Organization" in AGENTS-LANGUAGE.md. Project-specific additions:

- A test that reads a process-global must pin it rather than inherit it:
  `support::make_app()` takes `cwd_path` from the real process cwd, so a test
  asserting on a cwd-derived value sets `app.cwd`/`app.cwd_path` explicitly.
- **Tests must not write into the repository.** A test run must leave the working
  tree byte-identical: no generated `INDEX.md`/`.index.json`, no per-run item
  directories, no log spools, no snapshot files under a checked-in path. Point
  every write at the git-ignored `target/temp/` scratch tree instead:

  - pass the root explicitly through state (e.g. `AppState::research_root`)
    rather than reading `std::env::current_dir()` inside a handler, and give the
    test a `target/temp/...` value;
  - when a write location is not already a parameter, add a `#[doc(hidden)]`
    setter over a process-wide `OnceLock` (e.g.
    `ragent_agent::session::history::set_subagent_log_root`,
    `ragent_tui::app::set_log_root_override`) and call it from the test;
  - resolve `target/temp` from `env!("CARGO_MANIFEST_DIR")` so the path is
    correct regardless of the test's working directory.
    Never "fix" this by restoring files with `git restore` after a run — that hides
    the leak and can clobber unrelated work.

## Documentation Standards

- For all functions create DOCBLOCK documentation comments above each public function that describes the purpose of the function, and documents any arguments and return values.
- For all modules place a DOCBLOCK at the top of the file that describes the purpose of the module, and any dependencies.
- **Documentation Files**: All documentation markdown files (`*.md`) **SHOULD** be located in the `docs/` folder, except for `TUI-QUICKSTART.md, QUICKSTART.md`, `STATS.md`, `SPEC.md`, `AGENTS.md`, `README.md`, `PLAN.md`, and `CHANGELOG.md`, which remain in the project root. Existing root-level project documents that predate this convention may remain until they are explicitly reorganized. When updating legacy root-level documents, prefer moving or consolidating them into `docs/` unless they are one of the approved root exceptions. Any future documentation should be created in the `docs/` folder following this convention.
- Do not create explainer documents or other `.md` files unless specifically asked to.

## Team Workflow

When asked to use a team or when a task benefits from parallel reviewers / workers:

1. **Create the team**: Use `team_create` with an appropriate `blueprint` (e.g. `code-review`).
   **Always pass `context`** — the user's specific request details: which directories/files to
   target, what task to perform, and where to write output. This context is prepended to every
   teammate's spawn prompt so they know exactly what to work on.
2. **Wait for results**: Call `team_wait` after creation. This blocks until every teammate becomes idle. **Do NOT use `wait_agents` for teammates — `wait_agents` only tracks `new_agent` sub-agents.**
3. **Read results**: Use `team_status` or read the team's output files to collect teammate findings.
4. **Do not duplicate work**: Do not independently read files or do analysis that a teammate is already doing. Wait for them first.

```
team_create blueprint="code-review" context="Review the crates/ragent-server directory for security, test coverage, and performance issues. Write findings to docs/COMPLIANCE.md"
team_wait                          ← REQUIRED: blocks until all idle
team_status                        ← read what they found
```

## General Preferences

1. Don't suggest features unless asked to.
2. When debugging problems, use Occam's razor and assume that the simplest solution is more likely to be the right one.
3. When debugging a problem, change only one thing at a time. If it does not fix the problem, revert it before trying another possible solution.
4. DO NOT perform temporary solutions or fixes; always provide a complete solution.
5. DO NOT declare an issue as fixed unless it has been confirmed; 90% of assertions of completion turn out to be false.
6. No new `unsafe`; the approved sites are listed in AGENTS-LANGUAGE.md (Core Code Rules).
7. No .unwrap() on user-facing paths.

## Temporary Files

1. Use the existing `target/` directory in the project root for build artifacts.
2. Do NOT use `/tmp` for tempoary storage, this folder is outside of the workspace and access is not permitted.
3. Create and use a `target/temp` directory for temporary files, scripts, and other ephemeral items that would normally be placed in `/tmp`.

## Task Tracking

- Use `TaskCreate`, `TaskUpdate`, `TaskGet`, and `TaskList` to track tasks.
- Always mark the task as `in_progress` when work on a task is started.
- Always mark the task as "completed" when work on a task is done.

For more details, see README.md and QUICKSTART.md.
