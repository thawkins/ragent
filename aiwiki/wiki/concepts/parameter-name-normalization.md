---
title: "Parameter Name Normalization"
type: concept
generated: "2026-04-19T17:09:53.939245532+00:00"
---

# Parameter Name Normalization

### From: aliases

Parameter name normalization is the process of transforming semantically equivalent but syntactically different parameter names into a canonical form before processing. In ragent's aliases module, this is implemented for cases like `start`/`end` versus `start_line`/`end_line` in file reading tools, `directory` versus `path` in listing tools, and `old`/`new` versus `old_str`/`new_str` in editing tools. The normalization logic typically checks if the canonical key exists, and if not, attempts to promote an alias value to the canonical position in the input JSON object.

This technique addresses a dimension of LLM variability orthogonal to tool name hallucination: even when models select the correct tool name, they may use parameter names that differ from the schema. This can occur because different frameworks use different conventions (Python's `start` vs Rust's `start_line`), because models generalize from similar APIs, or because natural language descriptions of parameters lead to predictable alternative namings. The `extract_command` function in ragent demonstrates sophisticated normalization, handling not just string-to-string mapping but also array-to-string transformation for shell commands where models might provide `["bash", "-c", "..."]` instead of a command string.

The implementation pattern in ragent uses mutable modification of the input Value, cloning values from alias keys to canonical keys before delegation. This approach maintains backward compatibility while allowing gradual migration to preferred schemas. The design reflects practical lessons from production agent systems where strict schema adherence causes friction, and where the cost of accepting multiple parameter names is low compared to the robustness gained. Normalization is particularly important for agent frameworks aiming to work with multiple model providers, as each may have subtly different tendencies in parameter naming derived from their training data.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input JSON"]
        I1["{path: '/etc/passwd', start: 1, end: 10}"]
        I2["{directory: '/home/user'}"]
        I3["{code: 'echo hello'}"]
    end
    
    subgraph Normalization["Parameter Normalization"]
        N1["start/end → start_line/end_line"]
        N2["directory → path"]
        N3["code → command"]
    end
    
    subgraph Canonical["Canonical Form"]
        C1["{path: '/etc/passwd', start_line: 1, end_line: 10}"]
        C2["{path: '/home/user'}"]
        C3["{command: 'echo hello'}"]
    end
    
    subgraph Tools["Canonical Tools"]
        T1[read::ReadTool]
        T2[list::ListTool]
        T3[bash::BashTool]
    end
    
    I1 --> N1
    I2 --> N2
    I3 --> N3
    
    N1 --> C1
    N2 --> C2
    N3 --> C3
    
    C1 --> T1
    C2 --> T2
    C3 --> T3
```

## External Resources

- [JSON Schema specification for parameter validation](https://json-schema.org/) - JSON Schema specification for parameter validation
- [Serde serialization framework used in ragent](https://serde.rs/) - Serde serialization framework used in ragent

## Sources

- [aliases](../sources/aliases.md)
