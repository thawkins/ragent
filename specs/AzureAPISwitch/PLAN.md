# Implementation Plan: Azure Resource Provider API Type Switch

**Spec ID:** `AzureAPISwitch`

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Add `api_type` field to `AzureResourceEntry` | FR-001 | S | Critical | — |
| T-002 | Validate `api_type` during JSON parsing | FR-001, FR-006 | S | Critical | T-001 |
| T-003 | Carry `api_type` through `entry_to_model_info` | FR-005 | S | High | T-001 |
| T-004 | Create `AzureAnthropicClient` wrapper | FR-003, FR-004 | M | Critical | — |
| T-005 | Branch `create_client` on `api_type` | FR-002, FR-003 | M | Critical | T-003, T-004 |
| T-006 | Update `azureresources.json` schema docs | FR-001 | S | Low | T-001 |
| T-007 | Write unit tests for parser validation | FR-001, FR-006, FR-007 | M | High | T-002 |
| T-008 | Write unit tests for `create_client` branching | FR-002, FR-003, FR-007 | M | High | T-005 |
| T-009 | Write integration test for Anthropic branch headers | FR-004, FR-007 | M | High | T-004, T-005 |
| T-010 | Update `CHANGELOG.md` and cross-references | — | S | Low | T-005, T-006 |

---

## Task Details

### T-001 — Add `api_type` field to `AzureResourceEntry`

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Add to `AzureResourceEntry`:

```rust
#[serde(default, rename = "api_type")]
pub api_type: Option<String>,
```

**Acceptance:**
- Module compiles.
- Existing JSON without the field parses successfully (`None`).

---

### T-002 — Validate `api_type` during JSON parsing

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

In `parse_azure_resources`, after the existing mandatory-field checks, add:

```rust
let api_type = entry.api_type.as_deref().unwrap_or("openai");
if api_type != "openai" && api_type != "anthropic" {
    tracing::warn!(
        resource_id = %entry.id,
        api_type = %api_type,
        "Skipping Azure resource entry: unsupported api_type"
    );
    continue;
}
```

**Acceptance:**
- `"openai"` and `"anthropic"` are accepted.
- `"gemini"` (or any other value) causes a warning and the entry is skipped.
- Missing field is treated as `"openai"`.

---

### T-003 — Carry `api_type` through `entry_to_model_info`

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Option A — extend `ModelInfo` with a new `api_type` field (requires updating the
struct in `mod.rs`).

Option B — store the mapping in a side table (e.g. `HashMap<model_id, api_type>`)
inside `AzureResourceProvider` and look it up in `create_client`.

**Acceptance:**
- `create_client` can reliably determine the API type for any model it serves.

---

### T-004 — Create `AzureAnthropicClient` wrapper

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs` (or new sub-module)

Implement a thin wrapper around `AnthropicClient` that overrides the auth header
from `x-api-key` to `api-key`:

```rust
struct AzureAnthropicClient {
    inner: AnthropicClient,
    api_key: String,
}

#[async_trait::async_trait]
impl LlmClient for AzureAnthropicClient {
    async fn chat(&self, request: ChatRequest) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        // Build the request body using AnthropicClient::build_request_body
        // Send to {base_url}/anthropic/v1/messages with header "api-key: {self.api_key}"
        // Parse SSE stream using AnthropicClient's parser
    }
}
```

**Acceptance:**
- The wrapper constructs the correct Anthropic request body.
- The `api-key` header is present and `x-api-key` / `Authorization` are absent.
- SSE parsing reuses the existing Anthropic stream parser.

---

### T-005 — Branch `create_client` on `api_type`

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/src/providers/azure_resource.rs`

Modify `Provider::create_client`:

```rust
async fn create_client(
    &self,
    api_key: &str,
    base_url: Option<&str>,
    options: &HashMap<String, Value>,
) -> Result<Box<dyn LlmClient>> {
    let resolved = base_url.unwrap_or_default().trim_end_matches('/').to_string();
    let api_type = options.get("api_type")
        .and_then(Value::as_str)
        .unwrap_or("openai");

    match api_type {
        "anthropic" => {
            let client = AnthropicClient::new(api_key, &resolved);
            tracing::info!(chat_endpoint = %format!("{}/anthropic/v1/messages", resolved), "Azure Resource (Anthropic) connected");
            Ok(Box::new(AzureAnthropicClient { inner: client, api_key: api_key.to_string() }))
        }
        _ => {
            let client = AzureFoundryClient::new(api_key, &resolved);
            tracing::info!(chat_endpoint = %format!("{}/openai/v1/chat/completions", resolved), "Azure Resource (OpenAI) connected");
            Ok(Box::new(client))
        }
    }
}
```

**Acceptance:**
- `"anthropic"` instantiates the Anthropic branch.
- `"openai"` or missing `api_type` instantiates the Azure Foundry branch.
- Both branches log the correct endpoint path.

---

### T-006 — Update `azureresources.json` schema docs

**File:** `specs/AzureResource/FILEFORMAT.md`

Add `api_type` to the fields table with description and allowed values.

**Acceptance:**
- Docs mention `"openai"` (default) and `"anthropic"`.

---

### T-007 — Write unit tests for parser validation

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/tests/test_azure_resource_parser.rs` (extend existing)

Add tests:
- `test_api_type_openai_accepted`
- `test_api_type_anthropic_accepted`
- `test_api_type_missing_defaults_to_openai`
- `test_api_type_invalid_skipped_with_warning`

**Acceptance:**
- All 4 tests pass.

---

### T-008 — Write unit tests for `create_client` branching

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/tests/test_azure_resource_parser.rs` (new tests)

Add tests:
- `test_create_client_openai_branch`
- `test_create_client_anthropic_branch`

Use a mock or type assertion (`downcast_ref`) to verify the returned client type.

**Acceptance:**
- Both branches are exercised and the correct client type is returned.

---

### T-009 — Write integration test for Anthropic branch headers

**Crate:** `ragent-llm`  
**File:** `crates/ragent-llm/tests/test_azure_resource_anthropic.rs` (new)

Spin up a mock HTTP server (e.g. `wiremock` or `httptest`), point an
`AzureResourceProvider` entry at it with `api_type: "anthropic"`, call
`create_client().chat(...)`, and assert:
- Request URL ends with `/anthropic/v1/messages`.
- Request contains header `api-key: test-key`.
- Request does NOT contain header `x-api-key`.

**Acceptance:**
- Mock server receives exactly one request matching the assertions above.

---

### T-010 — Update `CHANGELOG.md` and cross-references

**File:** `CHANGELOG.md`

Add entry:
- Azure Resource Provider API type switch — `api_type` field in `azureresources.json`
  supports `"anthropic"` endpoints, routing to Anthropic Messages API with Azure
  `api-key` auth.

**Acceptance:**
- CHANGELOG updated; SPEC.md cross-reference added if applicable.

---

## Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| Breaking existing files that happen to contain a field named `api_type` with an invalid value | High | Low | Treat invalid values as warnings + skip, not fatal errors. |
| `ModelInfo` schema change propagates to serialisation (e.g. session storage) | Medium | Medium | Use `Option<String>` with `skip_serializing_if` to avoid altering stored JSON. |
| Anthropic client internals change, breaking the wrapper | Low | Low | Wrapper delegates to public `AnthropicClient` methods; upstream changes are rare. |

## Estimated Total Effort

| Effort | Tasks | Total |
|---|---|---|
| Small | T-001, T-002, T-003, T-006, T-010 | 5 × S ≈ 1 day |
| Medium | T-004, T-005, T-007, T-008, T-009 | 5 × M ≈ 2.5 days |

**Estimated calendar time:** 3–4 days (single developer).
