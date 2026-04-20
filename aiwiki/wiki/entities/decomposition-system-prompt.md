---
title: "DECOMPOSITION_SYSTEM_PROMPT"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:08:22.348240925+00:00"
---

# DECOMPOSITION_SYSTEM_PROMPT

**Type:** technology

### From: swarm

DECOMPOSITION_SYSTEM_PROMPT is a meticulously engineered constant containing the system-level instructions that shape LLM behavior during the task decomposition phase. This prompt represents significant domain expertise in multi-agent system design, encoding principles for effective parallelization while respecting practical constraints of distributed AI agent architectures. The prompt establishes the LLM's role as a "task decomposition engine for a multi-agent coding system," setting appropriate expectations for output format and quality.

The instruction set emphasizes independence as the primary decomposition criterion, requiring that subtasks be self-contained with agents able to complete work without visibility into other agents' outputs unless explicitly declared via dependencies. This constraint reflects the architectural reality that each agent operates within isolated context windows, fundamentally different from shared-memory or shared-state programming models. The guidance to minimize dependencies and prefer parallelizable tasks directly optimizes for wall-clock execution time in distributed deployments.

Practical constraints are enforced through specific numeric bounds and format requirements. The 2-8 subtask range prevents both under-decomposition (defeating parallelism) and over-decomposition (coordination overhead exceeding parallel gains). Detailed description requirements ensure agent executability without clarification cycles. The JSON schema specification with field optionality documentation enables backward-compatible evolution while maintaining strict parsing reliability. These design choices reflect operational experience with production LLM systems where output consistency cannot be assumed.

## Sources

- [swarm](../sources/swarm.md)
