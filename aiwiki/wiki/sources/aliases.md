---
title: "Ragent Tool Aliases: LLM-Compatible Name Mapping System"
source: "aliases"
type: source
tags: [rust, llm, agent-systems, tool-calling, api-design, async, ai-compatibility, ragent, serde, anyhow]
generated: "2026-04-19T17:09:53.934835982+00:00"
---

# Ragent Tool Aliases: LLM-Compatible Name Mapping System

This document describes a Rust module from the ragent-core crate that implements a comprehensive aliasing system for LLM tool calls. The aliases.rs file provides wrapper implementations that map commonly hallucinated or alternative tool names to canonical implementations, enabling seamless interoperability between large language models trained on different coding-agent frameworks and the ragent agent system. The module addresses a critical challenge in AI agent design: LLMs often emit tool names that differ from the canonical names expected by the runtime, either because they were trained on different frameworks (like OpenAI's Agents SDK) or because they extrapolate semantically plausible names from task context.

The implementation follows a consistent delegation pattern where each alias struct implements the Tool trait with normalized parameter schemas, then delegates execution to the underlying canonical tool after performing any necessary parameter name remapping. For example, tools like `view_file`, `read_file`, and `get_file_contents` all map to the canonical `read` tool, with parameter normalization handling variations like `start`/`end` versus `start_line`/`end_line`. Similarly, bash execution tools accept multiple parameter names (`command`, `code`, `cmd`) through the `extract_command` helper function, which even handles array-based command representations. This design ensures robustness against LLM output variability while maintaining clean canonical implementations.

The module covers six functional categories: file reading (with multiple aliases), directory listing, file globbing, text searching (including OpenAI SDK compatibility), file editing, file writing, shell execution (with five different aliases), user interaction, and file opening. Each alias includes proper permission category tagging for security enforcement, comprehensive parameter schemas with descriptions for LLM consumption, and async execution via the `#[async_trait::async_trait]` macro. The implementation demonstrates sophisticated understanding of real-world LLM behavior patterns, accommodating not just name variations but also structural differences in how models represent the same conceptual operations.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [OpenAI Agents SDK](../entities/openai-agents-sdk.md) — technology
- [async-trait](../entities/async-trait.md) — technology
- [Claude Code](../entities/claude-code.md) — product

### Concepts

- [Tool Name Hallucination](../concepts/tool-name-hallucination.md)
- [Parameter Name Normalization](../concepts/parameter-name-normalization.md)
- [Delegation Pattern in Tool Design](../concepts/delegation-pattern-in-tool-design.md)
- [Permission Categories in Agent Security](../concepts/permission-categories-in-agent-security.md)
- [JSON Schema for LLM Tool Specifications](../concepts/json-schema-for-llm-tool-specifications.md)

