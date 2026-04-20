---
title: "CopilotProvider"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:28:18.510184962+00:00"
---

# CopilotProvider

**Type:** technology

### From: copilot

The `CopilotProvider` struct serves as the factory and configuration layer for GitHub Copilot integration within the ragent-core provider system. It implements the `Provider` trait, which is the primary extension point for adding new language model backends to the framework. The provider's role is to manage static configuration, advertise available models with their capabilities and costs, and instantiate configured `CopilotClient` instances on demand. This separation between provider (factory) and client (runtime) follows the established pattern in ragent-core for managing connection lifecycle and configuration.

A notable characteristic of the `CopilotProvider` is its handling of model metadata. Unlike providers that fetch model lists dynamically at runtime, Copilot maintains a hardcoded default model list reflecting the models known to be available through Copilot subscriptions as of implementation time. This list includes GPT-4o and GPT-4o-mini (128K context, vision and tool capable), Claude Sonnet 4 (200K context, reasoning capable), and o3-mini (200K context, configurable reasoning effort). Each model entry includes detailed capability flags, context window sizes, and zero-cost pricing—reflecting Copilot's subscription-based pricing model rather than per-token billing. The provider also implements `fetch_usage()`, which retrieves the detected subscription tier (Pro, Business, etc.) from cached session metadata.

The provider supports flexible endpoint configuration through `with_url()`, enabling use cases such as corporate proxies, regional endpoints, or testing against Copilot-compatible APIs. This configurability is essential for enterprise deployments where direct internet access may be restricted. When creating clients, the provider delegates to `resolve_copilot_auth()`, which orchestrates the complex authentication resolution including token exchange, caching, and fallback handling. The provider's design anticipates failure modes at each layer, with graceful degradation when Copilot's internal token exchange is unavailable by falling back to the GitHub Models inference API.

## Sources

- [copilot](../sources/copilot.md)
