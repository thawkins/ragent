---
title: "Agent Guidelines for Rust Development Projects"
source: "ref:AGENTS"
type: source
tags: [rust, agent-guidelines, development-workflow, ci-cd, testing, code-quality, documentation, versioning, semantic-versioning, cargo, team-collaboration]
generated: "2026-04-19T14:54:44.379861932+00:00"
---

# Agent Guidelines for Rust Development Projects

This document establishes comprehensive operational guidelines for AI agents working on Rust-based software projects. It covers the entire development lifecycle including build commands, testing procedures, code quality standards, documentation requirements, and team collaboration workflows. The guidelines emphasize strict adherence to Rust ecosystem best practices, with specific mandates for test organization, error handling using modern Rust crates like anyhow and thiserror, and structured logging with the tracing crate rather than println statements. The document also defines critical workflow rules such as the prohibition against pushing to remote repositories without explicit authorization, proper changelog maintenance following Keep a Changelog format, and semantic versioning practices with alpha designations for development releases.

A significant portion of the guidelines addresses testing infrastructure, mandating that all tests reside in dedicated tests/ directories rather than inline within source files, with support for both synchronous and asynchronous testing patterns. The document establishes timeout parameters for builds (600 seconds) and test execution (10 minutes), along with platform-specific commands for Unix/Linux environments. Code style specifications include 4-space indentation, 100-character line width limits, and organized import grouping. The guidelines extend to dimensional unit conventions requiring internal representation in millimeters as f32 types with 2 decimal precision, and datetime handling in UTC with locale translation deferred to the UI layer.

The document also outlines team-based workflows for parallel code review and analysis, specifying the use of team creation with blueprints, mandatory waiting for teammate completion, and result aggregation procedures. Additional operational concerns include temporary file management through a target/temp directory structure, priority classification for issues ranging from critical security concerns to backlog items, and strict prohibitions against temporary fixes or premature issue resolution declarations. The guidelines reference external resources including Rust best practices documentation and the Keep a Changelog specification, while maintaining specific file location rules for documentation markdown files.

## Related

### Entities

- [Cargo](../entities/cargo.md) — technology
- [Clippy](../entities/clippy.md) — technology
- [Tracing](../entities/tracing.md) — technology
- [Anyhow](../entities/anyhow.md) — technology
- [Thiserror](../entities/thiserror.md) — technology

### Concepts

- [Semantic Versioning](../concepts/semantic-versioning.md)
- [Test Organization](../concepts/test-organization.md)
- [Error Handling Patterns](../concepts/error-handling-patterns.md)
- [Documentation Standards](../concepts/documentation-standards.md)
- [Team Collaboration Workflow](../concepts/team-collaboration-workflow.md)
- [Dimensional Unit Conventions](../concepts/dimensional-unit-conventions.md)

