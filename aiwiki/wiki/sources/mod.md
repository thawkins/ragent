---
title: "AIWiki Analysis Module (mod.rs)"
source: "mod"
type: source
tags: [rust, aiwiki, content-analysis, markdown-processing, yaml-frontmatter, wiki, llm-integration, text-processing, async-rust, serde]
generated: "2026-04-18T15:16:13.987264438+00:00"
---

# AIWiki Analysis Module (mod.rs)

This Rust source code defines the analysis module for AIWiki, a system that provides AI-powered content generation and analysis capabilities for wiki documents. The module implements functionality for generating comparative analyses between multiple wiki sources, including support for different analysis types such as comparisons, synthesis, trade-off evaluations, and custom analyses. The code includes public APIs for generating analyses and listing existing ones, with internal implementations for content generation, slug conversion, and YAML frontmatter parsing.

The module is structured with two public submodules: `qa` for question-answering with source citations, and `contradiction` for detecting contradictions across wiki pages. The main functionality revolves around the `generate_analysis` function, which orchestrates reading source content, generating analysis output with YAML frontmatter metadata, and writing results to markdown files. While the current implementation uses template-based content generation, the architecture is designed to accommodate future LLM integration for fully automated analysis generation.

## Related

### Entities

- [AIWiki](../entities/aiwiki.md) — product
- [Rust](../entities/rust.md) — technology
- [Serde](../entities/serde.md) — technology
- [Tokio](../entities/tokio.md) — technology
- [Chrono](../entities/chrono.md) — technology

### Concepts

- [Slug Generation](../concepts/slug-generation.md)
- [YAML Frontmatter](../concepts/yaml-frontmatter.md)
- [Multi-Source Analysis](../concepts/multi-source-analysis.md)
- [Async/Await Pattern](../concepts/async-await-pattern.md)
- [LLM Integration Stub](../concepts/llm-integration-stub.md)

