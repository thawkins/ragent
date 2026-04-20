---
title: "Runtime Permission Grants"
type: concept
generated: "2026-04-19T15:24:51.864757652+00:00"
---

# Runtime Permission Grants

### From: mod

Runtime permission grants represent a dynamic authorization mechanism that allows persistent permission exceptions to be established during system operation, complementing static configuration-based policies. In the ragent-core system, this concept is implemented through the `PermissionChecker::record_always()` method and the associated `always_grants` HashMap field. When a user interactively approves a permission request with the `Always` decision variant, the system can invoke `record_always()` to permanently authorize the specific permission-pattern combination for subsequent checks. These runtime grants are stored as pre-compiled `globset::GlobMatcher` instances, organized by permission type in a HashMap for O(1) lookup followed by efficient pattern matching. This architecture enables a seamless user experience where repeated permission prompts for the same operation are eliminated after initial approval, while maintaining security through explicit user consent. The precedence design ensures runtime grants are evaluated before static rulesets, creating a hierarchy where user-established permissions override configured policies. This pattern is common in modern operating systems and applications, such as macOS's persistent accessibility permissions or browser site-specific permissions, adapted here for AI agent operation contexts.

## External Resources

- [macOS accessibility permissions model](https://support.apple.com/guide/mac-help/allow-accessibility-apps-mh43185/mac) - macOS accessibility permissions model
- [Web Permissions API for runtime grants](https://developer.mozilla.org/en-US/docs/Web/API/Permissions_API) - Web Permissions API for runtime grants

## Sources

- [mod](../sources/mod.md)
