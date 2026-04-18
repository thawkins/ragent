---
title: "contradiction.rs - Wiki Contradiction Detection Module"
source: "contradiction"
type: source
tags: [rust, contradiction-detection, wiki, markdown-parsing, knowledge-management, ai, fact-extraction, static-analysis, documentation]
generated: "2026-04-18T14:47:52.807385949+00:00"
---

# contradiction.rs - Wiki Contradiction Detection Module

This Rust source file implements a contradiction detection system for an AI-powered wiki platform. The module provides functionality to review wiki pages, extract factual claims, and identify potential contradictions between different pages. The main entry point is the `review_contradictions` function, which loads wiki pages within an optional scope, extracts factual statements from markdown content, and returns a structured review result. The system uses heuristics to identify potential factual claims by looking for declarative sentences containing specific verbs like "is a", "has", "supports", "requires", "uses", and "provides". The module includes comprehensive data structures for representing contradictions, including severity levels, conflicting statements, involved pages, and suggested resolutions. It also provides markdown report generation for human-readable output of review results. Currently, the full LLM-based contradiction detection is marked as TODO, with the implementation returning empty results after loading and analyzing page content.

## Related

### Entities

- [Aiwiki](../entities/aiwiki.md) — product
- [Contradiction](../entities/contradiction.md) — technology
- [ReviewResult](../entities/reviewresult.md) — technology
- [PageData](../entities/pagedata.md) — technology
- [serde](../entities/serde.md) — technology
- [tokio](../entities/tokio.md) — technology

### Concepts

- [Contradiction Detection](../concepts/contradiction-detection.md)
- [Fact Extraction](../concepts/fact-extraction.md)
- [Frontmatter Parsing](../concepts/frontmatter-parsing.md)
- [Recursive Directory Scanning](../concepts/recursive-directory-scanning.md)
- [Markdown Analysis](../concepts/markdown-analysis.md)

