---
title: "Safety-First System Design"
type: concept
generated: "2026-04-19T22:10:19.614488482+00:00"
---

# Safety-First System Design

### From: resource

Safety-first system design is an architectural philosophy that prioritizes the elimination of entire categories of failures through construction, rather than relying on operational mitigations. This codebase exemplifies this approach through its explicit rejection of unsafe code, documented in the architecture decision record explaining why setrlimit was not used. The workspace-level #![deny(unsafe_code)] attribute is a contractual guarantee that no module, including dependencies, can invoke undefined behavior through raw pointer manipulation or FFI. This constraint shapes all technical decisions: when unsafe would be the conventional solution, engineers must find safe alternatives or accept reduced functionality. The benefits include eliminated memory safety vulnerabilities, simplified auditing and formal verification, and reduced cognitive load during development. The cost is accepting potentially less efficient or less capable solutions—here, the inability to set hard OS resource limits. This trade-off reflects growing industry recognition that many systems' security requirements outweigh performance optimizations, particularly in agent systems with autonomous capability.

## External Resources

- [The Rustonomicon - unsafe Rust guidelines](https://doc.rust-lang.org/nomicon/) - The Rustonomicon - unsafe Rust guidelines
- [Rust safety-critical code community initiatives](https://github.com/rust-secure-code/safety-dance) - Rust safety-critical code community initiatives

## Related

- [Application-Level Resource Limits](application-level-resource-limits.md)
- [Fork Bomb Prevention](fork-bomb-prevention.md)

## Sources

- [resource](../sources/resource.md)
