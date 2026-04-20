---
title: "Personal Access Token Security"
type: concept
generated: "2026-04-19T22:02:39.831918814+00:00"
---

# Personal Access Token Security

### From: auth

Personal Access Token (PAT) security encompasses the practices and mechanisms for generating, storing, transmitting, and revoking time-limited credentials that grant programmatic access to services. In GitLab's implementation, PATs serve as bearer tokens with configurable scopes and expiration, reducing blast radius compared to password-based authentication. The ragent module implements several PAT security best practices: encrypted storage at rest using database encryption rather than filesystem plaintext, secure transmission via HTTPS with proper header-based authentication, and immediate validation to detect compromised or expired tokens.

The module's security architecture reflects defense-in-depth principles. Tokens never appear in process command lines (unlike URL-embedded credentials), reducing exposure to `/proc` inspection and shell history. The `PRIVATE-TOKEN` header transmission avoids query parameter logging risks. The validation function provides cryptographic proof of possession without exposing the token to additional services—ragent validates directly against the GitLab instance rather than through intermediary authentication servers. This design supports air-gapped and self-hosted GitLab deployments where external validation services may be unavailable.

Migration from legacy file storage (`~/.ragent/gitlab_token`) to encrypted database demonstrates security evolution in response to threat model refinement. Filesystem storage risks include: world-readable permissions on multi-user systems, backup and sync tool exposure, and lack of encryption at rest. The migration function's conditional logic—only migrating when database entries don't exist—prevents accidental credential overwriting while enabling seamless upgrades. The explicit cleanup of migrated files (`remove_file` on success) reduces credential sprawl, though error handling allows continued operation if cleanup fails, prioritizing availability over perfect security hygiene.

## External Resources

- [OWASP Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html) - OWASP Secrets Management Cheat Sheet
- [GitHub PAT security practices (applicable to GitLab)](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) - GitHub PAT security practices (applicable to GitLab)

## Sources

- [auth](../sources/auth.md)
