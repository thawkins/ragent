---
title: "Semantic Versioning"
type: concept
generated: "2026-04-19T14:54:44.382132517+00:00"
---

# Semantic Versioning

### From: ref:AGENTS

Semantic Versioning (SemVer) is a versioning scheme that communicates the nature of changes in software releases through a structured version number format of major.minor.patch-prerelease. The guidelines explicitly mandate SemVer for all project versioning, with particular attention to the prerelease component used during development. The specification requires that version numbers during active development carry the -alpha suffix, which is only removed when preparing production releases. This practice allows clear distinction between development snapshots and stable releases while maintaining compatibility with SemVer 2.0.0 specification.

The major version increment signals breaking changes that require consumer code modification, the minor version adds functionality in a backward-compatible manner, and the patch version indicates backward-compatible bug fixes. The prerelease component (-alpha in this context) precedes the stable release and indicates work-in-progress status. The guidelines' strict rules about version bump commit messages—prohibiting "chore: bump version to ..." in favor of "Version: " prefix—reflect organizational conventions for release traceability. This structured approach to versioning enables automated tooling to determine compatibility ranges and upgrade safety.

The integration with changelog management creates a comprehensive release documentation system. The RELEASE.md file captures the current version and latest changelog entry for GitHub Releases page presentation, while CHANGELOG.md maintains the full history following Keep a Changelog format. The alpha designation practice prevents premature stability commitments while allowing continuous integration and distribution of development builds. This versioning discipline is particularly critical in Rust's ecosystem where Cargo's dependency resolution relies on SemVer for compatible version selection.

## External Resources

- [Semantic Versioning 2.0.0 specification](https://semver.org/) - Semantic Versioning 2.0.0 specification
- [Keep a Changelog standard for changelog maintenance](https://keepachangelog.com/) - Keep a Changelog standard for changelog maintenance

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
