---
status: draft
---

# Azure Resource Provider – Specification

**Spec ID:** `AzureResource`

**Created:** 2026-04-21
**Author:** ragent-agent
**Version:** 1.0.0-draft

---

## Executive Summary

This specification defines an **Azure Resource** provider that lets users pre-declare a
static set of Azure-deployed models in a local JSON file (`azureresources.json`).
During provider setup, the user can browse this curated list and select a model by
friendly name rather than typing an opaque deployment ID.  The provider reuses the
existing `azure_foundry` LLM backend, but the model list is sourced from the user's
local file instead of dynamic discovery.

## Scope & Objectives

### Scope

- A new provider type `azure_resource` registered in the provider system.
- Reading and validating `azureresources.json` stored in the ragent configuration
  directory (e.g. `~/.config/ragent/` or `.ragent/`).
- Presenting the file's entries as a selectable model list in the TUI provider-setup
  dialog.
- Mapping a selected entry to a fully-qualified `provider_id/model_id` string that
  the session processor can use.
- Persisting the chosen entry so it is restored on next startup.

### Out of Scope

- Editing `azureresources.json` from inside ragent (users edit the file manually).
- Dynamic refresh or polling of the file after startup.
- Cloud-sync of the file across machines.
- Validation that the Azure endpoint actually hosts the declared models (that is
  the responsibility of the underlying `azure_foundry` provider).

### Objectives

1. Allow users who manage many Azure deployments to keep a local, human-readable
   catalogue of models.
2. Reduce setup friction by presenting friendly names instead of raw Azure
   deployment IDs.
3. Keep the file format simple, versioned, and forward-compatible.

---

## Requirements

### FR-001 — Provider Registration (Ubiquitous)

`The <Azure Resource provider> shall <be registered in the provider system under the id
"azure_resource" with a human-readable name "Azure Resource (File)">.`

**Acceptance criteria:**
- `ProviderRegistry::get("azure_resource")` returns a valid provider instance.
- The provider appears in `ProviderRegistry::list()`.
- The provider's `name()` returns `"Azure Resource (File)"`.

### FR-002 — File Location & Discovery (Ubiquitous)

`The <Azure Resource provider> shall <look for a file named "azureresources.json" in
the ragent configuration directory (resolved via the same logic used for ragent.json)>.`

**Acceptance criteria:**
- If the file exists and contains valid JSON, it is parsed successfully.
- If the file is missing, the provider behaves as if the list is empty (no error).
- If the file contains malformed JSON, a descriptive error is logged and the list is
treated as empty.
- The file path is computed once at provider initialisation and cached for the session.

### FR-003 — File Schema & Versioning (Ubiquitous)

`The <Azure Resource provider> shall <accept a JSON file whose top-level object contains
a mandatory "version" field (value "1") and a "resources" array of model entries>.`

Each entry in `resources` shall have the following fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Azure deployment ID (used in API calls). |
| `name` | string | yes | Human-readable name shown in the picker. |
| `endpoint` | string | yes | Azure AI Foundry base URL for this deployment. |
| `api_key` | string | no | API key token provided directly (takes precedence over `api_key_env`). |
| `api_key_env` | string | no* | Name of the environment variable holding the API key (*required if `api_key` is absent*). |
| `context_window` | integer | no | Context-window size in tokens. |
| `capabilities` | array of strings | no | e.g. `["streaming","tool_use","vision"]` |
| `thinking` | object | no | `{ "enabled": bool, "level": "low\|medium\|high" }` |

**Acceptance criteria:**
- Entries missing mandatory fields (`id`, `name`, `endpoint`) are skipped with a logged warning.
- At least one of `api_key` or `api_key_env` must be present; if both are absent, the entry is skipped with a logged warning.
- If `api_key` is present, `api_key_env` is optional and ignored for key resolution.
- If `api_key` is absent, `api_key_env` becomes mandatory; the entry is skipped if `api_key_env` is missing.
- Extra fields are ignored (forward compatibility).
- The `version` field must equal `"1"`; other versions are rejected with a logged error.
- The parsed entries are converted into `ModelInfo` structs internally.

### FR-004 — Model List Presentation (Event-Driven)

`When <the user selects the "azure_resource" provider in the provider setup dialog>,
the <Azure Resource provider> shall <display a list of the entries from
azureresources.json ordered as they appear in the file>.`

**Acceptance criteria:**
- The list shows the `name` field of each entry.
- If the file is missing or contains no valid entries, the dialog shows a message
  such as "No Azure resources found. Create azureresources.json in your config directory."
- The user may cancel (Esc) to abort setup.
- The dialog reuses the existing `ProviderSetupStep::SelectModel` infrastructure.

### FR-005 — Entry Selection & Persistence (State-Driven)

`While <an entry is selected and confirmed>, the <Azure Resource provider> shall
<persist the chosen entry's `id`, `endpoint`, `api_key`, and `api_key_env` in durable
storage keyed by "azure_resource_last_selection">.`

`When <the provider is re-selected at a later time>, the <Azure Resource provider>
shall <read the persisted selection and attempt to restore it>.`

**Acceptance criteria:**
- Persistence uses `Storage::set_setting` with key `azure_resource_last_selection`.
- The stored value is a JSON object containing `id`, `endpoint`, `api_key`, and `api_key_env`.
- On re-selection, if the persisted entry is still present in the current file, it is
  pre-highlighted in the list.
- If the persisted entry is no longer in the file, the list falls back to the first
  entry and the stale persistence key is deleted.

### FR-006 — Provider Resolution After Selection (Event-Driven)

`When <the user confirms an entry>, the <Azure Resource provider> shall <resolve to the
"azure_foundry" backend using the selected entry's `endpoint` and the API key from the
entry's `api_key` field if present, otherwise from the `api_key_env` environment variable>.`

**Acceptance criteria:**
- The session processor treats the resolved provider as `azure_foundry`.
- The model ID passed to the backend is the entry's `id` (Azure deployment ID).
- The base URL is the entry's `endpoint`.
- If `api_key` is present in the entry, it is used directly as the API key.
- If `api_key` is absent, the API key is read from the environment variable named by `api_key_env` at runtime.
- If neither `api_key` is present nor the environment variable is set, a clear error message is shown.

### FR-007 — Capability & Thinking Overrides (Optional)

`Where <an entry in azureresources.json specifies optional fields (context_window,
capabilities, thinking)>, the <Azure Resource provider> should <apply those values as
overrides when constructing the ModelInfo for the selected entry>.`

**Acceptance criteria:**
- `context_window` overrides the default (128_000).
- `capabilities` replaces the default empty set.
- `thinking` is stored in the `ModelInfo` for use by the o-series reasoning path.
- If optional fields are absent, safe defaults are used.

### FR-008 — Prevention of Duplicate IDs (Unwanted)

`If <azureresources.json contains multiple entries with the same `id`>, the <Azure
Resource provider> shall <skip all but the first occurrence and log a warning>.`

**Acceptance criteria:**
- Duplicate IDs do not cause runtime errors.
- The warning includes the duplicated `id` and the line or index where it was found.

---

## Non-Functional Requirements

### NFR-001 — Performance
- Parsing `azureresources.json` must complete in under 5 ms for files with ≤ 100 entries.

### NFR-002 — Reliability
- Malformed JSON or invalid entries must never crash the provider; they are skipped
  with logged warnings.

### NFR-003 — Backward Compatibility
- Adding new optional fields to the entry schema in future versions must not break
  existing files.

### NFR-004 — Security
- Direct `api_key` values in `azureresources.json` are permitted for convenience but
  **not recommended** for shared or version-controlled environments.  The preferred
  pattern is to store the key in an environment variable and reference it via `api_key_env`.
- When `api_key` is present, it is stored in durable persistence alongside the other
  entry metadata; implementers must ensure this is treated as sensitive data.

---

## Interfaces & Dependencies

| Interface | Crate | Purpose |
|---|---|---|
| `ProviderRegistry` | `ragent-llm` | Register the new provider |
| `Storage::get_setting / set_setting` | `ragent-storage` | Persist last selection |
| `ProviderSetupStep::SelectModel` | `ragent-tui` | Reuse model-picker dialog |
| `azure_foundry` provider | `ragent-llm` | Delegate actual LLM calls after selection |
| Config directory resolution | `ragent-config` | Locate `azureresources.json` |

---

## Glossary

| Term | Definition |
|---|---|
| **Azure Resource** | A pre-declared Azure AI Foundry model deployment described in `azureresources.json`. |
| **azureresources.json** | User-managed JSON file in the ragent config directory containing a catalogue of Azure deployments. |
| **Deployment ID** | The Azure-side identifier for a model deployment (e.g. `gpt-4o-myproj`). |
| **Endpoint** | The base URL of an Azure AI Foundry project (e.g. `https://my-project.eastus2.services.ai.azure.com`). |
