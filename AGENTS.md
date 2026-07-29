## 1. Think Before Coding

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

## Agent Guidelines for Rust apps

- First when you startup say "Hi I'm RAgent and I have read Agents.md"

## Available Tools

You have access to the following tools. Use ONLY these exact tool names — do not invent or guess tool names. The canonical names below are the only ones you should use:

### Core Tools (always available)

- `bash` — Execute a shell command. Use `command` to provide the command.
- `read` — Read file contents with optional `start_line`/`end_line` range.
- `edit` — Replace an exact occurrence of text in a file.
- `multiedit` — Apply multiple edits across one or more files atomically.
- `write` — Create or overwrite a file.
- `create` — Create a new file with content.
- `append_to_file` — Append text to the end of a file.
- `grep` — Search file contents for a regex pattern using ripgrep.
- `glob` — Find files matching a glob pattern.
- `list` — List directory contents.
- `get_env` — Read environment variables.
- `file_info` — Return metadata for a file or directory.
- `diff_files` — Show a unified diff between two files or inline strings.
- `copy_file` — Copy a file to a new location.
- `move_file` — Move or rename a file or directory.
- `rm` — Delete a single file.
- `patch` — Apply a unified diff patch to one or more files.
- `make_directory` — Create a directory at the given path, including any missing parent directories.
- `calculator` — Evaluate a mathematical expression.
- `think` — Record a short reasoning note without changing project state.
- `todo_read` — List TODO items for the current session.
- `todo_write` — Add, update, remove, or clear TODO items.
- `memory_read` — Read the contents of a memory file.
- `memory_write` — Persist notes or learnings to memory files.
- `memory_replace` — Replace a specific string in a named memory block.
- `memory_search` — Search memories using semantic similarity or keyword matching.
- `memory_store` — Store a structured memory with category, tags, and confidence score.
- `memory_forget` — Delete structured memories by ID or filter criteria.
- `memory_migrate` — Analyse a flat MEMORY.md file and propose splitting it into named blocks.
- `plan_enter` — Delegate to the plan agent for read-only codebase analysis.
- `codeindex_search` — Search the codebase index for symbols, functions, types, and documentation.
- `codeindex_symbols` — Query symbols (functions, structs, enums, traits) from the codebase index.
- `codeindex_references` — Find all references to a symbol by name across the indexed codebase.
- `codeindex_dependencies` — Query file-level dependencies from the code index.
- `codeindex_status` — Show the current status and statistics of the codebase index.
- `codeindex_reindex` — Trigger a full re-index of the codebase.
- `ask_user` — Ask the user a question and get feedback from user.

### Code Intelligence Decision Flow

When the codebase index is active, you MUST use `codeindex` tools instead of `grep` for code symbol queries. The index is faster, returns structured results, and understands symbol kinds.

| Query type                     | Use                             |
| ------------------------------ | ------------------------------- |
| "Where is function X defined?" | `codeindex_search` (NOT grep) |
| "Find all structs matching Y"  | `codeindex_symbols`           |
| "Who calls function Z?"        | `codeindex_references`        |
| "What does file A import?"     | `codeindex_dependencies`      |
| "Is the index working?"        | `codeindex_status`            |
| "Re-index after bulk edits"    | `codeindex_reindex`           |

When searching for arbitrary text strings, comments, or non-symbol content, use `grep` with the `pattern` parameter. **Do NOT use `search` or `search_in_repo`** — these are not available tools. Always use `grep` for all text and pattern matching across files.

**CRITICAL — grep parameter requirement:**
The `grep` tool requires the `pattern` parameter. This is the ONLY required field. Do NOT omit it. Example:

```
grep(pattern: "fn main", path: "src")
```

**CRITICAL — grep is the ONLY text search tool:**
There is no `search` or `search_in_repo` tool. Use `grep` for every text search need, whether it's a regex pattern or a plain text string. There are no aliases or shortcuts.

### Shell Execution Rules

- For simple commands or code snippets, use `bash` with the `command` parameter.
- Timeout defaults to 600 seconds.
- The `bash_reset` tool resets the persistent shell state.

### Important

Always use the canonical `bash` tool.

## File Reading Best Practices

When reading files with the `read` tool:

- **REQUIRED for files larger than 100 lines**: Always use `start_line` and `num_lines` parameters to read the file in focused sections rather than all at once.
- **PREFERRED parameters**: `start_line` + `num_lines` (the most intuitive pair — `start_line` is the 1-based absolute line number where reading begins, `num_lines` is the COUNT of lines to read from that start). Example: `start_line=201, num_lines=100` reads lines 201–300 (inclusive).
- **CRITICAL — avoid `end_line` unless you really need an absolute last-line**: `end_line` is the 1-based absolute last line number to include (NOT a count). The common mistake is to pass `end_line=100` meaning "100 lines" — that is wrong; use `num_lines=100` for that. If you do use `end_line`, set it to a value `>= start_line`, e.g. `start_line=201, end_line=300` reads lines 201–300.
- **Auto-detect help**: If you accidentally pass `end_line` smaller than `start_line`, the tool will refuse with an actionable error suggesting `num_lines`. Re-read the message and retry with the suggested fix.
- `start_line` is **absolute 1-based** (not an offset from start).
- The tool will return an error if you exceed the file's total line count. The error message includes `total_lines`.
- When you read a file, the response metadata includes `total_lines` — use that value to plan subsequent reads.
- **Strategy**:
  1. Read the file without `start_line`/`num_lines` first — for large files this returns the first 100 lines plus a section map with the total line count
  2. Use `total_lines` from the response to plan your subsequent reads
  3. Then read specific sections using `start_line` + `num_lines`
  4. Never read an entire file >100 lines in a single call

## Technology Stack

- **Language**: Rust edition 2024 or greater

## Build Commands

- `cargo build` — Build debug binary; allow up to 600 seconds.
- `cargo build --release` — Build optimized release binary; allow up to 600 seconds.
- `cargo check` — Check code without building.
- Build only debug builds unless specifically asked to perform a `release build`.

Builds can take a long time, so allow up to 600 seconds for a rebuild.

## Test Commands

- `cargo test` — Run all tests
- `cargo test <test_function_name>` — Run specific test function
- `cargo test -- --nocapture` — Run tests with output visible
- `cargo test --lib` — Test library only (skip integration tests)
- **Test Timeout**: All test runs should have a 10-minute timeout to prevent hanging
  - Use `timeout 600 cargo test` on Unix/Linux
  - Use `cargo test --test-threads=1` for sequential execution if needed

### Test Organization

All tests **MUST** be located in the `tests/` directory inside each crate. If a test is at the workspace root, it should be placed in the root `tests/` folder, not inline in source files.

- Use `#[test]` for sync tests and `#[tokio::test]` for async tests.
- Import from the relevant public crate for the crate under test rather than assuming a single `ragent` crate path.
- Organize related tests together.
- Follow the naming convention: `test_<component>_<scenario>` (e.g. `test_jog_x_positive`).
- For each project crate, migrate related tests into suitable subfolders within that crate's `tests/` directory. Review both inline and external tests for migration candidates, and relocate inline tests from `.rs` files into separate files under the appropriate `tests/` subfolder where practical.
- **Migration strategies** (see `docs/reports/testconsolidate-completion.md` for the full migration report):
  - **Public-API tests**: use `use ragent_<crate>::module::Item;` — no source changes needed.
  - **Private-item tests**: widen the tested items to `pub(crate)` (FR-007) and re-import the source module via `#[path = "../src/<module>.rs"] mod <module>;` (FR-008). Provide shims for `super::` and `crate::` references at the test file root.
  - **Complex cases** (`//!` doc comments + `crate::` cross-module deps): use `#[cfg(test)] #[path = "../../tests/test_<module>.rs"] mod test_<module>;` in the source file to compile the external test within the crate's module tree.
- **Do not add new inline `#[cfg(test)]` modules** to library source files. All new tests go in `tests/`.

## Lint & Format Commands

- `cargo clippy` — Run linter with clippy
- `cargo fmt` — Format code with rustfmt, always use this to fix indentation
- `cargo fmt --check` — Check formatting without changes

## Units

- DateTime values should be represented internally in UTC and translated to locale-based representations in the UI layer.
- Dimensional units should be represented internally in millimetres (`mm`) as `f32`, and presented to 2 decimal places where relevant.
- Text should be represented internally as UTF-8 where feasible, with translation to and from UI-specific encodings only when required.

## GitHub Access

- Use "gh" to access all GitHub repositories.
- When asked to "push to remote", update the SPEC.md, README.md, STATS.md, RELEASE.md, QUICKSTART.md and CHANGELOG.md files with all recent activity and spec changes, construct a suitable commit message based on recent activity, commit all changes and push the changes to the remote repository.
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
- **RELEASE.md**: Write the version number and the most recent CHANGELOG.md entry to the RELEASE.md file for use as a Description in the Github Releases page.
- Whenever a new feature or function is added ensure that SPEC.md and QUICKSTART.md is updated if relevant.

## Documentation Standards

- For all functions create DOCBLOCK documentation comments above each public function that describes the purpose of the function, and documents any arguments and return values.
- For all modules place a DOCBLOCK at the top of the file that describes the purpose of the module, and any dependencies.
- **Documentation Files**: All documentation markdown files (`*.md`) **SHOULD** be located in the `docs/` folder, except for `QUICKSTART.md`, `RELEASE.md`, `STATS.md`, `SPEC.md`, `AGENTS.md`, `README.md`, `PLAN.md`, and `CHANGELOG.md`, which remain in the project root. Existing root-level project documents that predate this convention may remain until they are explicitly reorganized. When updating legacy root-level documents, prefer moving or consolidating them into `docs/` unless they are one of the approved root exceptions. Any future documentation should be created in the `docs/` folder following this convention.
- Do not create explainer documents or other `.md` files unless specifically asked to.

## Code Style Guidelines

- **Formatting**: 4 spaces, max width 100, reorder imports automatically, Unix newlines.
- **Naming**: snake_case for functions/variables, PascalCase for types/structs/enums.
- **Imports**: Group std, external crates, then local modules.
- **Error Handling**: Use `Result<T, E>` with `?`, `anyhow::Result` for main, and `thiserror` for custom errors.
- **Types**: Prefer explicit types, and use type aliases for complex types.
- **Logging**: Use the `tracing` crate with structured logging; avoid `println!` or `eprintln!` in application code. For performance profiling, use `debug!()` for non-hot paths and `trace!()` for debug scenarios.
- **Logging Cleanliness**: After an issue has been resolved, remove temporary `debug!()` and `tracing::debug!()` calls in the relevant code.
- **Documentation**: Use `//!` for crate/module docs, `///` for public APIs, and `//` for internal comments.
- **Linting**: No wildcard imports. Treat cognitive complexity ≤30 and missing docs warnings as review targets rather than guaranteed compiler-enforced limits.
- **Best Practices**: Read the best practices at https://www.djamware.com/post/68b2c7c451ce620c6f5efc56/rust-project-structure-and-best-practices-for-clean-scalable-code and apply them to the project where relevant.

## Team Workflow

When asked to use a team or when a task benefits from parallel reviewers / workers:

1. **Create the team**: Use `team_create` with an appropriate `blueprint` (e.g. `code-review`).
   **Always pass `context`** — the user's specific request details: which directories/files to
   target, what task to perform, and where to write output. This context is prepended to every
   teammate's spawn prompt so they know exactly what to work on.
2. **Wait for results**: Call `team_wait` after creation. This blocks until every teammate becomes idle. **Do NOT use `wait_tasks` for teammates — `wait_tasks` only tracks `new_task` sub-agents.**
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

## Versioning

1. During development the release number will have `-beta` appended to the end in line with semantic versioning conventions. Only remove it for a production release.

## Temporary Files

1. Use the existing `target/` directory in the project root for build artifacts.
2. Create and use a `target/temp` directory for temporary files, scripts, and other ephemeral items that would normally be placed in `/tmp`.
3. Ensure that the `target/temp/` path is present in `.gitignore`.

## Priorities

- `0` — Critical (security, data loss, broken builds)
- `1` — High (major features, important bugs)
- `2` — Medium (default, nice-to-have)
- `3` — Low (polish, optimization)
- `4` — Backlog (future ideas)

## Task Tracking

- Use `todo_read` and `todo_write` to track tasks
- Always mark the task as "done" when work on a task is done.

For more details, see README.md and QUICKSTART.md.
