---
title: "Documentation Standards"
type: concept
generated: "2026-04-19T14:54:44.383201071+00:00"
---

# Documentation Standards

### From: ref:AGENTS

The documentation standards establish comprehensive requirements for code-level and project-level documentation, emphasizing machine-readable docblocks and consistent file organization. For source code, the guidelines mandate DOCBLOCK comments—using /// for public APIs and //! for crate-level documentation—that describe purpose, arguments, and return values. This requirement aligns with Rust's rustdoc tool which generates API documentation from these comments. The module-level documentation requirement ensures that every file's purpose and dependencies are immediately apparent to readers. The internal comment convention (//) distinguishes implementation notes from public API documentation.

The file organization rules create a clear separation between root-level documentation files with specific purposes (README.md for overview, QUICKSTART.md for onboarding, SPEC.md for specifications, AGENTS.md for agent guidelines, etc.) and extended documentation in the docs/ directory. This structure supports multiple documentation audiences: end users encounter essential files at the repository root, while comprehensive guides, architecture documentation, and API references reside in the navigable docs/ hierarchy. The prohibition against unrequested explainer documents prevents documentation proliferation that can obscure essential information.

The documentation standards integrate with code quality through the missing_docs warning configuration, ensuring that public APIs remain documented as the codebase evolves. The external reference to djamware.com's Rust best practices indicates awareness of evolving community standards beyond official documentation. The combination of mandatory docblocks, specific file locations, and quality enforcement through Clippy creates a documentation culture where information discovery is predictable and comprehensive. This systematic approach addresses common documentation failures in open-source projects where API drift, outdated guides, and scattered information impede adoption and contribution.

## External Resources

- [Rustdoc guide on writing documentation](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html) - Rustdoc guide on writing documentation
- [Rust project structure and best practices reference](https://www.djamware.com/post/68b2c7c451ce620c6f5efc56/rust-project-structure-and-best-practices-for-clean-scalable-code) - Rust project structure and best practices reference

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
