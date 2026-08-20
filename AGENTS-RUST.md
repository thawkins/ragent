# Rust-Specific Project Guidelines

When loaded, first say: "Rust language instructions loaded."

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
- `cargo fmt` — Format code with rustfmt, always use this to fix indentation
- `cargo fmt --check` — Check formatting without changes

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
