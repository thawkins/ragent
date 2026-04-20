---
title: "SearchSink"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:55:31.954593952+00:00"
---

# SearchSink

**Type:** technology

### From: search

SearchSink is a private struct that implements the Sink trait from the grep_searcher crate, serving as the critical bridge between ripgrep's streaming search results and the tool's collected output format. This component demonstrates the visitor pattern in action, where the search engine calls back into user-provided code for each match found, allowing custom processing without buffering entire files in memory. The struct holds three fields: a path string for relative path display, an Arc-wrapped Mutex-protected Vec<String> for thread-safe result collection, and a maximum result limit for early termination.

The Sink trait implementation requires defining an associated Error type and implementing the matched method. SearchSink uses std::io::Error for compatibility with the underlying I/O operations, and its matched implementation performs several important transformations. It converts raw byte slices to lossy UTF-8 strings (handling potentially invalid UTF-8 gracefully), trims trailing whitespace for clean output, formats the result in the conventional path:line: content format, and atomically appends to the shared results vector. The return value indicates whether searching should continue, enabling efficient early termination when the result limit is reached.

This design pattern is particularly elegant for concurrent searching scenarios. While this implementation processes files sequentially, the Arc<Mutex<Vec<String>>> structure would support parallel searching if the walker were configured for it. The use of String::from_utf8_lossy demonstrates practical handling of real-world file encodings, accepting that source code files may occasionally contain non-UTF-8 byte sequences without failing the entire search. The trim_end call removes trailing newlines and whitespace that would otherwise clutter the formatted output, while the optional line number (defaulting to 0) provides location information when available from the searcher.

## External Resources

- [Sink trait documentation - callback interface for search results](https://docs.rs/grep-searcher/latest/grep_searcher/trait.Sink.html) - Sink trait documentation - callback interface for search results
- [Rust documentation for lossy UTF-8 conversion](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy) - Rust documentation for lossy UTF-8 conversion

## Sources

- [search](../sources/search.md)
