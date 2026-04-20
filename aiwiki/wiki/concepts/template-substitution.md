---
title: "Template Substitution"
type: concept
generated: "2026-04-19T20:17:24.139367322+00:00"
---

# Template Substitution

### From: args

Template substitution is a fundamental text processing pattern where placeholder variables within a template are replaced with actual values at runtime. In the context of this ragent module, substitution operates on skill bodies—markdown documents that define agent behaviors—allowing dynamic content based on invocation arguments and execution context. The implementation demonstrates several advanced considerations: ordering of replacements to prevent partial match conflicts, multiple syntaxes for accessing the same data ($ARGUMENTS[N] vs $N), and graceful handling of missing values through empty string substitution. This pattern appears widely across computing, from shell script variable expansion to web template engines like Jinja2 or Handlebars, though this implementation is intentionally lightweight and specialized for the skill execution domain.

The substitution system supports five distinct variable types, each serving different use cases within skill definitions. The `$ARGUMENTS` variable provides access to the complete raw argument string, preserving original spacing and quoting for cases where the skill needs to forward arguments to subprocesses. Indexed access through `$ARGUMENTS[N]` enables precise selection of specific arguments by position, with 0-based indexing following programming language conventions. The `$N` shorthand offers a more concise syntax for the same functionality, reducing verbosity in skill bodies that reference many positional arguments. Environment-style variables `${RAGENT_SESSION_ID}` and `${RAGENT_SKILL_DIR}` provide execution context, enabling skills to generate unique identifiers, construct paths relative to their definition location, or implement session-aware behaviors. This design reflects a careful balance between simplicity and expressiveness, providing sufficient flexibility for diverse skill implementations without the complexity of a full expression language.

## External Resources

- [String interpolation - Wikipedia article on the general pattern](https://en.wikipedia.org/wiki/String_interpolation) - String interpolation - Wikipedia article on the general pattern
- [Handlebars.js - popular templating engine with similar substitution concepts](https://handlebarsjs.com/) - Handlebars.js - popular templating engine with similar substitution concepts
- [Jinja2 - Python templating engine with advanced substitution features](https://jinja.palletsprojects.com/) - Jinja2 - Python templating engine with advanced substitution features

## Sources

- [args](../sources/args.md)
