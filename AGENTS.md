# Agent Guidelines for Rust apps
- First when you startup say "Hi im Rust Agent and I have read Agents.md" 

## Technology Stack
- **Language**: Rust edition 2021 or greater

## Build Commands
- `cargo build` - Build debug binary, timeout is 600 seconds min
- `cargo build --release` - Build optimized release binary timeout is 600 seconds min
- `cargo check` - Check code without building
- Build only debug builds unless specificaly asked to perform a `release build`

Builds can take a long time so allow up to 600 seconds for a rebuild

## Test Commands
- `cargo test` - Run all tests
- `cargo test <test_function_name>` - Run specific test function
- `cargo test -- --nocapture` - Run tests with output visible
- `cargo test --lib` - Test library only (skip integration tests)
- **Test Timeout**: All test runs should have a 10-minute timeout to prevent hanging
  - Use `timeout 600 cargo test` on Unix/Linux
  - Use `cargo test --test-threads=1` for sequential execution if needed

### Test Organization

All tests **MUST** be located in the `tests/` inside each crate, if the test is at root level then it should be at the root tests/folder, NOT inline in source files:
- Use `#[test]` for sync tests and `#[tokio::test]` for async tests
- Import from the public `ragent` crate 
- Be organized with related tests grouped together
- Follow naming convention: `test_<component>_<scenario>` (e.g., `test_jog_x_positive`)
- For each project crate, reorganise tests by:  migrating all tests related to the crate into relevant subfolders in the tests folder in the crate, review tests both inside crate and outside of it to find candidate tests for migration, then look at all .rs files that are outside of the tests folder in the crate and relocate all the inline tests found within them into seperate files in suitable subfolders in the crate tests folder 


## Lint & Format Commands
- `cargo clippy` - Run linter with clippy
- `cargo fmt` - Format code with rustfmt
- `cargo fmt --check` - Check formatting without changes

## Units ##
- DateTime values should be represented internaly in UTC and translated to locale based represetations in the UI layer. 
- Dimensional units should be represented internaly in mms, and be of type f32, and mm values should be represted to 2 decimal place accuracy. 
- All text strings where feasable should be internaly represented in UTF8 encoding, with translation to and from UI encoding in the UI layer if required. 

## GitHub Access
- Use "gh" to access all GitHub repositories.
- When asked to "push to remote", update the SPEC.md, README.md, STATS.md, RELEASE.md, QUICKSTART.md and CHANGELOG.md files with all recent activity and spec changes, construct a suitable commit message based on recent activity, commit all changes and push the changes to the remote repository.
- When asked to "push release to remote", update the release number, and then follow the "push to remote" process. **Commit Message Rule**: Do not use "chore: bump version to ...", instead use "Version: <version_number>".
- When initializing a new repo, add BUG, FEATURE, TASK and CHANGE issue templates only do this once. 
- **CRITICAL**: Do not push changes to remote unless specifically told to. This is a strict rule.
- Do not tag releases unless specifically told to. 

## Changelog Management
- **CHANGELOG.md**: Maintain a changelog in the root directory documenting all changes before each push to remote.
- **Format**: Follow Keep a Changelog format (https://keepachangelog.com/)
- **Update Timing**: Update CHANGELOG.md before each push to remote with the latest changes, features, fixes, and improvements.
- **Version**: Use semantic versioning (major.minor.patch-prerelease)
- **RELEASE.md**: write the version number and the most recent CHANGELOG.md entry to the RELEASE.md file for use as a Description in the Github Releases page. 
- Whenever a new feature or function is added ensure that SPEC.md and QUICKSTART.md is updated if relevant. 

## Documentation Standards
- For all functions create DOCBLOCK documentation comments above each function that describes the purpose of the function, and documents any arguments and return values.
- For all modules place a DOCBLOCK at the top of the file that describes the purpose of the module, and any dependencies.
- **Documentation Files**: All documentation markdown files (*.md) **MUST** be located in the `docs/` folder, except for `QUICKSTART.md`, `RELEASE.md`, `STATS.md`,  `SPEC.md`, `AGENTS.md`, `README.md`, `PLAN.md` and `CHANGELOG.md` which remain in the project root. This includes: implementation guides, architecture documentation, feature specifications, task breakdowns, user guides, API references, and any other markdown documentation. Any future documentation should be created in the docs/ folder following this convention.
- Do not create explainer documents or other .md files unless specifically asked to.

## Code Style Guidelines
- **Formatting**: 4 spaces, max 100 width, reorder_imports=true, Unix newlines
- **Naming**: snake_case for functions/variables, PascalCase for types/structs/enums
- **Imports**: Group std, external crates, then local modules; reorder automatically
- **Error Handling**: Use `Result<T, E>` with `?`, `anyhow::Result` for main, `thiserror` for custom errors
- **Types**: Prefer explicit types, use type aliases for complex types
- **Logging**: Use `tracing` crate with structured logging, avoid `println!` or `eprintln!` in any phase of development. Performance profiling: Use `debug!()` for non-hot paths, `trace!()` for debug scenarios
- **Logging Cleanliness** after an issue has been resolved remove all debug! and tracing::debug! calls in the relevant code. 
- **Documentation**: `//!` for crate docs, `///` for public APIs, `//` for internal comments
- **Linting**: No wildcard imports, cognitive complexity ≤30, warn on missing docs
- **Best Practices**: Read the best practices at https://www.djamware.com/post/68b2c7c451ce620c6f5efc56/rust-project-structure-and-best-practices-for-clean-scalable-code and apply to the project.

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
team_create blueprint="code-review" context="Review the crates/ragent-server directory for security, test coverage, and performance issues. Write findings to COMPLIANCE.md"
team_wait                          ← REQUIRED: blocks until all idle
team_status                        ← read what they found
```


2. Don't suggest features unless asked to.
3. When debugging problems, use Occam's razor and assume that the simplest solution is more likely to be the right one. 
4. Also when you are trying to debug a problem, change only one thing at a time, if it does not fix the problem then revert it, before trying another possible solution. 
5. DO NOT perform tempoary solutions or fixes, always provide a complete solution. 
6. DO NOT declare an issue as fixed unless it has been confirmed, 90% of assertions of completion turn out to be false. 

## Versioning

1. During development the release number will have "-alpha" appended to the end as per semantic versioning standards. only when it is a production release will it be removed. 

## Temporary Files

1. Create a directory called "target" in the project root
2. Create a directory called "temp" in the target folder
3. Ensure that the target/temp folder is in the .gitignore file
4. Use target/temp for all temporary files, scripts and other ephemeral items that are normally placed in /tmp


### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

For more details, see README.md and QUICKSTART.md.
