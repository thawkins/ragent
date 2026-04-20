---
title: "Tool Name Hallucination"
type: concept
generated: "2026-04-19T17:09:53.938709547+00:00"
---

# Tool Name Hallucination

### From: aliases

Tool name hallucination is a phenomenon in large language model behavior where models generate tool names that were not explicitly provided in their context, based on semantic extrapolation from task descriptions and patterns learned during training. In agent systems, this manifests as models emitting calls to `read_file` or `view_file` when only a `read` tool was defined, or `run_shell_command` when only `bash` was available. This behavior stems from LLMs' training on diverse coding assistant interactions where multiple naming conventions exist, creating a distribution over plausible tool names for given operations.

The hallucination problem is particularly acute for coding agents because the space of plausible tool names is large and semantically driven. A model tasked with "show me the contents of main.rs" might reasonably suggest `open_file`, `view_file`, `read_file`, `cat`, or `get_file_contents` based on various programming contexts it has encountered. Without explicit constraints, models will sample from this distribution, often choosing names that differ from the specific implementation the runtime expects. This creates friction in agent systems where strict name matching would cause frequent failures.

Ragent's aliases module represents a defensive engineering approach to this challenge: rather than attempting to eliminate hallucination through better prompting (which has limited effectiveness) or model retraining (which is expensive and slow), the system accepts hallucination as an inherent property of LLM behavior and builds adaptation mechanisms. This mirrors broader trends in robust AI system design where alignment between human intent and model output is achieved through system-level mediation rather than assuming perfect model behavior. The approach scales with model capability rather than fighting against it, accommodating increasingly sophisticated agents that might invent reasonable-sounding tool names for novel operations.

## External Resources

- [Research on tool learning and hallucination in LLMs](https://arxiv.org/abs/2302.00093) - Research on tool learning and hallucination in LLMs
- [Anthropic research on AI alignment and robustness](https://www.anthropic.com/research) - Anthropic research on AI alignment and robustness

## Sources

- [aliases](../sources/aliases.md)
