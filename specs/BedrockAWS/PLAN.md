# BedrockAWS — Implementation Plan

## Architecture

The Bedrock provider follows the same patterns as existing providers
(especially `azure_foundry` and `azure_resource`):

1. **New module**: `crates/ragent-llm/src/providers/bedrock.rs`
2. **Provider struct**: `BedrockProvider` implementing the `Provider` trait
3. **Client structs**:
   - `BedrockAnthropicClient` — wraps the existing `AnthropicClient` with
     SigV4 auth headers and the Bedrock Messages API path
   - `BedrockConverseClient` — implements `LlmClient` for non-Anthropic
     models using the Bedrock Converse API
4. **SigV4 signing**: A self-contained `sigv4` module implementing AWS
   Signature Version 4 without the full AWS SDK (~200 lines)
5. **Credential resolution**: `AwsCredentials` struct resolving from env
   vars, profile file, and instance metadata
6. **Registry wiring**: Register `BedrockProvider` in
   `create_default_registry()`
7. **Tests**: Unit tests for SigV4, credential resolution, and request
   format translation; integration test contract matching other providers

### Dependency Decision

Rather than adding the ~200MB `aws-sdk-bedrock` crate, we implement a
minimal SigV4 signer (~200 lines) and call the Bedrock HTTP API directly.
This matches the approach used by the Anthropic, OpenAI, and Gemini
providers, which all use raw HTTP clients rather than vendor SDKs.

### File Layout

```
crates/ragent-llm/src/providers/
├── bedrock.rs              # BedrockProvider + BedrockAnthropicClient + BedrockConverseClient
├── bedrock_sigv4.rs        # SigV4 signing (standalone, no AWS SDK dep)
├── bedrock_credentials.rs  # AWS credential resolution chain
└── mod.rs                  # +1 line: pub mod bedrock
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Implement AWS credential resolution | FR-001, FR-002, FR-003 | L | Critical | — |
| T-002 | Implement AWS SigV4 request signing | FR-014, FR-015, FR-016 | L | Critical | — |
| T-003 | Implement BedrockProvider struct | FR-008, FR-009, FR-010 | M | Critical | T-001, T-002 |
| T-004 | Implement BedrockAnthropicClient (Messages API) | FR-011, FR-017, FR-018, FR-019, FR-020, FR-025 | L | Critical | T-001, T-002 |
| T-005 | Implement BedrockConverseClient (Converse API) | FR-012, FR-017, FR-018, FR-019, FR-021, FR-024, FR-026 | L | High | T-001, T-002 |
| T-006 | Register BedrockProvider in default registry | FR-009 | S | Critical | T-003 |
| T-007 | Implement region and endpoint configuration | FR-004, FR-005, FR-006, FR-007 | M | High | T-003 |
| T-008 | Implement @bedrock model suffix stripping | FR-013 | S | Medium | T-003 |
| T-009 | Implement ListFoundationModels discovery | FR-022, FR-023 | M | Medium | T-001, T-002 |
| T-010 | Implement error handling for Bedrock API errors | FR-027, FR-028, FR-029, FR-030 | M | High | T-004, T-005 |
| T-011 | Implement vision (image) support | FR-024 | S | Medium | T-004, T-005 |
| T-012 | Write unit tests for SigV4 signing | NFR-001, FR-014, FR-015 | M | Critical | T-002 |
| T-013 | Write unit tests for credential resolution | FR-001, FR-002, FR-003 | M | Critical | T-001 |
| T-014 | Write unit tests for request format translation | FR-011, FR-012, FR-020, FR-021 | M | High | T-004, T-005 |
| T-015 | Write integration test (provider contract) | NFR-003 | M | High | T-006 |
| T-016 | Add TUI model selector integration | FR-031, FR-032 | S | Low | T-006, T-009 |
| T-017 | Update PROVIDERS.md and SPEC.md documentation | — | S | Low | T-006 |

## Task Details

### T-001 — AWS Credential Resolution (L, Critical)

Implement `AwsCredentials` struct and `resolve_aws_credentials()` function in
`bedrock_credentials.rs`:

- Check `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` env vars
- If `AWS_PROFILE` env var or config `profile` option is set, read
  `~/.aws/credentials` INI file for the named profile
- Optionally, check IAM instance metadata endpoint (EC2/ECS)
- Include `AWS_SESSION_TOKEN` support (FR-003)
- Return structured `AwsCredentials { access_key, secret_key, session_token, region }`
- Error with actionable message when no credentials found (FR-002)

### T-002 — AWS SigV4 Request Signing (L, Critical)

Implement `sign_request()` in `bedrock_sigv4.rs`:

- AWS SigV4 canonical request construction
- String-to-sign with SHA256 HMAC
- Signing key derivation (date-based key chain)
- Add `Authorization`, `x-amz-date`, `x-amz-content-sha256` headers
- Add `x-amz-security-token` when session token present (FR-015)
- No `Authorization: Bearer` or `x-api-key` headers (FR-016)
- Pure Rust implementation — no external AWS SDK dependency (NFR-001)

### T-003 — BedrockProvider Struct (M, Critical)

Implement `BedrockProvider` in `bedrock.rs`:

- `id()` → `"bedrock"`, `name()` → `"Amazon Bedrock"` (FR-008)
- `default_models()` → Static catalog of 8+ models (FR-010)
- `create_client()` → Resolve credentials, select Anthropic or Converse
  client based on model ID, return boxed `LlmClient`
- `fetch_usage()` → `None` (Bedrock doesn't expose usage via API)

### T-004 — BedrockAnthropicClient (L, Critical)

Implement `BedrockAnthropicClient` wrapping existing `AnthropicClient` logic:

- Route to `/model/{model_id}/invoke-with-response-stream` (FR-011)
- Use SigV4-signed requests instead of `x-api-key` header
- Parse Anthropic Messages streaming events (reuse existing parser)
- Support tool_use content blocks (FR-020)
- Support thinking/reasoning parameters for Claude models (FR-025)

### T-005 — BedrockConverseClient (L, High)

Implement `BedrockConverseClient` for non-Anthropic models:

- Route to `/model/{model_id}/converse-stream` (FR-012)
- Translate `ChatRequest` → Bedrock Converse request format
- Translate Converse `toolSpec` ↔ ragent `ToolDefinition` (FR-021)
- Parse Converse streaming events → `StreamEvent` items
- Support image content (FR-024)

### T-006 — Registry Registration (S, Critical)

Add `pub mod bedrock` to `mod.rs` and register in
`create_default_registry()`:

```rust
registry.register(Box::new(bedrock::BedrockProvider::new()));
```

### T-007 — Region and Endpoint Configuration (M, High)

Implement configuration resolution with precedence:

1. `AWS_BEDROCK_REGION` env var (FR-006)
2. `AWS_REGION` env var (FR-005)
3. `provider.bedrock.options.region` from config (FR-004)
4. Default `us-east-1`

Custom endpoint URL from `provider.bedrock.options.endpoint_url` (FR-007).

### T-008 — Model Suffix Stripping (S, Medium)

Strip `@bedrock` suffix from model IDs before constructing API paths (FR-013).

### T-009 — Model Discovery (M, Medium)

Implement `discover_models()` calling Bedrock `ListFoundationModels`:

- GET `https://bedrock.{region}.amazonaws.com/foundation-models`
- SigV4-signed request
- Parse response into `Vec<ModelInfo>`
- Fallback to static catalog on error (FR-023)

### T-010 — Error Handling (M, High)

Map Bedrock API error types to ragent errors:

- `ThrottlingException` → retryable error (FR-027)
- `ValidationException` → descriptive error (FR-028)
- `AccessDeniedException` → permissions error (FR-029)
- Never log raw AWS keys (FR-030)

### T-011 — Vision Support (S, Medium)

Encode image content parts as base64 in request payload:

- Anthropic: `image` block with `source.bytes` (FR-024)
- Converse: `ImageBlock` with `bytes` field

### T-012 — SigV4 Unit Tests (M, Critical)

Test cases:

- Known-input signing test (AWS documentation examples)
- Session token header inclusion
- Region and service name correctness
- Date header format validation

### T-013 — Credential Resolution Unit Tests (M, Critical)

Test cases:

- Env var credential resolution
- Profile-based credential resolution (temp INI file)
- Missing credentials error message
- Session token inclusion

### T-014 — Request Format Translation Tests (M, High)

Test cases:

- ChatRequest → Anthropic Messages format
- ChatRequest → Converse format
- ToolDefinition ↔ toolSpec roundtrip
- Image content encoding

### T-015 — Integration Test (M, High)

Provider contract test matching other providers' patterns:

- Provider ID and name correct
- Default models non-empty
- Model resolution works
- Error on missing credentials

### T-016 — TUI Integration (S, Low)

- Bedrock models appear in `/models` (FR-031)
- `[bedrock]` badge in model selector (FR-032)

### T-017 — Documentation (S, Low)

- Update PROVIDERS.md to mark Bedrock as supported
- Update SPEC.md provider section
- Update README.md provider list
- Add Bedrock section to QUICKSTART.md

## Estimated Effort

| Category | Effort |
|---|---|
| Core implementation (T-001 to T-011) | 7 tasks, ~5–7 days |
| Testing (T-012 to T-015) | 4 tasks, ~2–3 days |
| Integration & docs (T-016, T-017) | 2 tasks, ~0.5 days |
| **Total** | **~8–11 days** |

## Risks

| Risk | Mitigation |
|---|---|
| SigV4 signing edge cases (chunked encoding, S3-style) | Use standard JSON payloads only; test against AWS published test vectors |
| Bedrock Converse API differences across model families | Start with well-documented models (Nova, Llama, Mistral); add per-model quirks as discovered |
| IAM role metadata endpoint not accessible in dev | Make instance metadata optional; env vars and profile are primary |
| AWS SDK temptation | Strict NFR-001: implement SigV4 from scratch; no `aws-sdk-*` crates |