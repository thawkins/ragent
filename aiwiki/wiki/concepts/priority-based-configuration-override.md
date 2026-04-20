---
title: "Priority-Based Configuration Override"
type: concept
generated: "2026-04-19T20:20:16.274962185+00:00"
---

# Priority-Based Configuration Override

### From: bundled

Priority-based override is a configuration management pattern that enables hierarchical customization of system behavior, where settings at higher precedence levels supersede those at lower levels while maintaining fallback defaults. This pattern is ubiquitous in software tooling, exemplified by Git's configuration hierarchy (system → global → local), Docker's context settings, and CSS's cascade specificity rules. Ragent adapts this pattern for skill management through its SkillScope enumeration, creating a three-tier system: Bundled skills provide universal defaults, Personal skills encode user preferences across all projects, and Project skills capture team-specific conventions within a repository.

The mechanics of priority-based override in ragent involve scope ordering and name-based resolution. When the skill registry encounters multiple definitions with identical names, it compares their scopes using a defined precedence order—Project highest, Personal intermediate, Bundled lowest—and selects the highest-priority implementation. This simple rule enables powerful customization scenarios. A developer might override the bundled `simplify` skill to use different git history depth or preferred linting tools. A team might define a project-specific `batch` skill that includes additional verification steps required by their CI/CD pipeline. The override mechanism is complete replacement rather than merge, ensuring predictable behavior without complex conflict resolution rules.

The pattern offers significant advantages for maintainability and user experience. Defaults ensure immediate functionality for new users while enabling progressive customization as needs evolve. The hierarchical structure mirrors organizational boundaries—individual preferences, team standards, organizational defaults—supporting natural governance models. Debugging and introspection are facilitated by explicit scope tracking; ragent can report which skill implementations are active and why. The pattern also supports experimental workflows, where users can trial personal skill overrides before proposing them as project standards. Implementation considerations include clear scope documentation, tooling support for discovering active overrides, and migration paths when bundled skills evolve. Ragent's test suite validates this behavior through `test_bundled_skills_scope`, ensuring the resolution logic remains correct as the system evolves.

## External Resources

- [Git configuration file documentation - exemplifies priority-based override pattern](https://git-scm.com/docs/git-config#_configuration_file) - Git configuration file documentation - exemplifies priority-based override pattern
- [CSS specificity - another well-known priority-based override system](https://en.wikipedia.org/wiki/Cascading_Style_Sheets) - CSS specificity - another well-known priority-based override system

## Sources

- [bundled](../sources/bundled.md)
