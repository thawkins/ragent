---
title: "Security Boundary Configuration"
type: concept
generated: "2026-04-19T15:06:38.736160766+00:00"
---

# Security Boundary Configuration

### From: mod

Security boundary configuration in ragent addresses the fundamental tension between agent autonomy and system safety, particularly regarding command execution in development environments. The `BashConfig` struct implements a defense-in-depth approach through complementary allowlist and denylist mechanisms, acknowledging that purely blacklist-based security fails against creative attacker ingenuity while purely whitelist-based approaches stifle legitimate functionality. The union merge semantics—where global and project configurations combine rather than replace—enable organizational security policies to coexist with project-specific needs.

The allowlist pattern (command prefixes that bypass banned-command checks) recognizes that common utilities like `curl`, `wget`, or `git` may be essential for development workflows despite their potential for malicious use. By requiring explicit prefix matching, the system prevents substring bypass attacks where dangerous commands embed allowed strings. The denylist (substring patterns that always reject) provides emergency brake functionality for specific dangerous patterns like `git push --force` or `rm -rf /` variants, catching commands even when embedded in complex pipelines or shell scripts. This dual mechanism reflects the reality that security policies emerge from multiple sources: organizational compliance requirements, project-specific conventions, and individual risk tolerance.

The broader permission system referenced in `Config.permission` extends this boundary concept beyond bash execution to filesystem access, network operations, and potentially other capability domains. The vector of `PermissionRule` structures suggests policy evaluation against multiple dimensions, enabling fine-grained access control that can respond to context: different permissions might apply to read versus write operations, or to different directory subtrees. This architecture positions ragent for future hardening through sandboxing integration, capability-based security models, or integration with operating system permission frameworks—while current implementation focuses on configurable policy rather than mandatory enforcement.

## External Resources

- [OWASP Command Injection Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Command_Injection_Cheat_Sheet.html) - OWASP Command Injection Prevention Cheat Sheet
- [Principle of Least Privilege security concept](https://en.wikipedia.org/wiki/Principle_of_least_privilege) - Principle of Least Privilege security concept
- [Seccomp for system call filtering and sandboxing](https://docs.docker.com/engine/security/seccomp/) - Seccomp for system call filtering and sandboxing
- [Firejail security sandbox for Linux applications](https://firejail.wordpress.com/) - Firejail security sandbox for Linux applications

## Sources

- [mod](../sources/mod.md)
