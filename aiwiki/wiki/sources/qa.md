---
title: "Wiki Q&A System (qa.rs)"
source: "qa"
type: source
tags: [rust, wiki, qa, llm, search, markdown, async, tokio, serde, citations, knowledge-base]
generated: "2026-04-18T14:51:31.013749850+00:00"
---

# Wiki Q&A System (qa.rs)

This Rust source file implements a Q&A system for querying wiki content via LLM with source citations. The module provides functionality to search wiki pages based on user questions, extract relevant content, and generate answers with proper citations to source materials. The current implementation includes a stub for LLM integration, with a simple keyword-based search mechanism and frontmatter/heading extraction for page titles.

The core components include the `ask_wiki` function as the main entry point, which accepts a question and optional context pages. If no specific pages are provided, the system performs a keyword search across all markdown files in the wiki directory. Results are ranked by keyword match count and limited to the top 5 most relevant pages. The `generate_qa_response` function currently returns a placeholder response with formatted citations, noting that full LLM integration is pending. Supporting utilities handle recursive directory scanning, markdown file discovery, and title extraction from YAML frontmatter or H1 headings.

The module is designed with async/await patterns using Tokio for file system operations, and integrates with a larger AIWiki system. It includes comprehensive test coverage for title extraction functionality, demonstrating support for both frontmatter-defined titles and fallback to markdown H1 headings.

## Related

### Entities

- [Aiwiki](../entities/aiwiki.md) — product
- [serde](../entities/serde.md) — technology
- [tokio](../entities/tokio.md) — technology
- [QaResult](../entities/qaresult.md) — product
- [Citation](../entities/citation.md) — product

### Concepts

- [Retrieval-Augmented Generation (RAG)](../concepts/retrieval-augmented-generation-rag.md)
- [Keyword-based search](../concepts/keyword-based-search.md)
- [YAML frontmatter parsing](../concepts/yaml-frontmatter-parsing.md)
- [Async file system operations](../concepts/async-file-system-operations.md)
- [Source attribution](../concepts/source-attribution.md)
- [Confidence scoring](../concepts/confidence-scoring.md)
- [Graceful degradation](../concepts/graceful-degradation.md)

