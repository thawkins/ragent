# /simplify Review — 2026-08-21

Reviewed files changed in the last 3 commits (`HEAD~3..HEAD`).

## Issues Found and Fixed

### 1. Dead-code / Noise: `/*FR-010 export*/` comments in `slash.rs`

**Category:** dead-code / noise
**Severity:** Low (code clutter)
**File:** `crates/ragent-tui/src/app/slash.rs`

**Problem:** 121 `return;` statements were annotated with `/*FR-010 export*/` trailing comments. These provide no runtime or semantic value — they appear to be speculative traceability markers from a spec implementation, but they clutter every early-return in the file and make diffs noisy.

**Fix applied:** Removed all 121 `/*FR-010 export*/` trailing comments via `sed`, restoring clean `return;` statements.

### 2. Performance: Unnecessary `html_buf.clone()` in `render_markdown_pipeline`

**Category:** performance (unnecessary allocation)
**Severity:** Low
**File:** `crates/ragent-tui/src/app/models.rs`, line ~218

**Problem:** `render_markdown_pipeline` builds an `html_buf: String`, then clones it into `html_owned` for a spawned thread — but `html_buf` is never used again after the clone. The clone is a needless heap allocation of the entire HTML string.

**Fix applied:** Changed `let html_owned = html_buf.clone();` to `let html_owned = html_buf;` (move), eliminating the copy.

### 3. Duplication: Identical research-bypass guards in two render methods

**Category:** duplication
**Severity:** Medium
**File:** `crates/ragent-tui/src/app/models.rs`, lines ~143-156 and ~176-185

**Problem:** `render_markdown_to_ascii` and `render_markdown_unconditionally` both contained the same two guard clauses:
1. `try_extract_research_code_block(text)` → return pre-rendered research
2. `text.starts_with("🔬 Research Progress")` → return sanitized text

The code was copy-pasted between the two methods.

**Fix applied:** Extracted a shared `fn bypass_research_text(text: &str) -> Option<String>` helper. Both methods now call `Self::bypass_research_text(text)` and return the bypassed value if present. The non-research guard (`!text.starts_with("From: /")`) remains only in `render_markdown_to_ascii` since it is specific to that method's contract.

### 4. Duplication: Agent setup boilerplate in `dispatch_bang_command` and `dispatch_user_message`

**Category:** duplication
**Severity:** Medium
**File:** `crates/ragent-tui/src/app/session_ops.rs`, lines ~121-131 and ~240-250

**Problem:** Both `dispatch_bang_command` and `dispatch_user_message` contained the identical 10-line block:
```rust
let mut agent = self.agent_info.clone();
self.apply_selected_model_and_thinking(&mut agent);
if let Some(ref mode) = self.role_mode {
    let addition = mode.system_prompt_addition();
    if !addition.is_empty() {
        let existing = agent.prompt.clone().unwrap_or_default();
        agent.prompt = Some(format!("{existing}\n\n{addition}"));
    }
}
```

**Fix applied:** Extracted `fn prepare_agent_for_dispatch(&self) -> AgentInfo` that encapsulates cloning, model/thinking application, and role-mode prompt injection. Both call sites now use `let agent = self.prepare_agent_for_dispatch();`.

## Issues Noted but Not Fixed (pre-existing / pervasive)

### 5. `self.store.lock().unwrap()` — 25 instances in `codeindex/src/lib.rs`

**Category:** error-handling
**Severity:** Low (pre-existing pattern)
**File:** `crates/ragent-codeindex/src/lib.rs`

**Problem:** 25 call sites use `self.store.lock().unwrap()`. If any thread panics while holding the `Mutex` guard, the mutex becomes poisoned and every subsequent `.unwrap()` will panic the calling thread. The 6 new graph methods (`godnodes`, `path`, `explain`, `communities`, `build_graph`, `build_graph_for_language`) add to this count.

**Why not fixed:** This is a pervasive, consistent pattern across the entire file — changing it would require touching 25+ call sites and is a design-level decision (poisoned-mutex recovery strategy) that goes beyond the scope of a simplify pass. Notably, the `try_*` variants correctly use `try_lock()` with graceful `Ok(None)` fallback.

### 6. Codeindex graph `try_*` methods — repetitive lock-and-construct boilerplate

**Category:** duplication
**Severity:** Low
**File:** `crates/ragent-codeindex/src/lib.rs`, lines ~432-519

**Problem:** `try_godnodes`, `try_path`, `try_explain`, and `try_communities` all follow the identical pattern:
```rust
let store = match self.store.try_lock() {
    Ok(g) => g,
    Err(_) => return Ok(None),
};
let graph = graph::SymbolGraph::new(&store);
Ok(Some(graph.<method>(...)?))
```

**Why not fixed:** A macro or closure-based helper would reduce the 4 copies, but the `?` operator and differing return types (`Option<Option<T>>` nesting) make a clean abstraction non-trivial without adding complexity. Left as-is since the pattern is clear and each method is only ~8 lines.

## Verification

- `cargo check -p ragent-tui` — clean (no warnings)
- `cargo clippy -p ragent-tui` — clean (no warnings)
- `cargo fmt` — applied
- `cargo test -p ragent-tui --lib` — 62 passed, 0 failed
- `cargo test -p ragent-agent --lib` — 314 passed, 0 failed