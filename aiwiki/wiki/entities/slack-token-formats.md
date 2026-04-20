---
title: "Slack Token Formats"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.192751196+00:00"
---

# Slack Token Formats

**Type:** technology

### From: sanitize

Slack's authentication tokens use the `xoxb-` (bot token) and `xoxp-` (user token) prefixes, with the regex pattern `xox[bp]-[a-zA-Z0-9\-]{20,}` capturing both variants. These tokens originate from Slack's OAuth 2.0 implementation, where the prefix indicates the token type and authorization context. Bot tokens (`xoxb-`) are associated with Slack apps and grant permissions within specific workspaces, while user tokens (`xoxp-`) act on behalf of individual users with their full permission scope. Slack's token format has remained remarkably stable since the platform's 2013 API launch, though the company has gradually tightened security requirements including mandatory token rotation and granular permission scopes. The inclusion of hyphen in the character class (`[a-zA-Z0-9\-]`) is particularly important for Slack tokens, which frequently contain multiple hyphens as delimiters within the token body. Slack tokens are common in AI agent deployments because Slack represents a primary interface for business automation—agents frequently post messages, read channel history, and interact with Slack workflows. Compromised tokens can expose sensitive organizational communications and enable phishing attacks through compromised bot identities.

## External Resources

- [Slack API documentation on token types](https://api.slack.com/authentication/token-types) - Slack API documentation on token types
- [Slack OAuth and authentication basics](https://api.slack.com/authentication/basics) - Slack OAuth and authentication basics

## Sources

- [sanitize](../sources/sanitize.md)
