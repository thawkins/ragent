---
title: "Rust Module Re-exports"
type: concept
generated: "2026-04-19T21:22:28.592000715+00:00"
---

# Rust Module Re-exports

### From: mod

Rust module re-exports are a structural pattern used to create ergonomic public APIs while maintaining internal implementation organization. In this source file, the `pub use` statements lift items from submodules (`auth` and `client`) to the parent module level, allowing consumers to write `github::GitHubClient` and `github::device_flow_login` rather than the more verbose `github::client::GitHubClient` and `github::auth::device_flow_login`. This technique, sometimes called the "facade pattern" or "barrel exports" in other language ecosystems, represents a deliberate API design choice that balances code organization with user experience.

The implementation reveals several Rust-specific conventions: the `mod` declarations introduce the submodules, while `pub use` re-exports specific items with their original visibility or greater. The re-exported items retain their documentation and type information, appearing as if defined at the re-export location. This enables internal refactoring—such as splitting functionality across multiple files or reorganizing module structure—without breaking downstream code. The pattern is prevalent throughout the Rust ecosystem, seen in foundational crates like `tokio`, `serde`, and `hyper`, where flat public APIs hide complex internal hierarchies.

Effective use of re-exports requires careful consideration of API stability and discoverability. Items with distinct domains (authentication vs. client operations) may benefit from remaining namespaced, while tightly coupled items warrant flattening. The ragent authors chose to flatten the most common operations—token management and the main client—suggesting these are the primary expected use cases. This design also facilitates documentation generation, where the re-exported items appear in the parent module's documentation with cross-links to their original definitions. Advanced patterns include `pub use submodule::*` for wholesale re-exports, conditional re-exports with `cfg` attributes, and versioned API layers where re-exports provide compatibility shims.

## Diagram

```mermaid
flowchart LR
    subgraph Internal["Internal Structure"]
        auth_mod[\"mod auth\"]
        client_mod[\"mod client\"]
        auth_fn1[\"delete_token\"]
        auth_fn2[\"device_flow_login\"]
        auth_fn3[\"load_token\"]
        auth_fn4[\"save_token\"]
        client_struct[\"GitHubClient\"]
    end
    
    subgraph PublicAPI["Public API Surface"]
        github_mod[\"github module\"]
        pub_auth[\"pub use auth::*\"]
        pub_client[\"pub use client::GitHubClient\"]
    end
    
    auth_mod --> auth_fn1 & auth_fn2 & auth_fn3 & auth_fn4
    client_mod --> client_struct
    
    auth_fn1 & auth_fn2 & auth_fn3 & auth_fn4 --> pub_auth
    client_struct --> pub_client
    
    pub_auth --> github_mod
    pub_client --> github_mod
    
    style github_mod fill:#90EE90
    style PublicAPI fill:#E6F3FF
```

## External Resources

- [Rust Reference: Use declarations](https://doc.rust-lang.org/reference/items/use-declarations.html) - Rust Reference: Use declarations
- [The Rust Book: Separating Modules into Different Files](https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html) - The Rust Book: Separating Modules into Different Files

## Sources

- [mod](../sources/mod.md)
