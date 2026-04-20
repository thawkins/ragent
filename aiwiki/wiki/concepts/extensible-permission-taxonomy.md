---
title: "Extensible Permission Taxonomy"
type: concept
generated: "2026-04-19T15:24:51.865448925+00:00"
---

# Extensible Permission Taxonomy

### From: mod

An extensible permission taxonomy is a design pattern that provides comprehensive coverage of common operation types while maintaining adaptability for domain-specific requirements. The ragent-core `Permission` enum implements this pattern through its nine standard variants plus a `Custom(String)` catch-all variant. The standard variants address universal AI agent needs: `Read` and `Edit` for file manipulation, `Bash` for shell execution, `Web` for network access, `Question` for user interaction, `PlanEnter`/`PlanExit` for structured reasoning workflows, `Todo` for task management, `ExternalDirectory` for sandbox escape detection, and `DoomLoop` for infinite loop prevention. These carefully selected variants reflect operational experience with AI agent failure modes and security concerns. The `Custom` variant uses a String payload to support arbitrary permission names, enabling developers to define domain-specific permissions (such as `deploy`, `provision`, or `notify`) without forking or modifying the core library. This design exemplifies the open/closed principle: the enum is open for extension through `Custom` while closed for modification of standard variants. The string-based `From<&str>` implementation ensures seamless integration, automatically promoting unrecognized permission names to `Custom` variants. This approach balances type safety for common operations with flexibility for specialized domains, critical for a foundational library intended for diverse agent applications.

## External Resources

- [Open/Closed Principle in software design](https://en.wikipedia.org/wiki/Open%E2%80%93closed_principle) - Open/Closed Principle in software design
- [Rust enums and variant types](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html) - Rust enums and variant types

## Sources

- [mod](../sources/mod.md)
