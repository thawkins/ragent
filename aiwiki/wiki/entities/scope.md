---
title: "Scope"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:30:23.528642880+00:00"
---

# Scope

**Type:** technology

### From: bash_lists

Scope is an enumeration that defines the persistence target for configuration mutations in ragent's bash policy system, distinguishing between project-local and global user settings. The enum has two variants: Project, which resolves to `./ragent.json` in the current working directory, and Global, which resolves to `~/.config/ragent/ragent.json` using the platform-appropriate configuration directory. This scoping mechanism enables flexible policy management where teams can share baseline security rules through version-controlled project configs while individuals maintain personal overrides or additions in their global config.

The implementation includes a config_path method that performs the filesystem path resolution, using the dirs crate for cross-platform global config directory detection. This design pattern of separating semantic scope from concrete paths improves testability and would allow future extensions like workspace-level or repository-level scopes. The Scope type appears in all mutation APIs (add_allowlist, remove_allowlist, etc.), making the persistence target an explicit, required decision for every policy change. This prevents accidental modifications to the wrong configuration file and supports commands like `/bash add --global curl` for intentional global policy updates.

## Diagram

```mermaid
flowchart LR
    subgraph ScopeEnum["Scope Enum"]
        Project["Project variant"]
        Global["Global variant"]
    end
    
    subgraph Paths["Resolved Paths"]
        ProjectPath["./ragent.json"]
        GlobalPath["~/.config/ragent/ragent.json"]
    end
    
    subgraph Usage["Mutation APIs"]
        AddAllow["add_allowlist(entry, scope)"]
        RemoveAllow["remove_allowlist(entry, scope)"]
        AddDeny["add_denylist(pattern, scope)"]
        RemoveDeny["remove_denylist(pattern, scope)"]
    end
    
    Project -->|config_path| ProjectPath
    Global -->|config_path| GlobalPath
    ProjectPath --> AddAllow
    ProjectPath --> RemoveAllow
    ProjectPath --> AddDeny
    ProjectPath --> RemoveDeny
    GlobalPath --> AddAllow
    GlobalPath --> RemoveAllow
    GlobalPath --> AddDeny
    GlobalPath --> RemoveDeny
    
    style ScopeEnum fill:#e8f5e9
    style Paths fill:#fff3e0
```

## External Resources

- [dirs crate documentation for platform-appropriate configuration directory detection](https://docs.rs/dirs/latest/dirs/) - dirs crate documentation for platform-appropriate configuration directory detection
- [Rust enum documentation - the language feature used for Scope definition](https://doc.rust-lang.org/rust-by-example/custom_types/enum.html) - Rust enum documentation - the language feature used for Scope definition

## Sources

- [bash_lists](../sources/bash-lists.md)
