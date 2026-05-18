# Implementation Plan: Configured Provider Selection & Model Persistence

**Spec ID:** `selectConfigedProvider`

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Add `get_configured_providers()` enumeration function | FR-001, FR-005 | M | Critical | — |
| T-002 | Add `SelectConfiguredProvider` dialog step variant | FR-002 | S | Critical | T-001 |
| T-003 | Persist per-provider last-model on model selection | FR-003 | S | High | — |
| T-004 | Restore persisted model when a configured provider is selected | FR-003, FR-004 | M | High | T-003 |
| T-005 | Validate restored model against current provider model list | FR-004 | M | High | T-004 |
| T-006 | Wire `/model` slash command to show configured-provider picker | FR-002 | S | Critical | T-002 |
| T-007 | Render `SelectConfiguredProvider` dialog in layout | FR-002 | S | Critical | T-002 |
| T-008 | Handle key events for `SelectConfiguredProvider` step | FR-002 | S | Critical | T-002 |
| T-009 | Clear per-provider model when provider is reset | FR-005 | S | Medium | T-003 |
| T-010 | Add integration tests for the full provider-selection flow | FR-001–FR-005 | M | High | T-001–T-009 |

---

## Task Details

### T-001 — Add `get_configured_providers()` Enumeration Function

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/app.rs`

Extend the existing `detect_provider()` logic into a function that returns **all**
configured providers rather than just the first match.

**What to do:**
1. Add a new public associated function `App::get_configured_providers(storage: &Storage) -> Vec<ConfiguredProvider>`.
2. Walk `PROVIDER_LIST` in order. For each entry, reuse the credential-checking logic
   already present in `detect_provider()` (environment variables, database keys,
   auto-discovery, and the `is_disabled` guard).
3. Return a `Vec` ordered by `PROVIDER_LIST` priority.
4. The existing `detect_provider()` may be refactored to call this new function and
   return `.first().cloned()`, keeping its existing public API intact for backward
   compatibility.

**Acceptance:**
- `get_configured_providers()` returns the expected providers when tested with
  mocked environment variables and storage entries.

---

### T-002 — Add `SelectConfiguredProvider` Dialog Step Variant

**Crate:** `ragent-tui`  
**Files:**
- `crates/ragent-tui/src/app/state.rs` — enum variant
- `crates/ragent-tui/src/layout.rs` — renderer
- `crates/ragent-tui/src/input.rs` — key handler

Add a new variant to the `ProviderSetupStep` enum:

```rust
/// Choosing which configured provider to switch to (model selection mode).
SelectConfiguredProvider {
    /// Configurable providers that have usable credentials.
    providers: Vec<ConfiguredProvider>,
    /// Index of the highlighted provider.
    selected: usize,
},
```

This variant is distinct from `SelectProvider` (which lists ALL providers for first-time
setup) and serves the `/model` workflow where the user is switching between already-active
providers.

**Acceptance:**
- The variant compiles and is added to all relevant `match` arms.
- Placeholder rendering and key handling produce a usable dialog (detailed in T-007/T-008).

---

### T-003 — Persist Per-Provider Last-Model on Model Selection

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/input.rs`

When the user confirms a model selection (the `ProviderSetupStep::Done` transition or
equivalent code path), write the model ID to storage.

**What to do:**
1. In the code path that finalises a model choice (both from `SelectModel` and from
   the direct-restore path in T-004), call:
   ```rust
   storage.set_setting(&format!("provider_{}_last_model", provider_id), &model_id)?;
   ```
2. The write must happen before or at the same point where `selected_model` is updated
   (so both are durable together).
3. Ensure the write is gated behind the storage handle being available (the `App` struct
   always holds an `Arc<Storage>`).

**Acceptance:**
- After selecting "anthropic/claude-sonnet-4-20250514", the storage key
  `provider_anthropic_last_model` contains `"claude-sonnet-4-20250514"`.

---

### T-004 — Restore Persisted Model When a Configured Provider Is Selected

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/input.rs`

When the user picks a provider from the `SelectConfiguredProvider` dialog, before showing
the model picker, check storage for a persisted model.

**What to do:**
1. In the key-handler for `SelectConfiguredProvider` (or a helper method on `App`),
   read `provider_<id>_last_model` from storage immediately after the user confirms a
   provider choice.
2. If a non-empty value is found, attempt to validate it via T-005.
3. If the model is valid, set `app.selected_model` to `"<provider_id>/<model_id>"`,
   update `selected_model_ctx_window` and `selected_thinking_level` as appropriate,
   and transition directly to `ProviderSetupStep::Done` (skipping the model picker).
4. If no persisted model exists, transition to `ProviderSetupStep::SelectModel` with
   the provider's model list (existing behaviour).

**Acceptance:**
- Switching to a previously-used provider restores the last model without showing the
  model picker.
- First-time use of a provider (no persisted key) still shows the model picker.

---

### T-005 — Validate Restored Model Against Current Provider Model List

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/app.rs`

Ensure that a persisted model ID is still valid before silently restoring it.

**What to do:**
1. In the restore path (T-004), call `self.models_for_provider(provider_id)` to obtain
   the current model list (this function already handles fallback entries).
2. Check whether any entry in the list has a matching `id` (case-insensitive, since
   model IDs can vary in casing across providers).
3. If found: proceed with restore.
4. If NOT found:
   - Delete the stale `provider_<id>_last_model` key from storage.
   - Transition to `ProviderSetupStep::SelectModel` with the full model list.
   - Optionally set a brief status-bar message: `"Previous model <X> is no longer
     available — please choose a new one."`

**Acceptance:**
- Persisted model `"gpt-4-turbo"` for OpenAI, when removed from the provider's model list,
  triggers the fallback model picker and the stale key is pruned.

---

### T-006 — Wire `/model` Slash Command to Show Configured-Provider Picker

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/app.rs`

Currently the `/model` slash command (see `app.rs` around line 5410) transitions to
`ProviderSetupStep::SelectModel` showing models for the *current* provider. Update it
to first show the configured-provider picker.

**What to do:**
1. Replace the direct `SelectModel` transition in the `/model` handler with:
   ```rust
   let configured = Self::get_configured_providers(&self.storage);
   match configured.len() {
       0 => { /* show "no providers" message */ }
       1 => { /* auto-select the sole provider, then proceed to model restore/picker */ }
       _ => {
           self.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
               providers: configured,
               selected: 0,
           });
       }
   }
   ```
2. The single-provider fast path must invoke the same restore/fallback logic as the
   multi-provider dialog's Enter key handler.

**Acceptance:**
- `/model` with 3 configured providers shows a 3-item picker.
- `/model` with 1 configured provider immediately restores the model (or shows the
  model picker if no persisted model exists).
- `/model` with 0 configured providers shows a status message and no dialog.

---

### T-007 — Render `SelectConfiguredProvider` Dialog in Layout

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/layout.rs`

Add a match arm in `render_provider_setup_dialog()` for the new step variant.

**What to do:**
1. Copy the rendering logic from the existing `ProviderSetupStep::SelectProvider` arm
   as a starting point.
2. Iterate over `providers` instead of `PROVIDER_LIST`.
3. Change the dialog title from `" Provider Setup "` to `" Switch Provider "` (or
   similar) to distinguish it from the first-time setup dialog.
4. Optionally show a small checkmark or `✓` icon next to each provider to reinforce
   that they are already configured.
5. Keep the same footer: `"↑/↓ navigate  Enter select  Esc cancel"`.

**Acceptance:**
- The dialog renders with the correct title, configured providers only, and correct
  selection highlighting.

---

### T-008 — Handle Key Events for `SelectConfiguredProvider` Step

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/input.rs`

Add a match arm in `handle_provider_setup_key()` for the new step variant.

**What to do:**
1. Handle `Up`/`Down`/`k`/`j` keys for navigation (same as `SelectProvider`).
2. On `Enter`: read the selected `ConfiguredProvider`, invoke the restore/fallback flow
   from T-004/T-005.
3. On `Esc`: clear `app.provider_setup` (abort).
4. Ensure the key handler does not block on network calls — model-list resolution is
   synchronous (`models_for_provider` is in-memory for registered providers).

**Acceptance:**
- Arrow keys navigate the list correctly.
- Enter triggers provider selection and transitions to either `Done` or `SelectModel`.
- Esc dismisses the dialog cleanly.

---

### T-009 — Clear Per-Provider Model When Provider Is Reset

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/src/input.rs` (or `app.rs`)

In the existing `ProviderSetupStep::ResetProvider` handler (or wherever credential
deletion occurs), also delete the `provider_<id>_last_model` key.

**What to do:**
1. Find the code path that deletes the provider credential (the `delete_setting` call
   for `provider_<id>_key`).
2. Add an adjacent `storage.delete_setting(&format!("provider_{}_last_model", provider_id))` call.
3. If `selected_model` currently references this provider, clear it as well (or let it
   naturally fall back on the next `/model` invocation — the existing `detect_provider()`
   at startup will already skip the disabled provider).

**Acceptance:**
- After resetting Anthropic, `provider_anthropic_last_model` is absent from storage.
- Resetting the currently-selected provider causes no crash; the status bar or startup
  logic handles the missing provider gracefully.

---

### T-010 — Add Integration Tests for the Full Provider-Selection Flow

**Crate:** `ragent-tui`  
**Files:** `crates/ragent-tui/tests/test_configured_provider_selection.rs` (new file)

**Test cases:**

| Test | What it verifies |
|---|---|
| `test_get_configured_providers_env_vars` | FR-001 — env-var-based providers are enumerated |
| `test_get_configured_providers_db_keys` | FR-001 — database-credential providers are enumerated |
| `test_get_configured_providers_disabled_excluded` | FR-001, FR-005 — disabled providers are excluded |
| `test_get_configured_providers_auto_discovered` | FR-001 — auto-discovered tokens (Copilot) are included |
| `test_single_configured_provider_skips_picker` | FR-002 — single provider fast-path |
| `test_zero_configured_providers_shows_message` | FR-002 — no-provider case |
| `test_model_persistence_write_and_read` | FR-003 — write and recall of `provider_<id>_last_model` |
| `test_model_persistence_restore_on_switch` | FR-003 — end-to-end restore flow |
| `test_model_fallback_when_stale` | FR-004 — stale model triggers picker and pruning |
| `test_model_persistence_cleared_on_reset` | FR-005 — reset clears per-provider model |
| `test_configured_picker_esc_cancel` | FR-002 — Esc dismisses dialog |
| `test_configured_picker_enter_select` | FR-002 — Enter selects provider |

**Acceptance:** All tests pass with `cargo test -p ragent-tui -- test_configured_provider`.

---

## Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| `Storage::set_setting` call fails during model persistence | Model not saved; user sees picker again next time | Low | `set_setting` is already robust; wrap in `let _ =` to avoid panics |
| `PROVIDER_LIST` order changes cause unintended default | Provider priority shifts | Low | Use `PROVIDER_LIST` ordering consistently; document that order matters |
| Copilot auto-discovery is slow (file I/O) | Dialog open blocks on discovery | Medium | Call `get_configured_providers()` lazily; memoize result until next `/model` invocation |
| Model ID casing mismatch between persisted value and provider list | False "stale" detection | Medium | Normalise to lowercase during comparison (T-005) |

---

## Estimated Total Effort

| Effort | Count |
|---|---|
| S | 5 tasks |
| M | 5 tasks |
| **Total** | **10 tasks** |

Approximate wall-clock estimate: **2–3 days** for a single developer familiar with the
ragent TUI codebase.
