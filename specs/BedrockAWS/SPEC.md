---
status: implemented
---

# BedrockAWS — Amazon Bedrock Provider

## Overview

This specification defines a new LLM provider for ragent that connects to
models hosted on **Amazon Bedrock**. The provider implements the existing
`Provider` trait and `LlmClient` interface, enabling Bedrock-hosted models
(Claude, Titan, Llama, Mistral, etc.) to be used alongside the existing ten
providers.

Amazon Bedrock differs from existing ragent providers in two critical ways:

1. **Authentication** — Bedrock uses AWS Signature Version 4 (SigV4) rather
   than a static API key. Credentials are resolved from the standard AWS
   chain (environment variables, shared credentials file, IAM role).
2. **API protocol** — Bedrock exposes a `Converse` API for cross-model
   chat completions and a Messages-compatible endpoint for Anthropic models.
   The provider must route requests to the correct API based on the model.

## Requirements

### Authentication & Credential Resolution

**FR-001** (Ubiquitous) The system shall resolve AWS credentials for the
Bedrock provider using the standard AWS credential provider chain:
`AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` environment variables,
`AWS_PROFILE` named profile from `~/.aws/credentials`, IAM instance/profile
role metadata, in that precedence order.

**FR-002** (State-driven) While no valid AWS credentials are available from
any source in the credential chain, the system shall reject Bedrock provider
initialisation with a clear error message indicating which credential sources
were attempted.

**FR-003** (Optional) Where the `AWS_SESSION_TOKEN` environment variable or
`session_token` field in the AWS profile is present, the system shall include
the session token in the SigV4 signing payload for temporary credential
support (e.g. STS assumed roles).

### Configuration

**FR-004** (Ubiquitous) The system shall accept the following configuration
via `ragent.json` under `provider.bedrock`:

| Field | Type | Default | Description |
|---|---|---|---|
| `region` | string | `us-east-1` | AWS region for Bedrock endpoints |
| `profile` | string | — | Named AWS profile from `~/.aws/credentials` |
| `endpoint_url` | string | — | Custom endpoint URL (VPC endpoint override) |

**FR-005** (Event-driven) When the `AWS_REGION` environment variable is set,
the system shall use its value as the Bedrock region, overriding the
`ragent.json` `region` field.

**FR-006** (Event-driven) When the `AWS_BEDROCK_REGION` environment variable
is set, the system shall use its value as the Bedrock region, overriding both
`AWS_REGION` and the `ragent.json` `region` field. This provides a
Bedrock-specific override for multi-service AWS setups.

**FR-007** (Optional) Where `provider.bedrock.endpoint_url` is configured in
`ragent.json`, the system shall use the specified URL as the Bedrock API base,
enabling VPC interface endpoints and AWS PrivateLink.

### Provider Trait Implementation

**FR-008** (Ubiquitous) The system shall implement the `Provider` trait for
the Bedrock provider with `id()` returning `"bedrock"` and `name()` returning
`"Amazon Bedrock"`.

**FR-009** (Ubiquitous) The system shall register the Bedrock provider in
`create_default_registry()` so it appears in the provider list alongside
existing providers without requiring explicit configuration.

**FR-010** (Ubiquitous) The system shall provide a default model catalog
including at minimum: Claude Sonnet 4, Claude Haiku 3.5, Claude Opus 4,
Amazon Nova Pro, Amazon Nova Lite, Amazon Nova Micro, Llama 4 Maverick,
and Mistral Large.

### API Routing

**FR-011** (State-driven) While the requested model is an Anthropic Claude
model (model ID starting with `anthropic.claude`), the system shall route
the request to the Bedrock Messages API endpoint
(`/model/{model_id}/invoke-with-response-stream`) using the Anthropic
Messages request format.

**FR-012** (State-driven) While the requested model is a non-Anthropic
model (Titan, Llama, Mistral, Cohere, Amazon Nova, etc.), the system shall
route the request to the Bedrock Converse API endpoint
(`/model/{model_id}/converse-stream`) using the Bedrock Converse request
format.

**FR-013** (Event-driven) When the model ID contains `@bedrock` suffix
(e.g. `claude-sonnet-4-20250514@bedrock`), the system shall strip the suffix
before constructing the Bedrock API path and resolve the model through the
bedrock provider.

### Request Signing

**FR-014** (Ubiquitous) The system shall sign every HTTP request to the
Bedrock API using AWS Signature Version 4, including the `x-amz-date`
header, `Authorization` header with SigV4 signature, and `x-amz-content-sha256`
header.

**FR-015** (Event-driven) When AWS credentials include a session token, the
system shall include the `x-amz-security-token` header in signed requests.

**FR-016** (Unwanted) The system shall not send an `Authorization: Bearer`
or `x-api-key` header to Bedrock endpoints, as these are not accepted by the
Bedrock API.

### Streaming

**FR-017** (Ubiquitous) The system shall support streaming responses via the
Bedrock `invoke-with-response-stream` (Anthropic) and `converse-stream`
(non-Anthropic) endpoints, yielding `StreamEvent` items to the ragent session
processor.

**FR-018** (Event-driven) When the Bedrock API returns an event type of
`chunk`, the system shall emit a `StreamEvent::Content` with the decoded text
or tool-call delta.

**FR-019** (Event-driven) When the Bedrock API returns a `message_stop` or
`stop` event, the system shall emit a `StreamEvent::Finish` with the
appropriate `FinishReason`.

### Tool Use

**FR-020** (State-driven) While a model supports tool use (as indicated in
its `Capabilities.tool_use` flag), the system shall include tool definitions
in the request payload and decode `tool_use` content blocks from the
response.

**FR-021** (State-driven) While a model is invoked via the Converse API
(non-Anthropic), the system shall translate ragent `ToolDefinition` structs
into the Bedrock Converse `toolSpec` format and translate Converse
`toolUse` response blocks back into ragent `ContentPart::ToolCall` items.

### Model Discovery

**FR-022** (Optional) Where the `discover_models()` method is called on the
Bedrock provider, the system shall query the Bedrock
`ListFoundationModels` API and return a `Vec<ModelInfo>` containing all
models available in the configured region.

**FR-023** (Event-driven) When the `ListFoundationModels` API returns an
error (e.g. insufficient IAM permissions), the system shall fall back to
the static default model catalog and log a warning.

### Vision

**FR-024** (State-driven) While a model's `Capabilities.vision` flag is
`true`, the system shall encode image content parts as base64 inline data
in the Bedrock request payload, following the provider-specific format
(Anthropic `image` block or Converse `image` block).

### Thinking / Reasoning

**FR-025** (Optional) Where a model supports thinking/reasoning (Claude
Opus 4, Claude Sonnet 4), the system shall include the `thinking` parameter
in the Anthropic Messages request payload when the user or model default
requests reasoning, using the same `ThinkingConfig` infrastructure as the
direct Anthropic provider.

**FR-026** (Unwanted) The system shall not send thinking/reasoning
parameters to models that do not support them (e.g. Titan, Llama, Mistral).

### Error Handling

**FR-027** (Event-driven) When the Bedrock API returns a `ThrottlingException`,
the system shall return a retryable error to the session processor so the
existing retry mechanism can apply backoff.

**FR-028** (Event-driven) When the Bedrock API returns a `ValidationException`,
the system shall return an error with the API message text so the user can
correct the request (e.g. invalid model ID, unsupported feature).

**FR-029** (Event-driven) When the Bedrock API returns a
`AccessDeniedException`, the system shall return an error indicating that
the AWS principal does not have `bedrock:InvokeModel` permission for the
requested model.

**FR-030** (Unwanted) The system shall not expose raw AWS access keys or
secret keys in error messages or log output.

### TUI Integration

**FR-031** (Ubiquitous) The system shall appear in the `/models` TUI command
when the Bedrock provider is registered, listing all default models.

**FR-032** (Optional) Where dynamic model discovery succeeds, the system
shall display discovered Bedrock models in the TUI model selector,
distinguishing them with a `[bedrock]` badge or similar indicator.

### Configuration Example

The user configures Bedrock in `ragent.json`:

```jsonc
{
  "provider": {
    "bedrock": {
      "env": ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
      "options": {
        "region": "us-east-1",
        "profile": "my-dev-profile"
      }
    }
  }
}
```

Or uses environment variables exclusively:

```bash
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export AWS_REGION="eu-west-1"
ragent --model bedrock/claude-sonnet-4-20250514
```

## Non-Functional Requirements

**NFR-001** The Bedrock provider shall not introduce any additional runtime
dependencies beyond what is already in the Cargo workspace. AWS SigV4 signing
shall be implemented without pulling in the full AWS SDK.

**NFR-002** The Bedrock provider shall add no more than 500ms of latency to
the first token time compared to calling the Anthropic API directly for the
same model.

**NFR-003** The Bedrock provider shall pass the same integration test
contract as all other providers: valid `ChatRequest` in → valid
`StreamEvent` stream out.

## Out of Scope

- Cross-region inference (Bedrock's `InvokeModelWithResponseStream` across
  regions) — deferred to a future iteration.
- Bedrock Agent orchestration — this spec covers model invocation only.
- Bedrock Knowledge Bases — out of scope for the provider interface.
- Guardrails configuration — out of scope for the initial implementation.
- Provisioned Throughput — out of scope; on-demand invocation only.