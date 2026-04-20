---
title: "OllamaProvider"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:44:02.748694955+00:00"
---

# OllamaProvider

**Type:** technology

### From: ollama

The `OllamaProvider` struct is the core configuration and factory component within the ragent-core crate's provider architecture, implementing the `Provider` trait to enable seamless integration with Ollama servers. This struct encapsulates the connection parameters and lifecycle management for Ollama instances, serving as the entry point for creating executable LLM clients. The provider pattern employed here follows a separation of concerns where configuration (the provider) is distinct from execution (the client), allowing for flexible resource management and connection pooling strategies.

The implementation demonstrates sophisticated configuration discovery mechanisms. When instantiated via `new()`, the provider first checks for the `OLLAMA_HOST` environment variable, falling back to a sensible default of `http://localhost:11434` which corresponds to Ollama's standard port. This environment-aware initialization supports development workflows where developers may have Ollama running on non-standard ports or remote machines. The `with_url()` constructor provides explicit programmatic control, enabling runtime configuration based on user preferences or service discovery systems. Both constructors normalize URLs by stripping trailing slashes, preventing common URL construction bugs.

A key distinguishing feature of this provider is its support for dynamic model discovery through the `discover_models()` method. Unlike providers for commercial APIs that maintain static model lists, Ollama's model availability depends entirely on what the user has pulled locally. The provider queries Ollama's `/api/tags` endpoint to enumerate available models at runtime, parsing response structures like `OllamaTagsResponse` and `OllamaModelEntry`. This enables applications to present accurate model selectors and validate configurations against actual server state. The provider also implements intelligent context window estimation based on parameter size strings (like "70B" or "8B"), mapping these to appropriate token limits without requiring explicit configuration.

## Diagram

```mermaid
flowchart TD
    subgraph Provider["OllamaProvider Lifecycle"]
        A[Environment Variable<br/>OLLAMA_HOST] --> B{Check Config}
        C[Explicit URL<br/>with_url] --> B
        B --> D[Normalize URL<br/>strip trailing slash]
        D --> E[Provider Instance]
    end
    
    subgraph Discovery["Model Discovery"]
        E --> F[GET /api/tags]
        F --> G[Parse OllamaTagsResponse]
        G --> H[Extract OllamaModelEntry]
        H --> I[Estimate Context Window]
        I --> J[Return Vec&lt;ModelInfo&gt;]
    end
    
    subgraph ClientCreation["Client Factory"]
        E --> K[create_client called]
        K --> L{API key empty?}
        L -->|Yes| M[No auth header]
        L -->|No| N[Bearer token auth]
        M --> O[OllamaClient instance]
        N --> O
    end
```

## Sources

- [ollama](../sources/ollama.md)
