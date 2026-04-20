---
title: "Permission-Based Tool Access Control"
type: concept
generated: "2026-04-19T18:28:57.178639168+00:00"
---

# Permission-Based Tool Access Control

### From: lsp_symbols

Permission-based access control in tool frameworks establishes security boundaries around potentially sensitive operations, preventing unauthorized or unintended use of powerful capabilities. The `LspSymbolsTool` declares its `permission_category` as `"lsp:read"`, indicating that it performs read-only operations on LSP-managed resources. This categorization enables policy-driven access control where system administrators or user configurations can grant or deny tool categories based on trust levels and use cases.

The design of permission categories reflects the principle of least privilege and defense in depth. While LSP symbol reading is relatively low-risk compared to file modification or code execution, categorizing it separately allows fine-grained policies. An agent might be permitted `fs:read` for direct file access but denied `lsp:read` if LSP servers are considered external dependencies that should not be invoked. Conversely, `lsp:read` might be permitted while `lsp:write` (for refactoring operations) is denied, even though both use the same underlying infrastructure.

The colon-separated category syntax (`lsp:read`) suggests a namespacing scheme that could accommodate hierarchical permissions and wildcards. A policy granting `lsp:*` would encompass all LSP operations, while `lsp:read` specifically targets read-only queries. This design pattern appears in many security systems, from OAuth scopes to Kubernetes RBAC, providing both expressiveness and readability. The implementation in `LspSymbolsTool` is declarative—the tool merely reports its category, leaving enforcement to the framework's policy engine.

The security model for AI agent tools requires careful consideration of confused deputy problems, where an agent with legitimate access might be tricked into performing unauthorized actions on behalf of a malicious user. Permission categories are one layer of defense, but additional protections might include path traversal validation, rate limiting on LSP queries, and audit logging of tool invocations. The `LspSymbolsTool` implementation includes some implicit protections through rigorous path canonicalization and validation before LSP server contact, preventing directory traversal attacks that might exploit relative path manipulation.

## External Resources

- [Confused deputy security problem](https://en.wikipedia.org/wiki/Confused_deputy_problem) - Confused deputy security problem
- [OAuth scope-based access control](https://oauth.net/2/scope/) - OAuth scope-based access control

## Sources

- [lsp_symbols](../sources/lsp-symbols.md)
