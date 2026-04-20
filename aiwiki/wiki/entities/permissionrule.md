---
title: "PermissionRule"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:24:51.863562426+00:00"
---

# PermissionRule

**Type:** technology

### From: mod

The `PermissionRule` struct represents a single declarative policy statement within the ragent-core permission system. Each rule associates a specific `Permission` type (such as Read, Edit, or Bash) with a glob pattern string and a resulting `PermissionAction` (Allow, Deny, or Ask). This three-tuple structure enables expressive policy definitions that can match resource paths using standard glob syntax, supporting wildcards, character classes, and recursive directory traversal patterns. The struct derives standard Rust traits including `Debug`, `Clone`, `Serialize`, and `Deserialize`, enabling persistence and transmission across system boundaries. Rules are typically aggregated into a `PermissionRuleset` (a type alias for `Vec<PermissionRule>`) where their ordering becomes significant due to the last-match-wins evaluation semantics. This design pattern, familiar from CSS selector specificity and firewall rule ordering, allows more specific exceptions to override broader base policies when placed later in the sequence. The glob pattern integration leverages the `globset` crate's efficient matching algorithms, compiling patterns into optimized matchers for repeated evaluation against multiple resource paths.

## Diagram

```mermaid
erDiagram
    PERMISSION_RULE ||--|| PERMISSION : permission_type
    PERMISSION_RULE ||--|| PERMISSION_ACTION : action
    PERMISSION_RULE {
        string pattern
    }
    PERMISSION {
        enum variants
    }
    PERMISSION_ACTION {
        Allow
        Deny
        Ask
    }
```

## External Resources

- [Glob pattern syntax and history](https://en.wikipedia.org/wiki/Glob_(programming)) - Glob pattern syntax and history
- [Globset Glob pattern compilation](https://docs.rs/globset/latest/globset/struct.Glob.html) - Globset Glob pattern compilation

## Sources

- [mod](../sources/mod.md)
