---
title: "Output Truncation and Resource Limits"
type: concept
generated: "2026-04-19T16:11:18.098532352+00:00"
---

# Output Truncation and Resource Limits

### From: libreoffice_common

Output truncation is a defensive programming technique used to prevent unbounded resource consumption when processing potentially large inputs. The `truncate_output` function in this module implements intelligent truncation that respects content boundaries by truncating at newlines rather than mid-word. With a limit of 100KB (`MAX_OUTPUT_BYTES`), it balances providing useful information with preventing memory exhaustion or denial-of-service conditions. The implementation demonstrates sophisticated boundary handling: if truncation is needed, it finds the last newline before the limit using `rfind('\n')`, ensuring the truncated output ends cleanly.

This approach to resource limiting reflects lessons from production systems where unbounded outputs have caused failures. The specific threshold of 100KB suggests this is intended for display purposes or LLM context windows, where extremely large outputs are neither useful nor processable. The function's design with `#[must_use]` annotation prevents accidental discarding of the potentially-truncated result. The formatting of the truncation notice (`"... [Output truncated at {}KB.]"`) clearly communicates to users that content has been omitted, preventing confusion about incomplete data. The `unwrap_or(MAX_OUTPUT_BYTES)` fallback ensures the function never panics even if no newline is found.

The broader concept of resource limiting applies throughout robust systems design. Similar patterns appear in request size limits for web servers, query result limits for databases, and recursion depth limits for parsers. In document processing specifically, limits prevent "zip bomb" attacks where compressed archives expand to consume excessive memory, or documents with billion laughs-style XML entity expansion attacks. While this module doesn't implement full protection against such attacks, the truncation pattern establishes a defensive mindset. The choice to truncate at newlines rather than arbitrary byte positions shows attention to user experience, producing output that remains readable and parseable even when limited.

## External Resources

- [Wikipedia on zip bomb attacks and decompression bombs](https://en.wikipedia.org/wiki/Zip_bomb) - Wikipedia on zip bomb attacks and decompression bombs
- [Wikipedia on the billion laughs XML entity expansion attack](https://en.wikipedia.org/wiki/Billion_laughs_attack) - Wikipedia on the billion laughs XML entity expansion attack

## Related

- [defensive programming](defensive-programming.md)

## Sources

- [libreoffice_common](../sources/libreoffice-common.md)
