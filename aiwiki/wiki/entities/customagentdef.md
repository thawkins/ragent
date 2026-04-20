---
title: "CustomAgentDef"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:00:25.375651903+00:00"
---

# CustomAgentDef

**Type:** technology

### From: custom

CustomAgentDef is the central data structure representing a successfully loaded and validated custom agent definition within the ragent framework. This struct encapsulates four critical pieces of information: the raw OASF (Open Agent Schema Format) record as parsed from disk, the absolute file path indicating the source location, the resolved AgentInfo structure ready for runtime use by the session processor, and a boolean flag distinguishing between project-local and user-global scope. The struct derives Debug and Clone traits, enabling straightforward logging and duplication of agent definitions throughout the system. The source_path field provides essential provenance tracking, allowing the system to report meaningful error context and enabling developers to locate the original configuration file when debugging agent behavior. The is_project_local boolean implements the priority override mechanism where project-local agent definitions automatically supersede global ones with identical names, facilitating project-specific customization without affecting the user's global agent collection. This architecture supports collaborative development scenarios where team members can share project-specific agents through version control while maintaining personal agent libraries in their home directories.

## External Resources

- [Serde serialization framework used for JSON deserialization in agent loading](https://serde.rs/) - Serde serialization framework used for JSON deserialization in agent loading
- [Rust standard library PathBuf documentation for filesystem path handling](https://doc.rust-lang.org/std/path/struct.PathBuf.html) - Rust standard library PathBuf documentation for filesystem path handling

## Sources

- [custom](../sources/custom.md)
