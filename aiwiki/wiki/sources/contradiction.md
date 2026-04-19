---
title: "contradiction.rs - Wiki Contradiction Detection Module"
source: "contradiction"
type: source
tags: [rust, wiki, contradiction-detection, markdown-parsing, fact-extraction, content-analysis, async, serde]
generated: "2026-04-18T15:20:20.978460687+00:00"
---

# contradiction.rs - Wiki Contradiction Detection Module

This Rust source file implements contradiction detection functionality for the AIWiki system. The module provides automated analysis of wiki pages to identify potential factual inconsistencies across different documents. The core functionality is encapsulated in the `review_contradictions` function, which loads wiki pages, extracts factual claims, and generates a report of detected contradictions. The implementation includes structs for representing contradictions (`Contradiction`) and review results (`ReviewResult`), along with helper functions for parsing markdown content, extracting titles from frontmatter, and identifying declarative sentences that may contain factual claims.

The current implementation serves as a foundation with TODO comments indicating future integration with LLM-based comparison for more sophisticated contradiction detection. The module handles markdown file scanning, frontmatter parsing, and report generation in markdown format. It includes comprehensive test coverage for the fact extraction and report generation functions. The code demonstrates async/await patterns for file I/O operations and uses serde for serialization of contradiction data.

## Related

### Entities

- [AIWiki](../entities/aiwiki.md) — product
- [serde](../entities/serde.md) — technology
- [tokio](../entities/tokio.md) — technology
- [chrono](../entities/chrono.md) — technology

### Concepts

- [Contradiction Detection](../concepts/contradiction-detection.md)
- [Fact Extraction](../concepts/fact-extraction.md)
- [Frontmatter Parsing](../concepts/frontmatter-parsing.md)
- [Async File I/O](../concepts/async-file-i-o.md)
- [Recursive Directory Scanning](../concepts/recursive-directory-scanning.md)
- [Report Generation](../concepts/report-generation.md)

