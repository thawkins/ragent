---
title: "WebSearchTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:54:43.999053397+00:00"
---

# WebSearchTool

**Type:** technology

### From: websearch

WebSearchTool is the primary public struct in this module, serving as the main interface for web search functionality within the Ragent agent framework. This struct implements the `Tool` trait, which is the foundational abstraction for all capabilities that agents can invoke. The struct itself is a zero-sized type (unit struct), containing no fields, which reflects its stateless nature and emphasizes that all configuration comes from environment variables and runtime parameters rather than instance state.

The implementation of WebSearchTool follows a deliberate design philosophy common in Rust agent frameworks: tools are lightweight, composable, and self-describing. The `name()` method returns "websearch", providing a stable identifier for agent systems to reference this capability. The `description()` method returns a detailed explanation of functionality and requirements, which can be exposed to language models for tool selection decisions. This self-documenting approach enables dynamic tool discovery where agents can understand available capabilities without hardcoded knowledge.

The `execute()` method represents the core business logic, accepting JSON input and returning structured `ToolOutput`. This method handles parameter extraction with proper validation, environment variable retrieval for the Tavily API key, and delegation to the private `tavily_search()` function. Error handling is comprehensive: missing query parameters, empty queries, and missing API credentials all produce clear error messages. The method also implements result formatting, converting structured search results into human-readable text with numbered entries while preserving metadata about the query and result counts.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Validation"]
        I1[Extract query parameter] --> I2{Query present?}
        I2 -->|No| E1[Error: Missing query]
        I2 -->|Yes| I3{Query empty?}
        I3 -->|Yes| E2[Error: Empty query]
        I3 -->|No| I4[Extract num_results with defaults]
    end
    
    subgraph Auth["Authentication"]
        A1[Read TAVILY_API_KEY env var] --> A2{Key present?}
        A2 -->|No| E3[Error: Missing API key]
        A2 -->|Yes| A3[Proceed to API call]
    end
    
    subgraph API["Tavily API Call"]
        P1[Build HTTP client with timeout] --> P2[Construct TavilyRequest]
        P2 --> P3[POST to api.tavily.com/search]
        P3 --> P4{HTTP success?}
        P4 -->|No| E4[Error: API failure]
        P4 -->|Yes| P5[Parse TavilyResponse]
    end
    
    subgraph Output["Result Formatting"]
        O1[Iterate through results] --> O2[Truncate snippets to ~200 chars]
        O2 --> O3[Format as numbered list with title, URL, snippet]
        O3 --> O4[Build ToolOutput with content and metadata]
    end
    
    I4 --> A1
    A3 --> P1
    P5 --> O1
```

## External Resources

- [Tavily AI search API platform](https://tavily.com) - Tavily AI search API platform
- [Ragent framework source repository](https://github.com/thawkins/ragent) - Ragent framework source repository

## Sources

- [websearch](../sources/websearch.md)
