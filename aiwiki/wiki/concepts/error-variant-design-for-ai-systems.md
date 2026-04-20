---
title: "Error Variant Design for AI Systems"
type: concept
generated: "2026-04-19T20:14:19.482610448+00:00"
---

# Error Variant Design for AI Systems

### From: error

Error variant design for AI systems requires modeling failure modes unique to probabilistic, non-deterministic operations while maintaining operational reliability. The `RagentError` variants demonstrate domain-specific considerations: `Provider` errors capture LLM communication failures (timeouts, rate limits, malformed responses, model unavailability) distinct from `Tool` errors representing deterministic function execution failures. This separation enables different recovery strategies—provider errors may trigger fallback models or exponential backoff, while tool errors indicate bugs or input validation failures requiring immediate attention.

The design anticipates operational complexity in production AI deployments. Multiple LLM providers (OpenAI, Anthropic, local models) each have distinct failure modes: authentication errors, context length exceeded, content policy violations, and infrastructure outages. The `Provider` variant's `provider: String` field enables routing decisions and provider-specific retry policies without expanding the enum for each service. Similarly, `Tool` errors abstract over arbitrary function execution—local shell commands, HTTP APIs, database queries—unifying their failure reporting while preserving the specific tool identifier for debugging. The `SessionNotFound` variant reflects stateful agent architectures where conversation context must be retrieved from persistent storage, with clear error semantics for expired or invalid session identifiers.

Security and safety considerations permeate the variant design. `PermissionDenied` with its `permission` and `pattern` fields supports principle-of-least-privilege enforcement for agent capabilities, enabling audit trails of attempted unauthorized operations. The granularity permits sophisticated access control: distinguishing read vs. write permissions, resource glob patterns, and user-level vs. system-level denials. This aligns with emerging AI safety frameworks where agent capabilities must be constrained and observable, with errors serving as the mechanism for enforcement and accountability.

## External Resources

- [OpenAI API error codes and handling patterns](https://platform.openai.com/docs/guides/error-codes) - OpenAI API error codes and handling patterns
- [Anthropic API error handling documentation](https://docs.anthropic.com/en/api/errors) - Anthropic API error handling documentation

## Sources

- [error](../sources/error.md)
