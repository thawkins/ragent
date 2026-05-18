# Specification: Configured Provider Selection & Model Persistence

**Spec ID:** `selectConfigedProvider`

---

**status:** draft
**created:** 2026-04-21
**author:** ragent-agent
**version:** 1.0.0-draft

---

## Executive Summary

This specification defines a feature that streamlines the provider-selection workflow by
presenting only **configured** LLM providers (those with a valid API key or credential)
rather than the full provider catalogue. It persists the last-selected model on a
per-provider basis so that returning to a previously used provider automatically restores
the model choice. When no prior model exists, or the stored model is no longer available
from that provider, the system falls back to presenting the full model list for selection.

## Scope & Objectives

### Scope

- Provider-configuration detection: identify which providers have usable credentials.
- A TUI dialog that lists only configured providers when the user invokes a provider
  switcher (e.g. `/model` slash command or equivalent keyboard shortcut).
- Per-provider model persistence: store and recall the last model ID chosen for each
  provider whose key is present.
- Graceful fallback: when a stored model ID is absent from the provider's current model
  list (e.g. the provider has deprecated or renamed the model), present the full model
  picker instead of failing silently.
- The feature applies to all providers in `PROVIDER_LIST`, including those using
  environment variables, database-stored credentials, and auto-discovered tokens
  (e.g. GitHub Copilot IDE integration).

### Out of Scope

- Changing the mechanism by which API keys are stored or validated.
- Altering the `SelectProvider` dialog that shows when the user explicitly configures a
  new provider for the first time (`/setup` command).
- Multi-account support for a single provider.
- Cloud-sync of persisted model selections across machines.

### Objectives

1. Reduce the number of key-presses required to switch between already-configured providers.
2. Eliminate the friction of re-selecting a model every time the user returns to a provider.
3. Prevent the user from seeing providers that cannot be used because no credential exists.
4. Handle model deprecation gracefully without requiring manual cleanup.

---

## Requirements

### FR-001 — Enumerate Configured Providers (Ubiquitous)

`The <Configured Provider Selection system> shall <return the subset of PROVIDER_LIST entries
for which a valid API key, database credential, or auto-discovered token currently exists>.`

**Acceptance criteria:**
- The function accepts the `Storage` handle and the application environment.
- It returns a `Vec<ConfiguredProvider>`, ordered by `PROVIDER_LIST` priority.
- A provider is considered *configured* when one of the following holds:
  - Its expected environment variable is non-empty.
  - A database-stored credential exists (e.g. `provider_<id>_key` in storage).
  - An auto-discovery path succeeds (e.g. Copilot IDE token file).
- Providers that have been explicitly *disabled* via `provider_<id>_disabled` are excluded.
- The function is accessible from both the TUI layer and the HTTP-server layer.

### FR-002 — Present Configured-Provider Picker Dialog (Event-Driven)

`When <the user invokes the provider/model switcher (e.g. "/model" slash command or
equivalent keyboard shortcut)>, the <Configured Provider Selection system> shall <display
a dialog that lists only the configured providers enumerated by FR-001>.`

**Acceptance criteria:**
- If exactly one provider is configured, the dialog is skipped and that provider is
  selected immediately.
- If zero providers are configured, a brief message ("No providers configured — use
  /setup to add one") is displayed and no dialog appears.
- The dialog reuses the existing `ProviderSetupStep::SelectProvider` rendering and
  key-handling infrastructure.
- The dialog title reflects that providers shown are "available" or "configured".
- The user may cancel (Esc) to abort the switch.

### FR-003 — Per-Provider Model Persistence (State-Driven)

`While <a provider is selected and a model is chosen>, the <Configured Provider Selection
system> shall <persist the model ID in durable storage keyed by the provider ID>.`

`When <a provider is re-selected at a later time>, the <Configured Provider Selection
system> shall <read the persisted model ID and attempt to restore it>.`

**Acceptance criteria:**
- Persistence keys follow the pattern `provider_<id>_last_model` (e.g.
  `provider_anthropic_last_model`).
- The model ID is persisted immediately upon selection (in the `Done` step or equivalent).
- The existing `selected_model` setting continues to hold the global "current model" in
  `"provider/model"` format and is NOT replaced by this feature — per-provider persistence
  is separate.
- If the persisted model ID is empty or absent, the system treats it as "no prior model"
  and invokes the model-list fallback (FR-004).

### FR-004 — Model-Availability Fallback (Optional)

`Where <a persisted model ID is not found in the provider's current model list>, the
<Configured Provider Selection system> should <present the full model picker for that
provider instead of selecting a stale or invalid model>.`

**Acceptance criteria:**
- The system calls `ProviderRegistry::resolve_model()` (or the equivalent
  `models_for_provider()` path) to validate the persisted model ID.
- If the model is present in the resolved list, it is selected without showing the picker.
- If the model is absent, the dialog transitions to `ProviderSetupStep::SelectModel` with
  the full model list for that provider.
- No error is emitted; the transition is transparent to the user.
- The stale persisted entry is **pruned** (deleted from storage) after the user selects a
  new model so it does not cause repeated fallbacks.

### FR-005 — Prevention of Dead-End Provider Selection (Unwanted)

`If <a provider that was previously configured becomes unconfigured (e.g. the user
de-authorises via "/setup reset")>, the <Configured Provider Selection system> shall
<exclude it from the picker list on the next invocation>.`

`The <Configured Provider Selection system> shall <NOT retain a previously selected model
for a provider that is no longer configured>; instead the per-provider persisted model
key shall be cleared when the provider is reset.`

**Acceptance criteria:**
- The provider-reset flow in `ProviderSetupStep::ResetProvider` clears
  `provider_<id>_last_model`.
- The picker dialog re-enumerates configured providers on every invocation; it does not
  cache the list from a previous call.
- When the user deletes a credential via `/setup reset`, the provider immediately
  disappears from the configured list on the next `/model` invocation.

---

## Non-Functional Requirements

### NFR-001 — Performance
- Enumerating configured providers and validating a persisted model must each complete
  in under 10 ms (memory-bound key checks plus one storage read).

### NFR-002 — Reliability
- Model-persistence writes must use `Storage::set_setting` (or equivalent atomic
  write) so that a crash during persistence does not leave a partial key.

### NFR-003 — Usability
- The dialog must use the same visual style, key bindings (↑/↓/Enter/Esc), and
  accessibility features as the existing `SelectProvider` dialog.
- The fallback to model selection must include a subtle indication that the previous
  model is no longer available (e.g. a brief status-bar message).

### NFR-004 — Backward Compatibility
- The existing `selected_model` setting and its startup-load behavior in `App::new()`
  must continue to work unchanged.
- The existing `SelectProvider` dialog (showing ALL providers for `/setup`) must not
  be altered by this feature.

---

## Interfaces & Dependencies

### Internal Interfaces
| Interface | Crate | Purpose |
|---|---|---|
| `Storage::get_setting / set_setting` | `ragent-storage` | Persist and recall per-provider model keys |
| `ProviderRegistry::list / resolve_model` | `ragent-llm` | Enumerate available models for validation |
| `App::detect_provider` | `ragent-tui` | Existing single-provider detection (extended by this feature) |
| `PROVIDER_LIST` | `ragent-tui` | Canonical provider catalogue |
| `ProviderSetupStep` | `ragent-tui` | Dialog state machine (new variant added) |
| `render_provider_setup_dialog` | `ragent-tui` | Dialog renderer (new step rendered) |
| `handle_provider_setup_key` | `ragent-tui` | Key handler (new step handled) |

### External Dependencies
None. This feature is entirely self-contained within the existing ragent crates.

---

## Glossary

| Term | Definition |
|---|---|
| **Configured Provider** | A provider for which ragent holds a usable credential (API key, PAT, or auto-discovered token). |
| **Per-provider model persistence** | Storing the last-chosen model ID in a setting keyed by provider ID so it can be restored when the provider is re-selected. |
| **Model-availability fallback** | When a stored model is no longer offered by the provider, falling back to the full model picker rather than failing. |
| **Provider picker** | The TUI dialog that lists configured providers for the user to choose from. |
