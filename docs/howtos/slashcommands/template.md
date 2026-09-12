# /template
> List and apply reusable prompt templates: /template [name] [args]

## Overview

`/template` lists reusable prompt templates discovered from two directories -
`~/.ragent/templates/` (personal) and `.ragent/templates/` (project) - and
applies them by name. Templates are markdown files with `{{placeholder}}`
variables such as `{{title}}`, `{{description}}`, and `{{arguments}}`.

Applying a template substitutes `{{arguments}}` with the args you pass and
then **overwrites the input buffer** with the result: any text you had already
typed into the prompt input is replaced. The applied text is shown first in a
fenced block so you can see exactly what landed in the input.

## Syntax

```
/template                     # list all templates
/template <name>              # apply a template, {{arguments}} empty
/template <name> <args>       # apply with {{arguments}} substituted
/template help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/template` | List all templates in a `Name | Description | Scope | Placeholders` table |
| `/template <name>` | Apply the template; `{{arguments}}` stays empty |
| `/template <name> <args>` | Apply with `<args>` substituted into `{{arguments}}` |
| `/template help` | Print usage help |

Unknown `<name>` prints the sorted list of available template names plus
usage.

## Examples

List available templates:

```
/template
```

Apply a review template:

```
/template review-checklist
```

Apply with arguments:

```
/template release-notes v1.0.96
```

Apply a bug-report template with structured args:

```
/template bugreport "test_loop_termination flakes" --since 2026-09-01
```

Check the usage help:

```
/template help
```

## Output

Bare form: a markdown table sorted by name with the columns
`Name | Description | Scope | Placeholders`. When no templates are found,
the output explains where to create them:

- `~/.ragent/templates/` - personal templates
- `.ragent/templates/` - project templates (higher priority)
- placeholders use the `{{name}}` form, e.g. `{{title}}`, `{{description}}`,
  `{{arguments}}`

Applied template: the rendered text is shown in a fenced block, followed by
the placeholder list and three numbered next-step options. The input buffer
is then set to the rendered text and the cursor moved to its end - anything
you had typed before running the command is replaced.

Unknown name:

```
Template '<name>' not found.
```

followed by the sorted list of available names and the usage line.

## Related

- `/reload` - rescan configuration and skills (restart picks up new template files)
- `~/.ragent/templates/` - personal template directory
- `.ragent/templates/` - project template directory (higher priority)
- `/history` - reuse previous inputs as an alternative to templates