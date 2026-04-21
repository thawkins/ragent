# ragent-prompt_opt

Template-only prompt optimization helpers for ragent.

This crate provides a TemplateOptimizer implementing an Optimizer trait plus several
convenience functions that format user input into common prompt structures such as
CO-STAR, CRISPE, Chain-of-Thought, DRAW, RISE, VARI, Q* and adapters for platform
styling (OpenAI, Claude, Microsoft). These are template transformers only and do not
call external APIs.
