---
title: "Permission-Category-Based Access Control"
type: concept
generated: "2026-04-19T16:16:35.539533446+00:00"
---

# Permission-Category-Based Access Control

### From: think

Permission-category-based access control is an authorization pattern that groups related capabilities under semantic namespaces to enable fine-grained security policies in AI agent systems. The permission_category method returning "think:record" in ThinkTool demonstrates this pattern, where the colon-delimited structure suggests a hierarchical namespace with "think" as the domain and "record" as the specific action. This design enables security administrators to grant broad access (all "think:*" operations) or specific restrictions (only "think:record", not "think:modify") without the tool implementation needing to interpret or enforce these policies directly.

This approach addresses unique security challenges in AI agent systems where traditional role-based or attribute-based access control may be insufficient. Agent tools often have complex, emergent security properties—a tool that reads files might be safe in isolation but dangerous when combined with a tool that executes code. Permission categories enable policy definitions that capture these semantic nuances, expressing rules like "reasoning observation is always permitted" or "reasoning modification requires additional approval." The separation between permission category and tool name allows multiple tools to share categories (multiple tools might record thoughts under "think:record") and single tools to expose multiple categories if they have distinct modes with different risk profiles.

In operational deployment, this pattern integrates with policy-as-code systems like Open Policy Agent (OPA), Cedar, or custom authorization middleware. The permission_category serves as the resource/action identifier in policy evaluation, with the surrounding framework handling the enforcement decision. The design supports principle of least privilege by default—tools without explicit permission grants are inaccessible—while enabling dynamic policy updates without code changes. For multi-tenant agent systems, this pattern extends to include tenant identifiers in policy evaluation, ensuring that reasoning data from one organization is never visible to another even when both use the same underlying ThinkTool implementation.

## External Resources

- [Open Policy Agent (OPA) for policy-as-code authorization](https://www.openpolicyagent.org/) - Open Policy Agent (OPA) for policy-as-code authorization
- [Cedar policy language and authorization engine](https://docs.rs/ CedarPolicy/latest/cedar_policy/) - Cedar policy language and authorization engine
- [NCSC guidance on shared responsibility in cloud security](https://www.ncsc.gov.uk/collection/cloud/shared-responsibility-model) - NCSC guidance on shared responsibility in cloud security

## Sources

- [think](../sources/think.md)
