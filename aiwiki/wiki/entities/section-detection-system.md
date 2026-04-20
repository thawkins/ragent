---
title: "Section Detection System"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:09:27.052551225+00:00"
---

# Section Detection System

**Type:** technology

### From: read

The section detection system in read.rs implements a language-aware structural analysis capability that transforms raw file content into navigable semantic units. This system is specifically designed to address the challenge of large file handling in AI agent contexts, where loading an entire 10,000-line source file into a context window would be wasteful and potentially counterproductive. When a file exceeds 100 lines and no specific line range is requested, the tool activates its section detection logic to provide a high-level overview of the file's organization, enabling targeted subsequent access.

The detection architecture follows an extensible pattern matching approach where file extensions map to specialized detection functions. Twelve distinct language handlers are implemented: Rust (`detect_rust_sections`), Markdown (`detect_markdown_sections`), Python (`detect_python_sections`), JavaScript/TypeScript (`detect_js_sections`), TOML (`detect_toml_sections`), YAML (`detect_yaml_sections`), INI/configuration files (`detect_ini_sections`), C/C++ (`detect_c_sections`), Java/Kotlin (`detect_java_sections`), Go (`detect_go_sections`), Ruby (`detect_ruby_sections`), and CSS/SCSS/Less (`detect_css_sections`). Each handler implements language-specific heuristics to identify meaningful structural boundaries. For example, Rust detection recognizes function definitions through patterns like `pub fn`, `async fn`, `pub(crate) fn`, as well as struct, enum, trait, impl, and module declarations. The detection uses prefix matching on trimmed lines combined with delimiter extraction to capture meaningful labels while avoiding false positives from comments or string literals.

The `markers_to_sections` function converts raw detection results into contiguous, non-overlapping `Section` structs that represent the complete file structure. Each section records its 1-based start and end line numbers along with a descriptive label extracted from the source code. The conversion logic handles edge cases such as empty marker lists and ensures that sections span from their starting marker to either the next marker's starting line minus one, or the end of the file for the final section. This normalized representation enables consistent presentation across all supported languages and facilitates precise line-range calculations for subsequent tool invocations. The section labels preserve original source formatting where meaningful (like Markdown headers) or extract signature information (like function declarations), providing agents with sufficient context to make informed navigation decisions.

## Diagram

```mermaid
flowchart LR
    subgraph Detection["Language Detection"]
        A["File extension"] --> B{"Match ext"}
        B -->|rs| C[detect_rust_sections]
        B -->|py| D[detect_python_sections]
        B -->|js/ts| E[detect_js_sections]
        B -->|md| F[detect_markdown_sections]
        B -->|toml| G[detect_toml_sections]
        B -->|yaml| H[detect_yaml_sections]
        B -->|c/cpp| I[detect_c_sections]
        B -->|java/kt| J[detect_java_sections]
        B -->|go| K[detect_go_sections]
        B -->|rb| L[detect_ruby_sections]
        B -->|css| M[detect_css_sections]
        B -->|ini/cfg| N[detect_ini_sections]
    end
    
    subgraph Processing["Marker Processing"]
        O["Raw markers<br/>(line, label)"] --> P[markers_to_sections]
        P --> Q["Calculate end lines"]
        Q --> R["Build Section structs"]
    end
    
    subgraph Output["Output"]
        R --> S["Section map in preview"]
    end
    
    C --> O
    D --> O
    E --> O
    F --> O
    G --> O
    H --> O
    I --> O
    J --> O
    K --> O
    L --> O
    M --> O
    N --> O
    S --> T["Agent uses start_line/end_line"]
```

## External Resources

- [Tree-sitter parsing library (production-grade alternative approach)](https://tree-sitter.github.io/tree-sitter/) - Tree-sitter parsing library (production-grade alternative approach)
- [Abstract syntax tree concepts in compiler design](https://en.wikipedia.org/wiki/Abstract_syntax_tree) - Abstract syntax tree concepts in compiler design

## Sources

- [read](../sources/read.md)
