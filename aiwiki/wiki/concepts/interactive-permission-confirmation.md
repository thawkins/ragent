---
title: "Interactive Permission Confirmation"
type: concept
generated: "2026-04-19T15:24:51.865093027+00:00"
---

# Interactive Permission Confirmation

### From: mod

Interactive permission confirmation is a security pattern that defers authorization decisions to human users when automated policy evaluation yields ambiguous or restrictive results. In the ragent-core system, this concept is materialized through the `PermissionAction::Ask` variant and the `PermissionRequest`/`PermissionDecision` structs. When the `PermissionChecker::check()` method encounters a request that matches no explicit `Allow` or `Deny` rules, it returns `Ask`, triggering a workflow where the operation is suspended and a `PermissionRequest` is constructed containing contextual metadata including the permission type, target patterns, session identifier, and optional tool-call provenance. This request can be presented to a user interface for human evaluation, with the response captured as a `PermissionDecision` enum variant (`Once`, `Always`, or `Deny`). The `Once` variant enables conservative security postures where single-instance approvals don't create persistent vulnerabilities, while `Always` enables efficiency for recurring legitimate operations. This pattern addresses the fundamental tension between AI agent autonomy and security: agents require broad capabilities to be useful, but unconstrained access creates unacceptable risk. Interactive confirmation provides a middle path where capabilities are available but exercised only with human oversight for sensitive operations.

## External Resources

- [Authorization concepts in computer security](https://en.wikipedia.org/wiki/Authorization) - Authorization concepts in computer security
- [OWASP Access Control guidelines](https://owasp.org/www-community/Access_Control) - OWASP Access Control guidelines

## Sources

- [mod](../sources/mod.md)
