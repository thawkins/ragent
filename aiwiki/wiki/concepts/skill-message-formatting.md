---
title: "Skill Message Formatting"
type: concept
generated: "2026-04-19T20:22:02.786382436+00:00"
---

# Skill Message Formatting

### From: invoke

Skill message formatting implements the presentation layer between skill execution and LLM consumption, establishing clear provenance boundaries that help agent systems understand instruction sources. The format_skill_message and format_forked_result functions apply consistent structural conventions: bracketed headers identifying the skill origin [Skill: /name] or [Forked Skill Result: /name], followed by newline-separated content bodies. This formatting serves dual purposes—human readability for debugging and structured parsing for downstream processing.

The design reflects intentional semantic signaling through syntactic conventions. The leading slash in skill names (/deploy, /review) echoes command-line interface conventions, reinforcing the tool-like nature of skills within agent conversations. The bracketed header syntax creates visually distinct blocks that stand out from natural conversation flow, helping both human observers and potential parsing heuristics identify automated instruction injection. Forked results use distinct header wording ("Forked Skill Result" versus "Skill") to communicate execution mode differences that might affect result interpretation—forked execution implies isolated processing with potentially different agent capabilities or model characteristics.

Architectural evolution of skill formatting demonstrates responsiveness to operational requirements. Early implementations might have injected raw skill content without provenance headers, risking confusion about whether content represented user instructions, system prompts, or automated tooling output. The current explicit header approach supports emerging patterns in agent observability and attribution—understanding which capabilities contributed to specific agent behaviors. The multiline test cases validate that formatting preserves content structure without introducing escaping complexity, supporting skills containing code blocks, enumerated lists, or formatted documentation. The MustUse attribute on formatters prevents accidental discarding of formatted results in async code paths.

## External Resources

- [OpenAI prompt engineering best practices](https://platform.openai.com/docs/guides/prompt-engineering) - OpenAI prompt engineering best practices

## Sources

- [invoke](../sources/invoke.md)
