---
title: "Hierarchical Configuration Precedence"
type: concept
generated: "2026-04-19T21:17:40.434085132+00:00"
---

# Hierarchical Configuration Precedence

### From: store

Hierarchical configuration precedence is a design pattern where configuration sources are organized in layers of increasing specificity, with more specific configurations overriding more general ones. In the RAgent team store implementation, this pattern manifests as a two-tier hierarchy: project-local teams (stored in `[PROJECT]/.ragent/teams/`) take precedence over user-global teams (stored in `~/.ragent/teams/`). This architecture enables flexible team management where developers can maintain project-specific agent configurations while retaining access to personal global teams, with the system automatically resolving name collisions in favor of project-local definitions.

The precedence implementation appears throughout the module's discovery functions. The `find_team_dir` function explicitly implements this logic by first checking the project-local location and only falling back to global directories when no match is found. Similarly, `list_teams` enumerates project-local teams first, tracking seen names in a `HashSet` to filter duplicates when subsequently scanning the global directory. This ordering ensures that project-local teams appear in results and that global teams with conflicting names are effectively shadowed. The deduplication strategy using `seen.insert()` followed by `seen.contains()` checks provides O(1) membership testing while preserving the priority semantics.

This pattern offers significant practical benefits for multi-agent development workflows. Teams associated with specific codebases can be version-controlled alongside project files, enabling reproducible agent configurations across development environments. Meanwhile, personal global teams remain available for ad-hoc tasks and experimentation. The precedence system also supports configuration inheritance patterns where a project might extend or override specific aspects of a globally defined team, though the current implementation uses simple replacement rather than structural merging.

## External Resources

- [Twelve-Factor App configuration methodology](https://12factor.net/config) - Twelve-Factor App configuration methodology
- [CSS specificity as an analogy for configuration precedence](https://en.wikipedia.org/wiki/Cascading_Style_Sheets#Specificity) - CSS specificity as an analogy for configuration precedence

## Related

- [Directory Discovery Algorithms](directory-discovery-algorithms.md)

## Sources

- [store](../sources/store.md)
