---
title: "PermissionChecker"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:24:51.863166476+00:00"
---

# PermissionChecker

**Type:** technology

### From: mod

The `PermissionChecker` struct serves as the central evaluation engine for the permission system in ragent-core. It maintains two primary data structures: a static `ruleset` containing ordered `PermissionRule` definitions, and a dynamic `always_grants` HashMap that stores permanent grants established at runtime. The checker implements a hierarchical evaluation strategy where permanent grants are checked first, followed by ruleset evaluation using a last-match-wins algorithm. This design allows administrators to configure base policies while enabling users to establish persistent exceptions through interactive confirmation workflows. The struct provides two primary public methods: `new()` for initialization with a ruleset, and `check()` for evaluating permission requests against both static rules and runtime grants. The `record_always()` method enables the accumulation of permanent grants, which are stored as compiled `globset::GlobMatcher` instances for efficient pattern matching. This architecture supports high-performance permission evaluation with predictable precedence rules, making it suitable for real-time agent decision-making scenarios.

## Diagram

```mermaid
flowchart TD
    subgraph PermissionChecker["PermissionChecker Evaluation Flow"]
        direction TB
        start(["check(permission, path)"])
        checkAlways["Check always_grants HashMap"]
        matchAlways{"Matcher found?"}
        evalRuleset["Evaluate ruleset<br/>last-match-wins"]
        defaultAsk["Return Ask"]
        returnAllow["Return Allow"]
        returnAction["Return Rule Action"]
    end
    
    start --> checkAlways
    checkAlways --> matchAlways
    matchAlways -->|yes| returnAllow
    matchAlways -->|no| evalRuleset
    evalRuleset -->|match found| returnAction
    evalRuleset -->|no match| defaultAsk
```

## External Resources

- [Globset crate documentation for pattern matching](https://docs.rs/globset/latest/globset/) - Globset crate documentation for pattern matching
- [Serde serialization framework documentation](https://serde.rs/) - Serde serialization framework documentation

## Sources

- [mod](../sources/mod.md)
