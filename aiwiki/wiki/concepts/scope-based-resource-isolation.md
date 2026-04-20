---
title: "Scope-Based Resource Isolation"
type: concept
generated: "2026-04-19T18:32:32.710896290+00:00"
---

# Scope-Based Resource Isolation

### From: memory_migrate

Scope-based resource isolation constitutes an architectural pattern for partitioning system resources along contextual boundaries that reflect organizational, operational, or security domains. This pattern appears pervasively across computing infrastructure: Linux namespaces and cgroups provide process-level isolation; cloud IAM systems implement hierarchical permission scopes; package managers like npm and Cargo distinguish global, user, and project-level dependencies; and configuration systems like git support local, global, and system-wide settings. The ragent-core memory system's three-tier scope hierarchy—user, project, and global—directly instantiates this pattern, applying well-established isolation semantics to AI agent memory management.

The user scope in ragent-core captures personalization dimensions that should persist across agent invocations and project contexts: learned preferences regarding output formatting, accumulated expertise about the user's domain, conversation style adaptations, and potentially sensitive information about the user's identity and circumstances. This scope implements the principle of personalization portability, ensuring that users retain their customized agent experience regardless of which project they're currently working in. Security considerations for user scope typically include encryption at rest with user-controlled keys, strict access logging, and data retention policies aligned with privacy regulations that grant users rights over their personal data.

Project scope represents the most common operational context, capturing domain-specific knowledge tied to particular codebases, problem domains, or organizational initiatives. This scope enables agent specialization—memories about specific APIs, architectural decisions, team conventions, and project history that would be irrelevant or distracting in other contexts. The project scope default in ragent-core reflects empirical observation that most agent interactions are task-specific and benefit from focused context. Implementation challenges for project scope include handling multi-repository projects, cross-project dependencies, and the lifecycle management of project memories when repositories are archived or transferred between organizations.

Global scope provides system-wide defaults and shared knowledge bases that transcend individual users and projects, enabling consistent baseline behavior and organizational knowledge dissemination. This scope typically requires governance processes to manage content quality, prevent pollution with outdated information, and ensure appropriate licensing for shared resources. The interaction between scopes—where user preferences might override global defaults, or project conventions might take precedence over user habits—requires careful precedence rules and conflict resolution mechanisms. The `BlockScope` abstraction in ragent-core likely encapsulates these resolution semantics, presenting a unified interface that hides complexity of multi-source memory composition from consuming code.

## External Resources

- [Twelve-Factor App configuration methodology](https://12factor.net/config) - Twelve-Factor App configuration methodology
- [npm scope and configuration documentation](https://docs.npmjs.com/resolving-ewa-conflicts) - npm scope and configuration documentation
- [W3C privacy and fingerprinting guidance](https://www.w3.org/TR/fingerprinting-guidance/) - W3C privacy and fingerprinting guidance

## Sources

- [memory_migrate](../sources/memory-migrate.md)
