---
title: "OpenAI API Key Format"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.192141224+00:00"
---

# OpenAI API Key Format

**Type:** technology

### From: sanitize

OpenAI API keys follow a distinctive format that has become a de facto standard for LLM provider authentication, typically beginning with `sk-` for standard keys or `sk-live-`/`sk-test-` for Stripe-integrated deployments. The pattern `sk[-_][a-zA-Z0-9_\-]{20,}` in `sanitize.rs` specifically targets these credentials, recognizing both hyphen and underscore separators and requiring at least 20 alphanumeric characters to avoid false positives on short strings. This format emerged from OpenAI's 2020 API launch and has been widely imitated by other AI providers. The security implications of API key exposure are severe: leaked keys can enable unauthorized access to expensive model inference, potentially resulting in thousands of dollars in fraudulent usage within hours. OpenAI provides key revocation capabilities through their developer dashboard, but detection often lags behind exposure, making proactive redaction in logs and error messages critical. The regex pattern's inclusion of underscore variations (`sk_`) accommodates provider-specific implementations that deviate from the canonical hyphen format, demonstrating practical experience with real-world credential diversity.

## External Resources

- [OpenAI API authentication documentation](https://platform.openai.com/docs/api-reference/authentication) - OpenAI API authentication documentation
- [OpenAI API key management interface](https://platform.openai.com/account/api-keys) - OpenAI API key management interface

## Sources

- [sanitize](../sources/sanitize.md)
