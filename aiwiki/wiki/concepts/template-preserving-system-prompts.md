---
title: "Template-Preserving System Prompts"
type: concept
generated: "2026-04-19T15:00:25.379300429+00:00"
---

# Template-Preserving System Prompts

### From: custom

Template-preserving system prompts defer variable substitution until invocation time rather than resolving at load time, enabling dynamic context injection based on runtime conditions. This approach distinguishes between the static agent personality (loaded from disk) and dynamic execution context (available only during conversation). The implementation stores raw template strings with placeholders like `{{variable}}` intact through the validation and storage pipeline, with substitution occurring in the separate `build_system_prompt()` phase. This supports powerful use cases: agents can reference conversation-specific information (current date, user identity, project context), include dynamic tool descriptions generated from available skills, and adapt behavior based on runtime configuration without reloading definitions. The validation still checks template length against limits, ensuring the post-substitution result will likely remain within bounds. This design pattern appears in template engines like Handlebars, Jinja, and Liquid, adapted for AI system prompts where the template variables carry semantic meaning for the underlying language model. The separation of concerns enables agent definition reuse across different contexts while maintaining clear boundaries between configuration and state.

## External Resources

- [Handlebars.js templating engine documentation](https://handlebarsjs.com/) - Handlebars.js templating engine documentation
- [Jinja2 Python templating engine, widely used in AI frameworks](https://jinja.palletsprojects.com/) - Jinja2 Python templating engine, widely used in AI frameworks

## Sources

- [custom](../sources/custom.md)
