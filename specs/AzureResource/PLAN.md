# Implementation Plan: Azure Resource Provider

**Spec ID:** `AzureResource`

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Create `AzureResourceProvider` struct and module scaffold | FR-001 | S | Critical | — |
| T-002 | Define `azureresources.json` schema types and parser | FR-003 | M | Critical | — |
| T-003 | Implement file discovery and loading in config directory | FR-002 | S | Critical | T-002 |
| T-004 | Implement `list_models()` returning parsed entries as `ModelInfo` | FR-003, FR-007 | M | Critical | T-003 |
| T-005 | Register `azure_resource` in `ProviderRegistry` | FR-001 | S | Critical | T-001, T-004 |
| T-006 | Add `SelectAzureResource` dialog step variant | FR-004 | M | High | — |
| T-007 | Render Azure Resource list in TUI layout | FR-004 | S | High | T-006 |
| T-008 | Handle key events for `SelectAzureResource` step | FR-004 | S | High | T-006 |
| T-009 | Persist and restore last-selected entry | FR-005 | S | High | T-004 |
| T-010 | Wire `/setup` and `/model` flows to show Azure Resource picker | FR-004 | S | High | T-007, T-008 |
| T-011 | Implement provider resolution to `azure_foundry` backend | FR-006 | M | Critical | T-005 |
| T-012 | Handle missing or invalid `azureresources.json` gracefully | FR-002, FR-008 | S | Medium | T-003 |
| T-013 | Write unit tests for JSON parsing and validation | FR-003, FR-008 | M | High | T-002 |
| T-014 | Write integration test for full setup flow | FR-004–FR-006 | L | Medium | T-010, T-011 |
| T-015 | Update user docs with `azureresources.json` format and examples | — | S | Low | T-002 |
| T-016 | Update CHANGELOG.md | — | S | Low | T-014 |

---

## Task Details

### T-001 — Create `AzureResourceProvider` struct and module scaffold

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs` (new)

Create a minimal provider struct:

```rust
pub struct AzureResourceProvider {
    /// Path to azureresources.json (resolved once at init).
    config_path: PathBuf,
    /// Parsed entries, refreshed on each list_models() call.
    entries: Vec<AzureResourceEntry>,
}
```

Implement the `Provider` trait (or the ragent-llm provider interface) with stub
methods.  Add `mod azure_resource;` to `crates/ragent-llm/src/providers/mod.rs`.

**Acceptance:**
- Module compiles.
- `ProviderRegistry::get("azure_resource")` returns the provider after registration.

---

### T-002 — Define `azureresources.json` schema types and parser

**Crate:** `ragent-llm` or `ragent-config`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Define Rust types matching the JSON schema:

```rust
#[derive(Debug, Clone, Deserialize)]
struct AzureResourceFile {
    version: String,
    resources: Vec<AzureResourceEntry>,
}

#[derive(Debug, Clone)]
struct AzureResourceEntry {
    id: String,
    name: String,
    endpoint: String,
    /// Direct API key token (optional, takes precedence over api_key_env).
    api_key: Option<String>,
    /// Name of the environment variable holding the API key.
    /// Mandatory when api_key is absent.
    api_key_env: Option<String>,
    context_window: Option<usize>,
    capabilities: Option<Vec<String>>,
    thinking: Option<ThinkingConfig>,
}
```

Implement `parse_azure_resources(path: &Path) -> Result<Vec<AzureResourceEntry>>` with:
- Version validation (`"1"`).
- Per-entry validation (mandatory fields `id`, `name`, `endpoint` present).
- At least one of `api_key` or `api_key_env` must be present; if both are absent,
  skip the entry with a warning.
- Duplicate-ID detection (warn and skip duplicates).

**Acceptance:**
- Valid file parses to expected `Vec`.
- Missing mandatory fields produce warnings and skip the entry.
- Entries without `api_key` or `api_key_env` are skipped with a warning.
- Wrong version logs an error and returns an empty Vec.

---

### T-003 — Implement file discovery and loading in config directory

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Add a constructor that resolves the config directory:

```rust
impl AzureResourceProvider {
    pub fn new() -> Self {
        let config_path = resolve_config_dir().join("azureresources.json");
        Self { config_path, entries: Vec::new() }
    }
}
```

Reuse `ragent-config`'s config-directory resolution logic (or duplicate it if
necessary to avoid circular dependencies).

**Acceptance:**
- `new()` finds the file when it exists in `~/.config/ragent/`.
- `new()` does not panic when the file is missing.

---

### T-004 — Implement `list_models()` returning parsed entries as `ModelInfo`

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Implement `Provider::list_models()` (or equivalent):

1. Read `azureresources.json` from `config_path`.
2. Parse with the logic from T-002.
3. Convert each valid `AzureResourceEntry` to `ModelInfo`:
   - `id` → entry's `id`
   - `name` → entry's `name`
   - `provider_id` → `"azure_foundry"` (the underlying backend)
   - `context_window` → entry's value or default `128_000`
   - `capabilities` → entry's value or empty
   - `thinking_config` → entry's `thinking`
4. Cache the parsed entries internally.

**Acceptance:**
- Returns a `Vec<ModelInfo>` matching the file contents.
- Safe defaults applied when optional fields are absent.

---

### T-005 — Register `azure_resource` in `ProviderRegistry`

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/mod.rs`

In `create_default_registry()` (or equivalent), add:

```rust
registry.register(Box::new(AzureResourceProvider::new()));
```

**Acceptance:**
- `registry.list()` includes an entry with id `"azure_resource"` and name
  `"Azure Resource (File)"`.

---

### T-006 — Add `SelectAzureResource` dialog step variant

**Crate:** `ragent-tui`  
**File:** `crates/ragent-tui/src/app/state.rs`

Add a new variant to `ProviderSetupStep`:

```rust
/// Choosing an Azure deployment from the user's azureresources.json file.
SelectAzureResource {
    /// Parsed entries from the file.
    entries: Vec<AzureResourceEntry>,
    /// Index of the highlighted entry.
    selected: usize,
    /// Optional error message (e.g. file not found).
    error: Option<String>,
},
```

**Acceptance:**
- Variant compiles and is added to all relevant `match` arms.

---

### T-007 — Render Azure Resource list in TUI layout

**Crate:** `ragent-tui`  
**File:** `crates/ragent-tui/src/layout.rs`

In `render_provider_setup_dialog()`, add an arm for `ProviderSetupStep::SelectAzureResource`:

- Title: "Select Azure Resource"
- If `entries` is empty, show the error message or "No resources found".
- Otherwise render a scrollable list with entry names.
- Highlight the `selected` index.
- Footer hint: "↑/↓ to move, Enter to select, Esc to cancel".

Reuse the existing `SelectModel` rendering style for consistency.

**Acceptance:**
- Dialog renders correctly with and without entries.
- Visual style matches other provider-setup dialogs.

---

### T-008 — Handle key events for `SelectAzureResource` step

**Crate:** `ragent-tui`  
**File:** `crates/ragent-tui/src/input.rs`

In `handle_provider_setup_key()`, add an arm for `SelectAzureResource`:

- `KeyCode::Up` / `KeyCode::Down` — move selection, wrapping if desired.
- `KeyCode::Enter` — confirm selection:
  1. Persist the selection (T-009).
  2. Transition to `ProviderSetupStep::Done` with the chosen entry's name.
  3. Set the active provider to `azure_foundry` with the entry's endpoint and model id.
- `KeyCode::Esc` — cancel and close the dialog.

**Acceptance:**
- Key navigation works.
- Enter confirms and triggers persistence.
- Esc aborts without side effects.

---

### T-009 — Persist and restore last-selected entry

**Crate:** `ragent-tui`  
**File:** `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/input.rs`

**Persist:**
When an entry is confirmed, write to storage:

```rust
let payload = serde_json::json!({
    "id": entry.id,
    "endpoint": entry.endpoint,
    "api_key": entry.api_key,
    "api_key_env": entry.api_key_env,
});
storage.set_setting("azure_resource_last_selection", &payload.to_string())?;
```

**Restore:**
When the user starts the Azure Resource setup flow, read the key.  If the stored
`id` exists in the current file, pre-select it; otherwise delete the stale key and
start at index 0.

**Acceptance:**
- Selection survives restart.
- Stale selections are cleaned up automatically.

---

### T-010 — Wire `/setup` and `/model` flows to show Azure Resource picker

**Crate:** `ragent-tui`  
**File:** `crates/ragent-tui/src/app.rs`

When the user chooses "azure_resource" from `SelectProvider`, transition to
`ProviderSetupStep::SelectAzureResource` instead of `SelectModel`.

When the user invokes `/model` and `azure_resource` is already configured,
show the same picker (or skip if only one resource exists).

**Acceptance:**
- `/setup` → select "Azure Resource (File)" → shows resource list.
- `/model` with `azure_resource` configured → shows resource list (or auto-selects
  if only one entry and a prior selection exists).

---

### T-011 — Implement provider resolution to `azure_foundry` backend

**Crate:** `ragent-llm` / `ragent-agent`  
**Files:** `crates/ragent-llm/src/providers/azure_resource.rs`,
`crates/ragent-agent/src/session/processor.rs`

The `AzureResourceProvider` itself does **not** make HTTP calls.  Instead, after
selection, the session layer must:

1. Instantiate an `azure_foundry` provider.
2. Configure it with the selected entry's `endpoint`.
3. Use the entry's `id` as the model ID.
4. Resolve the API key:
   - If `api_key` is present in the entry, use it directly.
   - Otherwise, read from the environment variable named by `api_key_env`.
   - If neither is available, return a clear error.

This may require a small bridge in the processor or a factory method on
`AzureResourceProvider` that returns a boxed `azure_foundry` client configured for
the selected entry.

**Acceptance:**
- Chatting with a selected Azure Resource sends requests to the correct endpoint
  with the correct deployment ID.
- Missing env var produces a clear error: "Environment variable <name> is not set".

---

### T-012 — Handle missing or invalid `azureresources.json` gracefully

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Ensure all error paths return an empty list rather than failing:

- File not found → empty list, debug log.
- Permission denied → empty list, warning log.
- Malformed JSON → empty list, warning log with path.
- Invalid entries → skipped individually, warning per entry.

**Acceptance:**
- No panic or error dialog when the file is missing or broken.
- Warnings contain the file path for easy debugging.

---

### T-013 — Write unit tests for JSON parsing and validation

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/tests/test_azure_resource_parser.rs` (new)

Tests to cover:

1. **test_parse_valid_file** — happy path with two entries.
2. **test_parse_missing_mandatory_field** — skips entry, keeps others.
3. **test_parse_wrong_version** — returns empty list.
4. **test_parse_duplicate_ids** — keeps first, skips duplicate.
5. **test_parse_missing_file** — returns empty list, no panic.
6. **test_parse_malformed_json** — returns empty list, logs warning.
7. **test_optional_fields_defaults** — confirms safe defaults.

**Acceptance:**
- All tests pass (`cargo test -p ragent-llm`).

---

### T-014 — Write integration test for full setup flow

**Crate:** `ragent-tui` (or workspace integration tests)  
**File:** `tests/test_azure_resource_flow.rs` (new)

End-to-end test:

1. Create a temp config dir with a valid `azureresources.json`.
2. Launch the app with that config dir.
3. Trigger provider setup for `azure_resource`.
4. Simulate selecting the second entry.
5. Assert that the active provider becomes `azure_foundry` with the correct
   endpoint and model ID.
6. Assert that the persistence key is written.

**Acceptance:**
- Test passes in CI.

---

### T-015 — Update user docs with `azureresources.json` format and examples

**File:** `docs/userdocs/azure-resource.md` (new)

Document:
- Where to place the file.
- Full JSON schema with an example.
- How the `api_key_env` field works.
- How capabilities and thinking overrides work.

**Acceptance:**
- Markdown file is clear and contains a copy-pasteable example.

---

### T-016 — Update CHANGELOG.md

Add an entry under "Unreleased" describing the new Azure Resource provider and
`azureresources.json` support.

**Acceptance:**
- CHANGELOG entry follows Keep a Changelog format.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Circular dependency if `ragent-llm` tries to import `ragent-config` for directory resolution | Low | High | Resolve config dir in `ragent-config` and pass the path into `AzureResourceProvider::new(path)`, or duplicate the tiny resolution logic. |
| User confusion between `azure_foundry` and `azure_resource` | Medium | Medium | Clear naming (`Azure Resource (File)` vs `Azure AI Foundry`) and documentation. |
| `azure_foundry` backend changes breaking the bridge | Low | High | Keep the bridge thin; most logic lives in the existing `azure_foundry` provider. |

---

## Estimated Total Effort

- **T-001–T-005** (backend): ~2 days
- **T-006–T-010** (TUI integration): ~2 days
- **T-011** (resolution bridge): ~1 day
- **T-012–T-014** (tests & edge cases): ~1.5 days
- **T-015–T-016** (docs & changelog): ~0.5 days

**Total: ~7 days** (medium feature)
