---
title: "EditOp"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:53:10.506546669+00:00"
---

# EditOp

**Type:** technology

### From: multiedit

EditOp is a private struct that serves as the internal representation of a single edit operation within the MultiEditTool system. It acts as a bridge between the raw JSON input received by the tool and the structured data needed for execution. The struct contains three fields: a PathBuf representing the resolved file path, a String containing the exact text to search for, and a String containing the replacement text. This simple but precise structure encapsulates all information needed to perform one atomic replacement operation within a larger batch.

The design of EditOp reflects careful consideration of Rust's ownership and borrowing rules. By using owned String types rather than string slices, the struct can be stored in collections and moved between functions without lifetime complications. The PathBuf type provides cross-platform path handling with proper Unicode support and path normalization. The struct is constructed during the parsing phase of MultiEditTool execution, where JSON values are extracted, paths are resolved against the working directory, and string values are converted from JSON string references to owned Rust strings.

EditOp instances are collected into a Vec and processed in order, which matters when multiple edits target the same file—the changes accumulate sequentially on the in-memory content. This design choice means that earlier edits in the array affect the context for later edits to the same file, creating a predictable and intuitive behavior where edits compose naturally. The struct's privacy (being private to the module) enforces that EditOp instances can only be created through proper validation in the MultiEditTool implementation, preventing malformed edit operations from entering the system.

## External Resources

- [Rust PathBuf documentation for cross-platform path handling](https://doc.rust-lang.org/std/path/struct.PathBuf.html) - Rust PathBuf documentation for cross-platform path handling
- [Rust ownership and borrowing concepts](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) - Rust ownership and borrowing concepts

## Sources

- [multiedit](../sources/multiedit.md)
