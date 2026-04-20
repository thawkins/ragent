---
title: "Multimodal Content Representation"
type: concept
generated: "2026-04-19T15:32:39.532149602+00:00"
---

# Multimodal Content Representation

### From: gemini

The multimodal content handling in this implementation demonstrates the architectural challenges of unified interfaces across diverse media types. The `ChatContent` and `ContentPart` enums provide an abstraction layer that normalizes heterogeneous inputs—plain text, inline images, tool invocations, and tool results—into a structured format that can be transformed into provider-specific representations. This design enables the same application code to process text conversations, vision-enabled queries, and agentic tool-using workflows without branching logic.

For image processing, the implementation specifically handles data URI schemes (RFC 2397), which encode binary image data as base64 within a text URL format like `data:image/jpeg;base64,/9j/4AAQ...`. The parser extracts the MIME type from the `data:` prefix and semicolon-delimited media type specification, then locates the comma separator to isolate the base64 payload. This inline encoding avoids external URL dependencies that would require additional HTTP requests, though the code notes a limitation: Gemini requires either inline data or Google Cloud Storage URIs, so external HTTP URLs are converted to placeholder text representations rather than being fetched.

Tool use and tool results represent another multimodal dimension, structured as distinct content part variants. The `ToolUse` variant carries function name and argument mapping for model-initiated invocations, while `ToolResult` contains the execution output returned to the model. The transformation to Gemini's format maps these to `functionCall` and `functionResponse` objects respectively, with careful ID generation and name extraction to maintain correlation between invocations and results. This bidirectional transformation enables conversational agents where models can request external computation and incorporate results into ongoing dialogue, a pattern essential for retrieval-augmented generation, code execution, and API-integrated applications.

## External Resources

- [RFC 2397: The 'data' URL scheme](https://tools.ietf.org/html/rfc2397) - RFC 2397: The 'data' URL scheme
- [Gemini API vision and multimodal capabilities](https://ai.google.dev/gemini-api/docs/vision) - Gemini API vision and multimodal capabilities
- [Gemini function calling documentation](https://ai.google.dev/gemini-api/docs/function-calling) - Gemini function calling documentation

## Sources

- [gemini](../sources/gemini.md)
