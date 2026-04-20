---
title: "URI Shortening and Path Normalization"
type: concept
generated: "2026-04-19T18:22:18.104321298+00:00"
---

# URI Shortening and Path Normalization

### From: lsp_diagnostics

URI shortening and path normalization is a user experience optimization implemented in LspDiagnosticsTool to produce readable, context-relative output from absolute file URIs. The `shorten_uri` function addresses the common problem where LSP servers return file paths in `file://` URI format with absolute system paths, producing verbose output like `file:///home/user/projects/myapp/src/main.rs` when `src/main.rs` would suffice for user understanding. The implementation attempts to parse the URI as a URL, convert it to a filesystem path, strip the current working directory prefix if present, and return either the relative path or the original representation if conversion fails.

The implementation demonstrates defensive programming appropriate for handling external data from potentially varying LSP server implementations. The function uses a cascading approach with `let`-chains and pattern matching: first attempting URL parsing, then file path extraction, then prefix stripping with careful handling of leading separators. The use of `to_string_lossy` for path conversion acknowledges that not all paths are valid UTF-8, providing a best-effort string representation rather than failing outright. This resilience ensures that diagnostic output remains useful even when encountering unusual file paths or non-standard URI formats from language servers.

Path normalization in this context serves multiple practical purposes beyond mere aesthetics. In containerized and distributed development environments, absolute paths often contain system-specific prefixes that vary across machines, making diagnostic output non-portable and difficult to compare. Relative paths anchored to the project root provide consistent, reproducible references that support documentation generation, test result comparison, and collaborative debugging. For AI agents processing diagnostic output, shorter relative paths reduce token consumption in language model contexts and improve pattern matching reliability when correlating diagnostics with source code.

The careful handling of path separators—specifically the chained `strip_prefix` calls that remove both the working directory and a subsequent `/`—reveals attention to cross-platform compatibility. While the code appears Unix-oriented with forward slash assumptions, the underlying `PathBuf` operations would adapt to Windows conventions when running on that platform. The fallback to `uri.to_string()` ensures that non-file URIs (such as those referencing remote resources or virtual documents) are preserved intact rather than being mangled by inappropriate path conversion attempts. This comprehensive approach to URI handling reflects production-quality implementation suitable for diverse deployment scenarios.

## External Resources

- [URL Standard specification](https://url.spec.whatwg.org/) - URL Standard specification
- [Rust url crate documentation](https://docs.rs/url/latest/url/) - Rust url crate documentation
- [Rust standard library PathBuf documentation](https://doc.rust-lang.org/std/path/struct.PathBuf.html) - Rust standard library PathBuf documentation

## Sources

- [lsp_diagnostics](../sources/lsp-diagnostics.md)
