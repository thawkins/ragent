---
title: "RmTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:23:42.242020248+00:00"
---

# RmTool

**Type:** technology

### From: rm

RmTool is a Rust struct implementing a secure file deletion tool for agent-based systems. It provides a constrained interface specifically designed to prevent dangerous operations like wildcard-based mass deletions. The tool explicitly rejects paths containing `*`, `?`, or `[` characters, which are standard glob pattern metacharacters in Unix-like systems. This design decision prioritizes safety over convenience, ensuring that automated agents must explicitly enumerate files for deletion rather than using pattern matching.

The struct implements the `Tool` trait, making it discoverable and callable through the ragent-core framework's JSON-based tool system. This architecture allows language models and other automated systems to invoke file deletion through a well-defined, schema-validated interface. The tool returns structured output including both human-readable messages and machine-parseable metadata about the deletion operation. Error handling is comprehensive, with specific error messages for missing parameters, invalid paths, non-existent files, directories (which cannot be deleted by this tool), and permission failures.

Security is layered throughout the implementation. Beyond glob rejection, the tool performs path resolution that respects working directory context, validates path containment within a defined root directory, and uses asynchronous I/O to prevent blocking. The permission category system allows operators to restrict which agent instances can perform destructive file operations.

## Diagram

```mermaid
flowchart TD
    subgraph InputValidation["Input Validation"]
        A[Receive JSON Input] --> B[Extract path parameter]
        B --> C{Contains wildcards?<br/>* ? [}
        C -->|Yes| D[Error: Wildcards not allowed]
        C -->|No| E[Resolve path]
    end
    
    subgraph PathResolution["Path Resolution"]
        E --> F{Is absolute?}
        F -->|Yes| G[Use as-is]
        F -->|No| H[Join with working_dir]
        G --> I[Check within root]
        H --> I
        I --> J{Within root?}
        J -->|No| K[Error: Path outside root]
        J -->|Yes| L[Continue]
    end
    
    subgraph FileValidation["File Validation"]
        L --> M{Exists?}
        M -->|No| N[Error: File not found]
        M -->|Yes| O{Is directory?}
        O -->|Yes| P[Error: Is directory]
        O -->|No| Q[Proceed to delete]
    end
    
    subgraph Execution["Execution"]
        Q --> R[Tokio remove_file]
        R --> S{Success?}
        S -->|No| T[Error: Delete failed]
        S -->|Yes| U[Return success with metadata]
    end
```

## External Resources

- [Rust PathBuf documentation for path manipulation](https://doc.rust-lang.org/std/path/struct.PathBuf.html) - Rust PathBuf documentation for path manipulation
- [Tokio async runtime for non-blocking file operations](https://tokio.rs/tokio/tutorial/async) - Tokio async runtime for non-blocking file operations
- [Anyhow error handling library used in implementation](https://docs.rs/anyhow/latest/anyhow/) - Anyhow error handling library used in implementation

## Sources

- [rm](../sources/rm.md)
