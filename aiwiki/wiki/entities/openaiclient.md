---
title: "OpenAiClient"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:26:42.456314028+00:00"
---

# OpenAiClient

**Type:** technology

### From: generic_openai

The `OpenAiClient` is the underlying HTTP client implementation that handles actual communication with OpenAI-compatible API endpoints. This struct is instantiated by `GenericOpenAiProvider` with a resolved base URL and API key, encapsulating all network operations, request serialization, response handling, and error management. The client implements the `LlmClient` trait, providing a common interface that allows the broader system to work with different LLM providers interchangeably through dynamic dispatch.

The client's design follows Rust's ownership and borrowing rules, taking ownership of the API key string and base URL string during construction. This ensures thread safety and eliminates lifetime complications that would arise from borrowed references. The `new` constructor accepts the resolved base URL from the provider, meaning `OpenAiClient` itself is agnostic to how that URL was determined—it simply uses whatever endpoint it's given. This separation of concerns between provider configuration and client execution enables clean testing and modular architecture.

The `OpenAiClient` implements the complete OpenAI Chat Completions API protocol, including streaming and non-streaming request variants, proper header authentication via Bearer tokens, JSON request/response handling through serde, and appropriate error conversion to the `anyhow::Result` type used throughout the crate. By wrapping this client in `Box<dyn LlmClient>`, the `GenericOpenAiProvider` enables runtime polymorphism, allowing different provider implementations to return their specific client types through a common interface. This pattern is essential for supporting multiple LLM backends in a unified application architecture.

## Diagram

```mermaid
classDiagram
    class LlmClient {
        <<trait>>
        +chat(request) Result~Response~
        +chat_stream(request) Result~Stream~
    }
    
    class OpenAiClient {
        -api_key: String
        -base_url: String
        -client: HttpClient
        +new(api_key, base_url) Self
    }
    
    class GenericOpenAiProvider {
        +create_client() Result~Box~LlmClient~~
    }
    
    OpenAiClient ..|> LlmClient : implements
    GenericOpenAiProvider ..> OpenAiClient : instantiates
    GenericOpenAiProvider ..> LlmClient : returns as trait object
```

## External Resources

- [Reqwest HTTP client library commonly used for API clients in Rust](https://docs.rs/reqwest/latest/reqwest/) - Reqwest HTTP client library commonly used for API clients in Rust
- [Serde serialization framework for Rust](https://serde.rs/) - Serde serialization framework for Rust

## Sources

- [generic_openai](../sources/generic-openai.md)
