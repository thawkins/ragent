---
title: "Granular Component Caching"
type: concept
generated: "2026-04-19T15:52:20.010543244+00:00"
---

# Granular Component Caching

### From: cache

Granular component caching is an architectural pattern implemented in the SystemPromptCache that recognizes system prompts as composite structures composed of independent sub-components with different change frequencies and dependency profiles. Rather than treating the system prompt as an indivisible unit, this approach decomposes it into logical components—agent prompts, tool references, LSP guidance, code index guidance, and team guidance—each with dedicated cache storage and targeted invalidation triggers. This decomposition is based on the observation that in production LLM systems, certain inputs change frequently while others remain stable over long periods.

The economic rationale for granular caching becomes clear when examining typical change patterns. Agent prompts may change with every user interaction as context evolves, while tool definitions remain constant unless new capabilities are registered. LSP connection state fluctuates with editor availability, and team membership changes only during explicit collaboration actions. By caching these independently, the system avoids the multiplicative cost of rebuilding entire prompts when only one component changes. The implementation achieves this through separate `Mutex<Cached<String>>` fields for each component, each paired with dependency-specific hash tracking.

The invalidation strategy for each component reflects its unique dependency structure. Tool reference caching uses a hash of the ToolRegistry's definitions, recomputing only when tool signatures change. LSP guidance employs a hash of server connection states, capturing both language and status fields. Code index guidance tracks a simple boolean for activation state, while team guidance hashes the team context structure. Agent prompts use a compound key of agent name and prompt content hash, enabling multiple agents to coexist with independent cache entries. This design requires careful consideration of hash stability—changes in hash computation must be versioned to prevent false cache hits—and demonstrates how domain analysis of data change patterns can drive cache architecture decisions.

## Diagram

```mermaid
flowchart TB
    subgraph SystemPrompt["System Prompt Composition"]
        direction TB
        
        subgraph Components["Cached Components"]
            ap[Agent Prompt<br/>key: (name, hash)<br/>changes: per message]
            tr[Tool Reference<br/>key: registry hash<br/>changes: registration]
            ls[LSP Guidance<br/>key: server states<br/>changes: connection]
            ci[Code Index<br/>key: active boolean<br/>changes: toggle]
            tm[Team Guidance<br/>key: context hash<br/>changes: membership]
        end
        
        compose[Compose Full Prompt]
    end
    
    llm[LLM API Call]
    
    ap --> compose
    tr --> compose
    ls --> compose
    ci --> compose
    tm --> compose
    compose --> llm
    
    style Components fill:#e8f5e9
    style compose fill:#fff3e0
```

## External Resources

- [HTTP caching patterns and cache granularity](https://web.dev/articles/http-cache) - HTTP caching patterns and cache granularity
- [LRU cache implementation and memoization patterns](https://docs.python.org/3/library/functools.html#functools.lru_cache) - LRU cache implementation and memoization patterns
- [Database research on fine-grained caching strategies](https://www.vldb.org/pvldb/vol14/p1277-varts.pdf) - Database research on fine-grained caching strategies

## Sources

- [cache](../sources/cache.md)
