---
title: "Semantic Versioning"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:27:28.523881513+00:00"
---

# Semantic Versioning

**Type:** technology

### From: mod

Semantic Versioning, commonly abbreviated as SemVer, is a versioning scheme that uses a three-part version number (MAJOR.MINOR.PATCH) optionally followed by pre-release metadata. The ragent updater implements a custom semantic versioning parser that handles standard versions like '1.2.3' as well as pre-release variants such as '0.1.0-alpha.21'. The implementation decomposes version strings into comparable tuples of (major, minor, patch, pre_release_suffix), enabling lexicographical comparison that respects SemVer precedence rules. Pre-release versions are considered lower precedence than their release counterparts, and numeric pre-release identifiers are compared appropriately. This system allows ragent to determine when updates are available while correctly handling the complexities of development-stage releases where multiple alpha or beta versions may exist between stable releases.

## External Resources

- [Official Semantic Versioning 2.0.0 specification](https://semver.org/) - Official Semantic Versioning 2.0.0 specification
- [Cargo's interpretation of semantic versioning for Rust crates](https://doc.rust-lang.org/cargo/reference/semver.html) - Cargo's interpretation of semantic versioning for Rust crates

## Sources

- [mod](../sources/mod.md)
