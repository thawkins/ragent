# /reverse

> Reverse-engineer a GitHub repo: /reverse <owner/repo | URL> [--tech <stack>] [--create <name>]

## Overview

`/reverse` fetches a public repo's metadata, root file tree, and README via
the GitHub API, then asks the currently selected LLM model to generate a
synthetic creation prompt that could have produced that repo.

The generated prompt is useful as a seed for scaffolding a similar project:
copy it into the message window, feed it to another command, or chain it
straight into spec creation with the `--create <name>` option.

Synthesis runs on the currently selected model, so switch to the model you
want before running the command.

## Syntax

```
/reverse <owner/repo | URL>
/reverse <owner/repo | URL> --tech <stack>
/reverse <owner/repo | URL> --tech <stack> --create <name>
```

## Options

| Form | Description |
| ---- | ----------- |
| `/reverse <owner/repo>` | owner/repo shorthand, e.g. `ratatui/ratatui` |
| `/reverse <URL>` | full GitHub URL, e.g. `https://github.com/ratatui/ratatui` |
| `--tech <stack>` | constrain the synthetic prompt to the named technology stack |
| `--create <name>` | chain into `/spec create`, auto-generating a spec named `<name>` from the reverse-engineered prompt |

Both identifier forms are accepted by the same command; pick whichever you
have at hand.

## Examples

Plain owner/repo shorthand:

```
/reverse ratatui/ratatui
```

Full GitHub URL:

```
/reverse https://github.com/ratatui/ratatui
```

Constrain the synthetic prompt to a Rust stack:

```
/reverse tokio-rs/axum --tech rust
```

Constrain the stack and chain straight into spec creation:

```
/reverse tokio-rs/axum --tech rust --create config-svc
```

## Output

The message window shows repo fetch progress first, then the generated
prompt. With `--create <name>`, the spec creation output follows the prompt.

## Workflow

1. Fetch - the GitHub API supplies the repo's metadata, root file tree, and
   README.
2. Synthesise - the currently selected LLM model turns that material into a
   synthetic creation prompt that could have produced the repo.
3. Chain (optional) - `--create <name>` hands the prompt to `/spec create`,
   which auto-generates a spec named `<name>`.

## Notes

- `--tech <stack>` only steers synthesis; it does not change what is
  fetched.
- `--create <name>` names the spec produced by the chained `/spec create`
  run.
- The fetch step uses the GitHub API, so the target repo must be public.

## Related

- `/spec create` - the command `--create` chains into
- `/new` - scaffold a project directly