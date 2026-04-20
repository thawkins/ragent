---
title: "Permission-Based Security Models"
type: concept
generated: "2026-04-19T16:48:21.431362293+00:00"
---

# Permission-Based Security Models

### From: http_request

Permission-based security models in agent systems classify operations by risk category to enable policy-driven access control, illustrated by HttpRequestTool's `permission_category()` returning "network:fetch". This categorization permits security infrastructure to grant or deny tool execution based on capability requirements rather than identity-based permissions alone. The hierarchical namespace convention (`network:` prefix with `fetch` subcategory) supports fine-grained policies distinguishing network operations by risk level or data sensitivity.

The specific categorization "network:fetch" distinguishes this tool from potentially more sensitive categories like "network:modify" (state-changing operations) or "network:internal" (intranet access). This distinction enables policy expressions such as "agents may fetch public data but not POST to external APIs" or "development agents may access internal services while production agents are restricted to approved domains". The category serves as metadata for policy engines that may operate at various system boundaries: LLM sampling-time tool selection, execution sandbox configuration, or audit logging categorization.

Implementation of this model requires coordination across agent system components: tool registration must capture and store category metadata, policy configuration must map categories to allow/deny decisions or conditional requirements (authentication, approval workflows), and execution infrastructure must enforce these decisions before `execute()` invocation. HttpRequestTool's role in this ecosystem is providing accurate self-categorization; the tool itself does not enforce policies, trusting external infrastructure for consistent application. This separation of concerns permits centralized policy management across diverse tools while respecting tool-specific risk assessments from their implementers. The "network:fetch" category specifically acknowledges the tool's read-only, external-facing nature relative to more sensitive capabilities like file system modification or code execution.

## External Resources

- [W3C Permissions API - web platform permission model](https://www.w3.org/TR/permissions/) - W3C Permissions API - web platform permission model
- [Kubernetes RBAC - role-based access control patterns](https://kubernetes.io/docs/concepts/security/rbac-good-practices/) - Kubernetes RBAC - role-based access control patterns

## Related

- [Structured Tool Interfaces](structured-tool-interfaces.md)
- [Resource Safety Limits](resource-safety-limits.md)

## Sources

- [http_request](../sources/http-request.md)
