---
title: "GenericOpenAiProvider"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:26:42.455961824+00:00"
---

# GenericOpenAiProvider

**Type:** technology

### From: generic_openai

The `GenericOpenAiProvider` is a Rust struct that implements the `Provider` trait to enable connections to arbitrary OpenAI-compatible API endpoints. Unlike the standard OpenAI provider that connects exclusively to OpenAI's official API, this implementation adds flexibility through configurable base URLs while maintaining full protocol compatibility. The struct itself is a zero-sized type (unit struct) containing no fields, with all configuration handled through method parameters, environment variables, and options maps at client creation time.

The provider's architecture separates concerns effectively: the `GenericOpenAiProvider` handles endpoint resolution and client instantiation, while delegating actual API communication to the existing `OpenAiClient`. This composition pattern allows code reuse without duplication of HTTP handling, authentication, and response parsing logic. The implementation supports multiple endpoint resolution strategies with clear priority ordering, making it suitable for diverse deployment scenarios from local development to production environments with custom infrastructure.

The provider exposes two configuration constants: `ENDPOINT_OPTION_KEY` ("endpoint_url") for explicit programmatic configuration, and `DEFAULT_ENV_ENDPOINT_KEY` ("GENERIC_OPENAI_API_BASE") for environment-based configuration. This dual approach follows twelve-factor app principles, allowing configuration through both code and environment. The provider is particularly valuable for organizations running self-hosted language models via tools like Ollama, LocalAI, or vLLM, as well as those using proxy services or alternative providers like Together AI, Replicate, or custom corporate AI gateways that expose OpenAI-compatible interfaces.

## Diagram

```mermaid
flowchart TD
    subgraph GenericOpenAiProvider["GenericOpenAiProvider"]
        id["id(): 'generic_openai'"]
        name["name(): 'Generic OpenAI API'"]
        default_models["default_models(): Vec<ModelInfo>"]
        create_client["create_client(api_key, base_url, options)"]
    end
    
    subgraph EndpointResolution["Endpoint Resolution Priority"]
        opt["options['endpoint_url']"]
        param["base_url parameter"]
        env["GENERIC_OPENAI_API_BASE env"]
        default["OPENAI_API_BASE default"]
    end
    
    subgraph OpenAiClient["OpenAiClient"]
        new["new(api_key, resolved_base)"]
        impl["LlmClient implementation"]
    end
    
    create_client --> opt
    opt -->|None| param
    param -->|None| env
    env -->|None| default
    default --> new
    param -->|Some| new
    env -->|Some| new
    opt -->|Some| new
    new --> impl
    
    style GenericOpenAiProvider fill:#e1f5e1
    style OpenAiClient fill:#e1e5f5
```

## External Resources

- [OpenAI Chat Completions API specification that compatible endpoints must implement](https://platform.openai.com/docs/api-reference/chat) - OpenAI Chat Completions API specification that compatible endpoints must implement
- [Ollama's OpenAI-compatible API documentation for self-hosted models](https://github.com/ollama/ollama/blob/main/docs/openai.md) - Ollama's OpenAI-compatible API documentation for self-hosted models
- [LocalAI - OpenAI-compatible API for local model inference](https://github.com/mudler/LocalAI) - LocalAI - OpenAI-compatible API for local model inference
- [Twelve-Factor App methodology on configuration management](https://12factor.net/config) - Twelve-Factor App methodology on configuration management

## Sources

- [generic_openai](../sources/generic-openai.md)
