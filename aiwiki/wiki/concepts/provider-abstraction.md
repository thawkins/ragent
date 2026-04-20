---
title: "Provider Abstraction"
type: concept
generated: "2026-04-19T15:13:13.811792012+00:00"
---

# Provider Abstraction

### From: mod

Provider abstraction in LLM client libraries addresses the fragmentation of API conventions across services—OpenAI's `/v1/chat/completions`, Anthropic's `/v1/messages`, Google's distinct formats, and open-source model servers with OpenAI-compatible or custom interfaces. This module achieves abstraction through the `LlmClient` trait, defining a common interface while allowing provider-specific implementations to handle translation. The abstraction enables application portability: code written against `Box<dyn LlmClient>` works with any backend without modification, supporting runtime provider switching, A/B testing across models, and migration strategies.

The abstraction layer must reconcile significant semantic differences. Message formats vary—some use `system`/`user`/`assistant` roles, others add `tool`, different services structure content as strings versus arrays. Tool definitions use subtly incompatible JSON Schema subsets. Streaming formats differ in event names, data structures, and termination signals. Reasoning/thinking blocks are provider-specific features. This module's `ChatRequest`, `ChatMessage`, and `StreamEvent` types represent a unified superset—applications use common fields while `options: HashMap<String, Value>` escapes to provider specifics. Implementations handle normalization: mapping unified types to provider formats, parsing responses into `StreamEvent` sequences, and managing authentication, retries, and rate limits.

The tradeoff between abstraction completeness and feature accessibility shapes the design. Pure least-common-denominator abstraction would exclude advanced features; this module instead provides escape hatches. The `options` map forwards arbitrary parameters—Anthropic's `thinking` budget, OpenAI's `response_format`—without struct changes. Re-exporting `FinishReason` and related types creates shared vocabulary for stop conditions. The trait's `Send + Sync` bounds and `anyhow::Result` error type support production deployment patterns. Testing benefits significantly: mock implementations verify application logic, and provider-specific integration tests validate translation correctness. The abstraction anticipates ecosystem evolution—new providers implement the trait, existing implementations update as APIs change, applications remain stable.

## External Resources

- [Adapter pattern on Wikipedia](https://en.wikipedia.org/wiki/Adapter_pattern) - Adapter pattern on Wikipedia
- [Rust trait objects and dynamic dispatch](https://doc.rust-lang.org/book/ch17-02-trait-objects.html) - Rust trait objects and dynamic dispatch
- [OpenAI API reference showing provider-specific conventions](https://platform.openai.com/docs/api-reference) - OpenAI API reference showing provider-specific conventions

## Sources

- [mod](../sources/mod.md)
