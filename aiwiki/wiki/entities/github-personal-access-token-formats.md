---
title: "GitHub Personal Access Token Formats"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.192406376+00:00"
---

# GitHub Personal Access Token Formats

**Type:** technology

### From: sanitize

GitHub employs multiple token formats distinguished by prefixes indicating their type and authorization scope: `ghp_` for personal access tokens, `gho_` for OAuth access tokens, `ghs_` for GitHub App server-to-server tokens, `ghu_` for GitHub App user-to-server tokens, and `ghr_` for refresh tokens. The regex pattern `gh[pousr]_[a-zA-Z0-9]{20,}` in `sanitize.rs` efficiently captures all variants using a character class. GitHub introduced these prefixed formats in 2021, replacing the legacy 40-character hexadecimal tokens that were visually indistinguishable from SHA-1 commit hashes. This change was part of GitHub's broader secret scanning program, which now automatically revokes detected tokens across public repositories. The 20+ character base64url requirement in the pattern reflects GitHub's actual implementation using URL-safe base64 encoding. GitHub tokens are particularly high-value targets as they often grant extensive repository access, including code read/write permissions and in some cases administrative capabilities. The module's explicit inclusion of these patterns reflects the common occurrence of GitHub tokens in AI agent contexts, where agents frequently interact with code repositories, issue trackers, and CI/CD pipelines.

## External Resources

- [GitHub documentation on personal access tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) - GitHub documentation on personal access tokens
- [GitHub blog post explaining the 2021 token format redesign](https://github.blog/2021-04-05-behind-githubs-new-authentication-token-formats/) - GitHub blog post explaining the 2021 token format redesign

## Sources

- [sanitize](../sources/sanitize.md)
