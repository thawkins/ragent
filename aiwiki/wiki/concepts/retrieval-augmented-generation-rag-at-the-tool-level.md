---
title: "Retrieval-Augmented Generation (RAG) at the Tool Level"
type: concept
generated: "2026-04-19T19:55:41.391894591+00:00"
---

# Retrieval-Augmented Generation (RAG) at the Tool Level

### From: aiwiki_search

Retrieval-Augmented Generation (RAG) typically refers to pattern where retrieved documents are injected into LLM prompts to ground generation in external knowledge. This implementation demonstrates a tool-level variant where the retrieval mechanism itself is exposed as a capability that agents can invoke explicitly rather than having retrieval automatically applied to every generation. The `AiwikiSearchTool` provides structured search with filtering, excerpts, and metadata, giving agents fine-grained control over when and how to access external knowledge.

This architectural choice reflects important trade-offs in agent design. Automatic RAG systems retrieve context based on query embedding similarity, which can be opaque and difficult to debug when wrong documents are retrieved. By contrast, tool-based RAG makes retrieval an explicit reasoning step where the agent decides whether external knowledge is needed, constructs appropriate queries, and interprets results. The page type filtering (entities, concepts, sources, analyses) enables targeted retrieval where agents can express intent about what kind of knowledge they seek, impossible with pure embedding similarity. The excerpt inclusion provides immediate relevance signals without requiring full document retrieval.

The pattern also enables richer interaction models. Failed searches return explicit "no results" messages that agents can incorporate into reasoning, rather than simply having no context to work with. The enabled/disabled state management allows graceful degradation where agents can be informed that knowledge bases are unavailable and potentially take corrective action. The result limiting and pagination hints suggest support for large result sets that agents might need to browse iteratively. These capabilities exceed what typical automatic RAG provides and align with emerging research on tool-augmented language models where explicit tool use outperforms implicit retrieval for complex knowledge tasks.

## External Resources

- [Retrieval-Augmented Generation for Knowledge-Intensive NLP (original RAG paper)](https://arxiv.org/abs/2005.11401) - Retrieval-Augmented Generation for Knowledge-Intensive NLP (original RAG paper)
- [Tool-Augmented Language Models - explicit tool use patterns](https://arxiv.org/abs/2312.08301) - Tool-Augmented Language Models - explicit tool use patterns

## Sources

- [aiwiki_search](../sources/aiwiki-search.md)
