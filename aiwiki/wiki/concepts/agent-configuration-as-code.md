---
title: "Agent Configuration as Code"
type: concept
generated: "2026-04-19T14:58:16.762090312+00:00"
---

# Agent Configuration as Code

### From: oasf

Agent Configuration as Code (ACaC) represents a paradigm shift from programmatic agent construction to declarative, version-controlled specification of agent behavior and capabilities. The OASF implementation in ragent exemplifies this approach by storing agent definitions as JSON files in a discovery directory (`.ragent/agents/`), enabling Git-based version control, code review workflows, and collaborative development practices previously associated with traditional software artifacts. This methodology treats agent prompts, permission rules, and execution parameters as first-class configuration entities rather than hardcoded implementation details.

The benefits of this approach extend across the development lifecycle. During development, declarative configuration allows rapid iteration on agent behavior without recompilation, as changes to JSON files are picked up on the next agent invocation. For deployment, the file-based structure supports environment-specific overlays and configuration management practices from infrastructure-as-code disciplines. The example in the source documentation illustrates a minimal security-focused code reviewer agent defined entirely through JSON configuration, demonstrating how complex agent behaviors can be specified without writing executable code.

This pattern also enables powerful composition and reuse mechanisms. Agents can reference shared skill definitions through the `skills` vector, inherit global configuration for parameters like model selection, and be organized hierarchically through the filesystem structure. The templating system in `system_prompt` with variables like `{{WORKING_DIR}}` and `{{DATE}}` bridges static configuration with dynamic runtime context, preserving the declarative nature while accommodating environment-specific values.

## External Resources

- [Discussion of Agent Configuration as Code patterns](https://www.mattiasgees.be/2023/12/agent-configuration-as-code/) - Discussion of Agent Configuration as Code patterns

## Sources

- [oasf](../sources/oasf.md)
