---
title: "GeminiProvider"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:32:39.529962142+00:00"
---

# GeminiProvider

**Type:** technology

### From: gemini

The GeminiProvider struct serves as the main integration component for Google's Gemini API within the Ragent Core framework. This struct implements the Provider trait, establishing a standardized interface that allows the application to instantiate and interact with Gemini language models through a unified abstraction layer. The provider encapsulates all Gemini-specific configuration, including the provider identifier "gemini" and the human-readable name "Google Gemini".

The provider maintains a comprehensive catalog of available Gemini models through the `gemini_default_models` function, which returns model metadata including pricing per million tokens, context window sizes, and capability flags. This model catalog spans multiple generations of Gemini models, from the experimental 2.5 series through the production-ready 2.0 and 1.5 variants. Each model specification includes granular cost structures for input and output tokens, enabling precise usage tracking and budget management. Capabilities are represented as boolean flags indicating support for reasoning, streaming responses, vision (image understanding), and tool use (function calling).

The `create_client` method instantiates a `GeminiClient` configured with the appropriate API credentials and endpoint URL. This factory pattern allows the provider to encapsulate client construction logic while returning a boxed trait object that conforms to the `LlmClient` interface. The implementation supports custom base URLs, enabling use cases such as regional API endpoints, proxy configurations, or testing against mock servers. The provider's design follows the dependency inversion principle, allowing higher-level application code to depend on abstract traits rather than concrete Gemini implementations.

## External Resources

- [Official Google Gemini API documentation](https://ai.google.dev/gemini-api/docs) - Official Google Gemini API documentation
- [Google AI Gemini model pricing information](https://ai.google.dev/pricing) - Google AI Gemini model pricing information

## Sources

- [gemini](../sources/gemini.md)
