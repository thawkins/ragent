---
title: "Token Storage Security"
type: concept
generated: "2026-04-19T21:26:02.204197879+00:00"
---

# Token Storage Security

### From: auth

Secure credential storage represents a critical security concern for applications that persist authentication tokens to disk. OAuth access tokens grant significant privileges to applications, making their protection essential for preventing unauthorized access to user accounts and data. This authentication module implements multiple layers of protection for stored GitHub tokens, reflecting security best practices for CLI applications. The primary mechanism is Unix file permissions, specifically setting the token file to mode 0o600 (read/write for owner only) using `PermissionsExt::from_mode`. This prevents other users on the same system from reading the token, even if they have filesystem access. The module stores tokens in a hidden directory within the user's home directory (`~/.ragent/github_token`), following the XDG Base Directory Specification pattern while maintaining simplicity. The code also prioritizes environment variable-based tokens (`GITHUB_TOKEN`) over file storage, allowing users to use secrets management tools, CI/CD environment injection, or memory-only credentials when appropriate. The `delete_token` function enables secure cleanup when credentials are no longer needed. These measures align with the principle of least privilege and defense in depth, acknowledging that filesystem security is just one component of a comprehensive credential protection strategy that should also include transport security (TLS), memory protection, and user education.

## External Resources

- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) - XDG Base Directory Specification
- [OWASP Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html) - OWASP Secrets Management Cheat Sheet
- [GitHub token security best practices](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation) - GitHub token security best practices

## Sources

- [auth](../sources/auth.md)
