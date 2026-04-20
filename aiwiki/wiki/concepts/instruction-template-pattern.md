---
title: "Instruction Template Pattern"
type: concept
generated: "2026-04-19T20:20:16.275789983+00:00"
---

# Instruction Template Pattern

### From: bundled

The instruction template pattern is a technique for parameterizing AI agent behavior through structured text containing variable substitution markers, enabling flexible reuse of complex operational procedures. In ragent's bundled skills, this pattern appears through body constants like `SIMPLIFY_BODY`, `BATCH_BODY`, `DEBUG_BODY`, and `LOOP_BODY`, which contain detailed step-by-step instructions with embedded `$ARGUMENTS` placeholders. These templates separate the static structure of a skill's operation from dynamic inputs, allowing the same skill definition to handle varying contexts without code modification.

The pattern's implementation in bundled.rs reveals sophisticated template design considerations. Templates include conditional logic through descriptive instructions rather than programming constructs, as in SIMPLIFY_BODY's "Check the output path above. If it contains a file path (not empty/blank), you MUST..." This approach leverages the underlying language model's reasoning capabilities while maintaining explicit procedural structure. Argument parsing instructions are embedded directly in templates, as seen in LOOP_BODY's "Parse the arguments: - If the first argument looks like a duration..." This self-contained specification reduces coupling between the skill system and execution runtime. Templates also include safety constraints, formatting requirements, and example outputs that guide consistent behavior across invocations.

The instruction template pattern offers several architectural advantages. Templates are human-readable and editable, enabling skill customization without programming expertise. They are version-controllable as plain text, facilitating review and rollback. They support prompt engineering optimization, where template refinement improves model outputs without system redeployment. The pattern enables A/B testing of instruction variants and gradual rollout of behavioral changes. However, the pattern also presents challenges: template injection vulnerabilities if arguments aren't sanitized, brittleness to model behavior changes, and difficulty in validating template correctness statically. Ragent's test suite partially addresses these through `test_bundled_skills_have_nonempty_bodies` and content assertions in individual skill tests. Future enhancements might include template linting, parameterized testing across input variations, and structured template languages that compile to optimized prompts for specific model versions.

## External Resources

- [Template processor - general concept underlying instruction templates](https://en.wikipedia.org/wiki/Template_processor) - Template processor - general concept underlying instruction templates
- [OpenAI prompt engineering guide - best practices for instruction design](https://platform.openai.com/docs/guides/prompt-engineering) - OpenAI prompt engineering guide - best practices for instruction design

## Sources

- [bundled](../sources/bundled.md)
