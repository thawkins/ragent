---
title: "ExtractionEngine"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:35:00.215856645+00:00"
---

# ExtractionEngine

**Type:** technology

### From: mod

ExtractionEngine represents ragent's automated knowledge acquisition subsystem, responsible for identifying and extracting memorable information from agent interactions without explicit human curation. This component analyzes session message summaries and tool call patterns to detect candidates for long-term memory storage, implementing a form of computational introspection where the system evaluates its own experiences for lasting value. The engine produces `MemoryCandidate` instances representing potential additions to the knowledge base, which may include learned patterns, successful problem-solving approaches, domain insights, or corrected misconceptions that improved agent performance.

The extraction process incorporates confidence scoring through the `decay_confidence` function, enabling time-based degradation of memory certainty that mirrors human forgetting curves. This biological inspiration allows the system to naturally deprioritize stale information while maintaining stronger confidence in recently reinforced memories. The `SessionMessageSummary` and `ToolCallSummary` types provide structured representations of agent activity, enabling pattern recognition across similar situations and identification of reusable solutions. This automated extraction reduces the manual overhead of knowledge base maintenance while ensuring agents continuously learn from their experiences.

The engine's design reflects sophisticated understanding of machine learning operations, treating agent sessions as training data for behavioral refinement. By extracting `ExtractedEntity` and `ExtractedRelationship` information for the knowledge graph, the engine builds structured representations of domain concepts and their interconnections. This dual output—both narrative memory blocks and structured graph data—provides complementary retrieval mechanisms: semantic similarity search for contextual recall and graph traversal for relational reasoning. The integration with the broader memory system's compaction and deduplication ensures extracted knowledge remains high-quality and non-redundant over time.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Session Analysis"]
        messages["SessionMessageSummary"]
        tools["ToolCallSummary"]
    end
    
    subgraph Engine["ExtractionEngine"]
        pattern["Pattern Detection"]
        insight["Insight Recognition"]
        correction["Error Correction Detection"]
    end
    
    Input --> Engine
    
    subgraph Output["Extraction Outputs"]
        candidate["MemoryCandidate"]
        entity["ExtractedEntity"]
        relationship["ExtractedRelationship"]
    end
    
    Engine --> Output
    
    subgraph Confidence["Confidence Management"]
        initial["Initial Confidence Score"]
        decay["decay_confidence over time"]
        reinforcement["Reinforcement on Reuse"]
    end
    
    candidate --> Confidence
    
    subgraph Storage["Dual Storage"]
        blocks["Memory Blocks (Narrative)"]
        kg["Knowledge Graph (Structured)"]
    end
    
    Output --> Storage
    
    subgraph Retrieval["Retrieval Methods"]
        semantic["Semantic Search"]
        graph["Graph Traversal"]
    end
    
    Storage --> Retrieval
```

## External Resources

- [Ebbinghaus forgetting curve - basis for confidence decay](https://en.wikipedia.org/wiki/Forgetting_curve) - Ebbinghaus forgetting curve - basis for confidence decay
- [Research on automated knowledge extraction from conversations](https://arxiv.org/abs/2009.00031) - Research on automated knowledge extraction from conversations

## Sources

- [mod](../sources/mod.md)
