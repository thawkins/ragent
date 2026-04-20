---
title: "Structured Code Search"
type: concept
generated: "2026-04-19T17:26:49.145566525+00:00"
---

# Structured Code Search

### From: codeindex_search

Structured code search represents an evolution beyond text-based pattern matching, leveraging parsed program representations to understand code semantics. The `CodeIndexSearchTool` embodies this concept by operating on a code index that encodes knowledge of symbol kinds, relationships, and locations rather than raw file contents. This enables queries like "find all trait implementations named Display" that would be imprecise or impossible with `grep` alone. The structured approach understands that `impl Display for MyType` contains distinct semantic components—a keyword, a trait name, and a type name—each searchable and filterable.

The implementation demonstrates practical tradeoffs in structured search design. The index stores symbol signatures and documentation snippets but not full bodies (`include_body: false`), balancing search relevance against index size and query performance. Kind filtering spans 15 symbol types across multiple languages, revealing the challenge of unified code intelligence across language boundaries. The tool's description explicitly positions it against `grep` for "named code entities," educating users on appropriate tool selection—a common challenge in rich tool ecosystems where multiple approaches overlap in capability.

Structured search indexes must balance completeness against maintenance burden. The fallback to `grep`, `glob`, and LSP-based tools when the index is unavailable suggests the index is optional infrastructure rather than a core dependency. This reflects real-world deployment constraints where indexing large codebases may be computationally expensive or temporarily unavailable. The 100-result maximum and default of 20 indicate expected query patterns—precise symbol lookups rather than broad exploration—shaping the user experience toward targeted discovery.

## External Resources

- [Abstract syntax trees - foundation of structured code representation](https://en.wikipedia.org/wiki/Abstract_syntax_tree) - Abstract syntax trees - foundation of structured code representation
- [SCIP - code intelligence protocol](https://scip.dev/) - SCIP - code intelligence protocol
- [Sourcegraph semantic code search introduction](https://sourcegraph.com/blog/introduction-to-semantic-code-search) - Sourcegraph semantic code search introduction

## Sources

- [codeindex_search](../sources/codeindex-search.md)
