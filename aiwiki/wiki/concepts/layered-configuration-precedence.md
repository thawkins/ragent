---
title: "Layered Configuration Precedence"
type: concept
generated: "2026-04-19T15:06:38.735413283+00:00"
---

# Layered Configuration Precedence

### From: mod

Layered configuration precedence is a fundamental software engineering pattern that enables flexible, context-sensitive behavior in complex systems while maintaining predictable override semantics. The ragent implementation demonstrates this pattern through five distinct layers, each successively overriding values from previous layers. This approach solves the tension between distributing defaults (which should work for most users without configuration) and enabling customization (which must be possible without modifying source code). The compiled defaults layer establishes invariant baselines and documentation-through-code, ensuring the system functions even in complete absence of configuration files.

The global and project file layers introduce user and repository context respectively. Global configuration in `~/.config/ragent/ragent.json` follows XDG Base Directory specifications, respecting user preferences across all projects while remaining outside version control. Project-local `ragent.json` enables teams to share operational settings—preferred providers, agent variants, LSP configurations—through version control, ensuring consistent behavior across development environments. This separation respects the 12-factor app methodology's distinction between codebase (shared) and config (environment-specific), though with nuanced overlap where team conventions blur the boundary.

Environment variable layers provide the highest precedence for deployment-specific secrets and ephemeral configurations. The `RAGENT_CONFIG` path indirection enables containerized deployments to mount configuration from Kubernetes secrets or similar orchestration mechanisms, while `RAGENT_CONFIG_CONTENT` enables CI/CD pipelines to inject complete configurations without filesystem operations. The deep merge algorithm implemented in `Config::merge` handles complex nested structures intelligently: HashMaps extend with overlay precedence, vectors append rather than replace (enabling permission and instruction accumulation), and primitive fields apply standard Option-based override logic. This sophisticated merging prevents the common pitfall where entire configuration subtrees must be duplicated to override single values.

## External Resources

- [12-Factor App Configuration methodology](https://12factor.net/config) - 12-Factor App Configuration methodology
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) - XDG Base Directory Specification
- [Serde serialization framework for Rust](https://serde.rs/) - Serde serialization framework for Rust
- [Configuration inheritance patterns in software design](https://en.wikipedia.org/wiki/Inheritance_(object-oriented_programming)) - Configuration inheritance patterns in software design

## Sources

- [mod](../sources/mod.md)
