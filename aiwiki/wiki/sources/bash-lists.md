---
title: "Ragent Bash Lists Module: Runtime Security Policy Management"
source: "bash_lists"
type: source
tags: [rust, security, configuration-management, bash, allowlist, denylist, agent-systems, concurrency, json-configuration, runtime-policy]
generated: "2026-04-19T21:30:23.527398179+00:00"
---

# Ragent Bash Lists Module: Runtime Security Policy Management

This Rust source file implements a runtime security policy system for the ragent project, managing allowlists and denylists for bash command execution. The module provides thread-safe, in-memory storage of security policies backed by persistent JSON configuration files at both global (user) and project levels. The architecture separates read-heavy validation operations from write-heavy mutation operations, using RwLock for concurrent access and OnceLock for lazy initialization of global state. The system integrates with ragent's broader configuration framework, loading merged settings from `~/.config/ragent/ragent.json` and `./ragent.json` at startup, then persisting runtime mutations back to the targeted scope. This design enables fine-grained control over command execution security, where allowlist entries exempt commands from banned-command checks (treating them as trusted prefixes) while denylist entries act as unconditional substring patterns that reject matching commands regardless of other security settings.

The module's security model reflects a defense-in-depth approach common in agent-based AI systems, where automatic code execution requires multiple layers of validation. By distinguishing between prefix-based allowlisting (for trusted commands like `curl` or `wget`) and substring-based denylisting (for dangerous patterns), the system provides flexible yet robust protection. The persistence layer uses atomic file operations and pretty-printed JSON to ensure human-readable, version-controllable configuration files. The Scope enum enables users to choose between project-local policies (shared via version control) and global personal preferences, supporting collaborative development workflows while maintaining individual security postures. The implementation demonstrates idiomatic Rust patterns for global mutable state, error handling with the anyhow crate, and safe concurrency primitives.

## Related

### Entities

- [BashLists](../entities/bashlists.md) — technology
- [Scope](../entities/scope.md) — technology
- [anyhow](../entities/anyhow.md) — technology

### Concepts

- [Runtime Security Policy Management](../concepts/runtime-security-policy-management.md)
- [Global Mutable State in Rust](../concepts/global-mutable-state-in-rust.md)
- [Allowlist/Denylist Pattern](../concepts/allowlist-denylist-pattern.md)
- [Configuration Merging and Scope](../concepts/configuration-merging-and-scope.md)

