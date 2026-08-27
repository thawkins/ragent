# Rust-Specific Project Guidelines

When loaded, first say: "Rust language instructions loaded."

## MANDATORY: Run `cargo fmt` After EVERY Rust File Edit

**THIS IS THE MOST IMPORTANT RULE IN THIS FILE. READ IT TWICE.**

Rust is NOT Python, NOT JavaScript, NOT Go. Rust files have strict, non-negotiable
formatting rules enforced by `rustfmt`. If you try to hand-format a Rust file the way
you would format Python (indentation, brace placement, spacing), you WILL get it wrong
and break the build with `cargo fmt --check` failures.

**You MUST run `cargo fmt` after writing or editing ANY `.rs` file. No exceptions.**

### The Rule

After you create, edit, or append to ANY Rust source file (`.rs`), you MUST run:

```bash
cargo fmt
```

BEFORE you do anything else. Do this for every single file you touch. Do not batch
it. Do not skip it because "the edit looked small." Do not skip it because "I already
know rustfmt style." YOU DO NOT. RUN IT.

### Why This Is Non-Negotiable

1. **You will get indentation wrong.** Rust uses 4 spaces, no tabs. Python-trained
   models frequently emit 2-space or tab-indented Rust. `cargo fmt` fixes this
   automatically.
2. **You will get brace and parenthesis spacing wrong.** `rustfmt` has specific rules
   for spacing inside `fn(...)`, `match { ... }`, struct literals, etc. that differ
   from Python/JS conventions. You cannot reproduce these by hand reliably.
3. **You will get import ordering wrong.** `rustfmt` reorders `use` statements.
   Hand-ordering them will fail `cargo fmt --check`.
4. **`cargo fmt --check` is enforced.** A file that does not pass `cargo fmt --check`
   is a broken file, period. It does not matter if the code compiles.
5. **Broken formatting wastes review cycles.** Every time you submit a Rust file that
   fails `cargo fmt --check`, a human or CI must fix your mistake. This is
   unacceptable.

### Enforcement Procedure

For every Rust file you edit, the workflow is:

```
edit/create the .rs file  →  run `cargo fmt`  →  verify `cargo fmt --check` passes
```

If you edit three Rust files, you run `cargo fmt` three times (or at minimum once at
the end before you finish). NEVER mark a task complete if any edited `.rs` file has
not been formatted with `cargo fmt`.

**If you are unsure whether a file is formatted correctly, run `cargo fmt`. It is
always safe — `rustfmt` is idempotent and will only fix formatting, never change
semantics.**

### What This Replaces

Do NOT attempt to format Rust code by hand to "match existing style." Do NOT copy
indentation from a Python file. Do NOT guess at `rustfmt` rules. The ONLY reliable
source of truth for Rust formatting is the `cargo fmt` command itself. Use it.

## Technology Stack

- **Language**: Rust edition 2024 or greater

## Build Commands

Use the `Bash` tool to run the following cargo commands:

- `cargo build` — Build debug binary; allow up to 1000 seconds.
- `cargo build --release` — Build optimized release binary; allow up to 1000 seconds.
- `cargo check` — Check code without building.
- Build only debug builds unless specifically asked to perform a `release build`.

Builds can take a long time, so allow up to 1000 seconds for a rebuild.

## Test Commands

Use the Bash tool to run the following `cargo` commands

- `cargo test` — Run all tests
- `cargo test <test_function_name>` — Run specific test function
- `cargo test -- --nocapture` — Run tests with output visible
- `cargo test --lib` — Test library only (skip integration tests)
- **Test Timeout**: All test runs should have a 10-minute timeout to prevent hanging
  - Use `timeout 1000 cargo test` on Unix/Linux
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
  - **Private-item tests**: widen the tested items to `pub(crate)` where necessary and re-import the source module via `#[path = "../src/<module>.rs"] mod <module>;`. Provide shims for `super::` and `crate::` references at the test file root.
  - **Complex cases** (`//!` doc comments + `crate::` cross-module deps): use `#[cfg(test)] #[path = "../../tests/test_<module>.rs"] mod test_<module>;` in the source file to compile the external test within the crate's module tree.
- **Do not add new inline `#[cfg(test)]` modules** to library source files. All new tests go in `tests/`.

## Lint & Format Commands

Use the `Bash` tool to run the following cargo commands:

- `cargo clippy` — Run linter with clippy
- `cargo fmt` — Format code with rustfmt. **MANDATORY** to run after editing any `.rs` file. See the top of this file for the full procedure.
- `cargo fmt --check` — Check formatting without changes. Fails if any file is unformatted. This is the CI gate.

## Workspace Layout

- Keep all crates under the `crates/` directory.
- Place each crate in its own subfolder under `crates/`, named after the crate (e.g. `crates/ragent-core`).
- Keep the root `Cargo.toml` configured as a workspace manifest only. Do not declare package metadata or dependencies in the root `Cargo.toml`; list workspace members instead:

  ```toml
  [workspace]
  members = ["crates/*"]
  resolver = "2"
  ```
- Define crate-specific metadata, dependencies, and targets in each crate's own `Cargo.toml`.
- Prefer small, focused crates with a single responsibility.

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

Core Code Rules

* **No `unsafe` Blocks:** Never use `unsafe` unless explicitly approved or required by FFI bounds; document all safety invariants clearly when used.
* **Exhaustive Pattern Matching:** Always match enums and results exhaustively; avoid wildcard `_` fallbacks unless handling non-exhaustive external types.
* **Explicit Cloning:** Call `.clone()` explicitly on non-`Copy` types; forbid hidden or cascading clones inside intensive iterators or closures.
* **No Silent Error Swallowing:** Never use `.unwrap()` or `.expect()` in production code paths; always propagate or handle `Result` and `Option`

Code Style & Formatting

* **Naming Conventions:** Use `snake_case` for variables, functions, and modules; `PascalCase` for types, traits, and enums; `SCREAMING_SNAKE_CASE` for global constants.
* **Idiomatic Constructs:** Prefer expressive iterator adapters (`map`, `filter`, `fold`) over explicit manual index-based loops.
* **Formatting standard:** Adhere strictly to default `rustfmt` layouts and keep maximum line lengths under 100 characters.
* **No Unicode/Emojis:** Exclude emojis or fancy unicode symbols from comments and output code.

Workflow & Tool Enforcement

* **Zero Warnings Policy:** Generated code must compile cleanly without warnings under standard compiler checks.
* **Clippy Compliance:** Run and pass `cargo clippy` with standard lints on all proposed changes.
* **Automated Formatting Check (MANDATORY):** You MUST run `cargo fmt` on every Rust file you edit before finishing. See the "MANDATORY: Run `cargo fmt`" section at the top of this file for the full procedure. `cargo fmt` is idempotent and safe — run it freely. A task is NOT complete if any edited `.rs` file has not been formatted with `cargo fmt`. This is enforced by `cargo fmt --check` in CI; unformatted files are build failures.
