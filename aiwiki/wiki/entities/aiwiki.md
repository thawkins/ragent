---
title: "aiwiki"
entity_type: "product"
type: entity
generated: "2026-04-19T19:55:41.390671073+00:00"
---

# aiwiki

**Type:** product

### From: aiwiki_search

AIWiki is a knowledge base system designed for AI agent consumption and interaction, referenced throughout this implementation as an external crate (`aiwiki::`). The system provides structured storage and retrieval of information organized into semantic categories including entities, concepts, sources, and analyses. This taxonomy suggests AIWiki is not merely a document store but a knowledge graph with typed nodes representing different epistemological categories of information.

The integration patterns shown in the code reveal AIWiki's operational model. The `Aiwiki::exists()` static method checks for initialization state in a working directory, indicating file-system based persistence rather than purely in-memory or network-remote storage. The `Aiwiki::new()` constructor is async and fallible, suggesting potential I/O operations or lock acquisition during initialization. The `search_wiki()` function accepts the wiki instance, query string, and optional page type filter, returning structured results with titles, paths, and excerpts—indicating rich document indexing with content summarization capabilities.

The configuration system exposed through `wiki.config.enabled` implies AIWiki has toggleable operational states, useful for deployments where knowledge base features may be conditionally available. The command references in error messages (`/aiwiki init`, `/aiwiki on`) suggest AIWiki includes a command interface, possibly for a REPL or chat-based agent environment. This design mirrors patterns from projects like MemGPT and other agent memory systems, where knowledge bases are first-class citizens with explicit lifecycle management rather than transparent caches.

## External Resources

- [Microsoft GraphRAG - knowledge graph approach to RAG systems](https://github.com/microsoft/graphrag) - Microsoft GraphRAG - knowledge graph approach to RAG systems
- [MemGPT - agent memory and knowledge management system](https://memgpt.ai/) - MemGPT - agent memory and knowledge management system

## Sources

- [aiwiki_search](../sources/aiwiki-search.md)

### From: aiwiki_ingest

The `aiwiki` crate represents the foundational technology stack underlying the AIWiki knowledge management system. This Rust library provides the core infrastructure for creating, managing, and interacting with AI-enhanced wikis that integrate with language models and agent systems. The crate abstracts complex operations such as document parsing, content hashing, text extraction from various formats, and hierarchical storage management into a coherent API that tools like `AiwikiIngestTool` can leverage.

At the heart of the crate is the `Aiwiki` struct, which encapsulates the wiki's configuration, storage layout, and operational state. The crate implements sophisticated ingestion pipelines that can process diverse document types including Markdown, plain text, PDF, Microsoft Word (.docx), and OpenDocument (.odt) formats. These pipelines typically involve format detection, binary-to-text extraction where applicable, content hashing for deduplication, and organized storage in a `raw/` directory structure. The `IngestOptions` and `IngestionResult` types provide fine-grained control over ingestion behavior and comprehensive feedback about operation outcomes.

The technology emphasizes reliability and reproducibility through content-addressable storage patterns. By computing cryptographic hashes of ingested content, the system can detect modifications, prevent duplicates, and maintain precise provenance tracking. The crate also implements directory scanning algorithms that efficiently identify new and changed files, supporting incremental synchronization workflows. Integration with the broader agent ecosystem is facilitated through structured metadata output and clear command interfaces, as evidenced by the `/aiwiki` command namespace referenced throughout the tool implementation. The separation between ingestion (copying files to `raw/`) and synchronization (generating wiki pages) allows for batch processing and review workflows.

### From: aiwiki_export

AIWiki is a knowledge management system designed specifically for AI agent interactions, providing structured storage and retrieval of information gathered during conversational workflows. The system maintains wiki content as markdown files with associated metadata, enabling both human readability and machine processing. Unlike traditional wikis, AIWiki is optimized for automatic population through agent tools, with semantic understanding of concepts, entities, and relationships extracted from natural language conversations. The architecture supports multiple export formats, recognizing that knowledge captured by agents often needs to migrate to user-preferred environments.

The AIWiki implementation referenced in this source code provides several key operations through its public API: existence checking via `Aiwiki::exists()`, initialization through `Aiwiki::new()`, and format-specific export functions. The system maintains configuration state including an enabled/disabled flag, allowing users to control when agents can access or modify wiki content. This design supports iterative knowledge building where users might disable the wiki during sensitive conversations or while restructuring their knowledge base. The working directory-based storage model ensures portability and version control compatibility, treating the wiki as a standard filesystem directory that can be managed with existing developer tools.

AIWiki's export capabilities demonstrate its commitment to data portability and user ownership. The `export_single_markdown()` function consolidates all wiki pages into one document with proper heading hierarchy and cross-references, ideal for documentation generation or simple backups. The `export_obsidian_vault()` function produces a directory structure compatible with Obsidian's conventions, including proper file organization and link formats that enable the application's powerful graph visualization features. These export functions handle edge cases like missing pages, circular references, and filesystem permissions, providing robust error messages that help users resolve issues independently.
