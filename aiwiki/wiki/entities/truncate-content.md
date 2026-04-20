---
title: "truncate_content"
entity_type: "technology"
type: entity
generated: "2026-04-19T17:03:16.926143700+00:00"
---

# truncate_content

**Type:** technology

### From: truncate

The `truncate_content` function serves as the primary truncation utility in the ragent-core toolkit, designed to intelligently limit text output to a specified maximum number of lines while preserving readability through informative omission markers. This function accepts any type implementing `AsRef<str>`, providing flexibility for handling string slices, owned strings, and other string-like types without unnecessary cloning. When the input content exceeds the `max_lines` threshold, the function retains the first `max_lines - 1` lines and appends a marker indicating the count of omitted lines, formatted grammatically for both singular and plural cases. The implementation handles edge cases gracefully: when `max_lines` is zero, it returns an empty string; when content fits within the limit, it returns the original unchanged; and when truncation occurs, it constructs the result through efficient vector joining and string concatenation operations. The grammatical precision of the omission marker—distinguishing between "1 line omitted" and "N lines omitted"—demonstrates attention to user experience details that enhance the professionalism of tool output presentation.

## Diagram

```mermaid
flowchart TD
    A[Input: content, max_lines] --> B{max_lines == 0?}
    B -->|Yes| C[Return empty String]
    B -->|No| D[Split content into lines Vec]
    D --> E{total_lines <= max_lines?}
    E -->|Yes| F[Return content unchanged]
    E -->|No| G[Calculate lines_to_show & lines_omitted]
    G --> H[Join first lines_to_show with newlines]
    H --> I{lines_omitted == 1?}
    I -->|Yes| J[Format singular marker]
    I -->|No| K[Format plural marker]
    J --> L[Append marker to result]
    K --> L
    L --> M[Return truncated content]
```

## External Resources

- [Rust String documentation for understanding string manipulation methods used](https://doc.rust-lang.org/std/string/struct.String.html) - Rust String documentation for understanding string manipulation methods used
- [AsRef trait documentation explaining the flexible type parameter](https://doc.rust-lang.org/std/str/trait.AsRef.html) - AsRef trait documentation explaining the flexible type parameter

## Sources

- [truncate](../sources/truncate.md)
