# ragent-prompt_opt

Prompt optimization templates and the `Completer` abstraction for ragent.
Provides 12 structured prompt frameworks and platform adapters with no LLM
call required for template selection.

## Workspace Dependencies

None. This crate has no `ragent-*` workspace dependencies.

## External Dependencies

- async-trait
- anyhow

Dev-dependencies: tokio (rt, macros).

## Public API (crate root)

All items are declared directly in `lib.rs` — there are no `pub mod` declarations.

- **Completer** (trait) — Thin async abstraction over a single LLM completion call; implementors send `system` as the system prompt and `user` as the user message, returning the full response as a `String`. Decorated with `#[async_trait]`, requires `Send + Sync`.
- **OptMethod** (enum) — The 12 supported prompt optimization frameworks. Variants: `CoStar`, `Crispe`, `ChainOfThought`, `Draw`, `Rise`, `O1Style`, `MetaPrompting`, `Variational`, `QStar`, `OpenAI`, `Claude`, `Microsoft`. Derives `Debug, Clone, Copy, PartialEq, Eq, Hash`.
- **OptMethod::from_str** (impl via `std::str::FromStr`) — Parses a case-insensitive method name or alias (e.g. `"cot"`, `"co-star"`, `"q*"`, `"azure"`) into an `OptMethod`.
- **OptMethod::name** (method, `const`) — Canonical short name (`&'static str`).
- **OptMethod::description** (method, `const`) — Human-readable one-line description.
- **OptMethod::all** (method, `const`) — Returns `&'static [OptMethod]` of all 12 variants in display order.
- **OptMethod::help_table** (method) — Returns a markdown-formatted table listing all methods and descriptions.
- **system_prompt** (free function, `const`) — Given an `OptMethod`, returns the meta-prompt (system message) as `&'static str`.
- **optimize** (async free function) — Sends the method's meta-prompt (as system message) plus the user's input (as user message) to the provided `Completer` and returns the optimized prompt as `anyhow::Result<String>`.