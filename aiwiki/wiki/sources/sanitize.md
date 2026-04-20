---
title: "Ragent-Core Sanitization Module: Secret Redaction System"
source: "sanitize"
type: source
tags: [rust, security, secret-management, redaction, pii-protection, regex, concurrency, ai-agents, credential-sanitization, data-privacy]
generated: "2026-04-19T21:32:34.190519907+00:00"
---

# Ragent-Core Sanitization Module: Secret Redaction System

The `sanitize.rs` module in ragent-core provides a comprehensive secret redaction system designed to prevent sensitive information leakage in AI agent applications. This Rust-based implementation employs a dual-layer approach combining exact-match string replacement with regex-based pattern detection to identify and mask API keys, authentication tokens, passwords, and other credentials that might appear in logs, error messages, or output streams. The module maintains thread-safe global state through `LazyLock` synchronization primitives, allowing runtime registration and unregistration of secrets while ensuring safe concurrent access across multiple threads.

The architecture distinguishes between two complementary redaction strategies. The first layer uses an in-memory registry of known secret values stored in a `RwLock<HashSet<String>>`, enabling precise substring matching for secrets explicitly registered at runtime—typically loaded from databases or environment variables during application startup. The second layer applies a comprehensive regex pattern that recognizes common secret formats including OpenAI/Stripe API keys (`sk-` prefixed), GitHub tokens (`ghp_`, `gho_`, etc.), Slack tokens (`xoxb-`, `xoxp-`), AWS access keys (`AKIA...`), Bearer tokens/JWTs, and generic credential patterns in URLs or configuration strings. This hybrid approach ensures both flexibility for custom secrets and broad coverage of standard formats without requiring explicit registration.

The implementation demonstrates several important Rust patterns for systems programming: deferred initialization with `LazyLock` for expensive regex compilation, reader-writer locking for efficient concurrent access to the secret registry, and careful ordering of replacement operations (longest secrets first) to prevent partial masking issues. The module's API design prioritizes safety with empty string checks, provides bulk operations for efficient initialization, and includes comprehensive documentation with usage examples. These characteristics make it particularly well-suited for AI agent frameworks where prompts, responses, and intermediate processing steps may inadvertently contain sensitive credentials that must be sanitized before logging or display.

## Related

### Entities

- [Regex Crate](../entities/regex-crate.md) — technology
- [RwLock Synchronization Primitive](../entities/rwlock-synchronization-primitive.md) — technology
- [LazyLock Deferred Initialization](../entities/lazylock-deferred-initialization.md) — technology
- [OpenAI API Key Format](../entities/openai-api-key-format.md) — technology
- [GitHub Personal Access Token Formats](../entities/github-personal-access-token-formats.md) — technology
- [Slack Token Formats](../entities/slack-token-formats.md) — technology
- [AWS Access Key ID Format](../entities/aws-access-key-id-format.md) — technology

