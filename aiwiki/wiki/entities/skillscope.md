---
title: "SkillScope"
entity_type: "technology"
type: entity
generated: "2026-04-19T20:20:16.274172794+00:00"
---

# SkillScope

**Type:** technology

### From: bundled

SkillScope is an enumeration type that implements ragent's priority-based skill resolution system, enabling a sophisticated override mechanism for skill definitions. The scope hierarchy consists of three levels: Bundled (built-in skills shipping with ragent), Personal (user-defined skills in home directory), and Project (workspace-specific skills). When skills share the same name, higher-priority scopes override lower ones, allowing customization without modifying core functionality. This design pattern, similar to Git's configuration levels or CSS specificity, provides flexibility while maintaining predictability.

The Bundled scope represents the foundation layer, containing skills that are always available and serve as sensible defaults for common development tasks. These skills are immutable from the user's perspective but can be completely replaced by defining identically-named skills in Personal or Project scopes. The Personal scope enables user preferences to persist across all projects, while Project scope allows team-specific conventions and workflows to be encoded in version-controlled skill definitions. This scoping system encourages skill sharing and reuse while empowering customization at appropriate granularity levels.

SkillScope is integrated throughout ragent's skill loading and resolution pipeline. When the system encounters multiple skill definitions with the same identifier, it uses scope priority to determine which implementation to activate. The enumeration is stored in SkillInfo and used for validation, debugging, and UI presentation. This architecture supports advanced use cases like skill inheritance, where a Project skill might extend a Bundled skill, and skill shadowing diagnostics, where the system can report when overrides are active. The explicit scope model also facilitates skill marketplace and package management scenarios, where provenance and trust boundaries are critical concerns.

## Sources

- [bundled](../sources/bundled.md)
