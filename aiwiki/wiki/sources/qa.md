---
title: "Wiki Q&A System - Rust Implementation"
source: "qa"
type: source
tags: [rust, qa-system, wiki, llm-integration, markdown-processing, ai, search, citations, async-rust, tokio]
generated: "2026-04-18T15:18:35.506009987+00:00"
---

# Wiki Q&A System - Rust Implementation

This Rust source code implements a wiki Q&A (question-answering) system that allows users to query wiki content and receive AI-generated answers with source citations. The module `qa.rs` provides functionality to search through markdown wiki files, extract relevant content based on keyword matching, and generate structured responses. The current implementation serves as a foundation with stub LLM integration, preparing for future expansion where an actual language model would process the queries.

The system is built around several key components: the `QaResult` struct which encapsulates the answer text, citations, and confidence score; the `Citation` struct for tracking sources; and the `PageContent` struct for internal processing. The main entry point is the `ask_wiki` async function, which coordinates between loading specific pages or searching across the entire wiki, then generating a response. The implementation includes a simple keyword-based relevance scoring system that limits results to the top 5 most relevant pages, along with utilities for extracting titles from YAML frontmatter or Markdown H1 headings.

## Related

### Entities

- [Aiwiki](../entities/aiwiki.md) — product
- [serde](../entities/serde.md) — technology
- [tokio](../entities/tokio.md) — technology
- [LLM](../entities/llm.md) — technology

### Concepts

- [Retrieval-Augmented Generation](../concepts/retrieval-augmented-generation.md)
- [Keyword-based Search](../concepts/keyword-based-search.md)
- [Relevance Scoring](../concepts/relevance-scoring.md)
- [Source Citation](../concepts/source-citation.md)
- [YAML Frontmatter Parsing](../concepts/yaml-frontmatter-parsing.md)
- [Asynchronous File Operations](../concepts/asynchronous-file-operations.md)
- [Recursive Directory Scanning](../concepts/recursive-directory-scanning.md)
- [Feature Flag / Enablement Check](../concepts/feature-flag--enablement-check.md)

