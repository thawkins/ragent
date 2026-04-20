---
title: "Directory Discovery Algorithms"
type: concept
generated: "2026-04-19T21:17:40.435045893+00:00"
---

# Directory Discovery Algorithms

### From: store

Directory discovery algorithms in the RAgent team store implement filesystem traversal patterns for locating configuration directories relative to execution context. The primary implementation, `find_project_teams_dir`, employs a bottom-up ancestor traversal that walks from a starting working directory toward the filesystem root, testing each directory for the presence of a `.ragent/` marker subdirectory. This pattern, common in version control systems like Git (which searches for `.git` directories), enables context-sensitive configuration discovery where the appropriate settings depend on the location from which a command is executed.

The algorithm's correctness depends on several implementation details that handle edge cases in filesystem semantics. The loop structure uses `current.parent()` to ascend the directory tree, with explicit `None` handling to terminate at the root. The `is_dir()` check on candidate paths prevents false positives from files named `.ragent` that might exist in unrelated directories. The use of `PathBuf` for result construction ensures proper path separator handling across Windows and Unix-like platforms, while the `Option` return type distinguishes successful discovery from the absence of project-local configuration.

This discovery mechanism enables sophisticated workflow patterns in multi-project development environments. Developers can nest projects within other projects, with the nearest ancestor `.ragent/` taking precedence, or maintain separate agent configurations for different branches of a monorepo by placing `.ragent/` at strategic directory levels. The algorithm's O(depth) complexity is bounded by typical filesystem depths and executes entirely in memory without external dependencies. Integration with `find_team_dir` and `TeamStore::list_teams` creates a cohesive discovery system where teams can be referenced by simple names that resolve to appropriate storage locations based on execution context, abstracting away filesystem organization details from higher-level application logic.

## External Resources

- [Git worktree documentation showing similar discovery patterns](https://git-scm.com/docs/git-worktree) - Git worktree documentation showing similar discovery patterns
- [Rust Path::parent() documentation for directory traversal](https://doc.rust-lang.org/std/path/struct.Path.html#method.parent) - Rust Path::parent() documentation for directory traversal

## Related

- [Hierarchical Configuration Precedence](hierarchical-configuration-precedence.md)

## Sources

- [store](../sources/store.md)
