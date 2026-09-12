# /github

> GitHub integration: /github login | logout | status | help

## Overview

`/github` manages GitHub authentication for the TUI session.

Token sources:

- The `GITHUB_TOKEN` environment variable
- The stored token file `~/.ragent/github_token`

`/github login` runs the OAuth device flow.

## Syntax

```
/github
/github login
/github logout
/github status
/github help
```

## Options

| Form | Description |
| ---- | ----------- |
| `/github` | bare form, no subcommand |
| `/github login` | start the OAuth device flow |
| `/github logout` | remove stored credentials |
| `/github status` | report the current auth state |
| `/github help` | print command help |

## The login device flow

1. The command shows a user code in the message window.
2. A pending dialog opens.
3. The flow polls in the background until authorisation completes or the
   attempt times out.
4. On timeout the dialog reports:

```
Device flow timed out - please try /github login again.
```

`logout` removes stored credentials. `status` reports the current auth
state.

## Examples

Bare form:

```
/github
```

Start the device flow:

```
/github login
```

Check the auth state:

```
/github status
```

Remove stored credentials:

```
/github logout
```

## Output

- `login` shows the user code and opens the pending dialog; completion and
  failure arrive via the dialog.
- `status` prints whether a token is loaded.

## Notes

- The stored token lives in `~/.ragent/github_token`; `logout` removes
  stored credentials.
- `/reverse` fetches repos through the GitHub API.

## Related

- `/gitlab` - the GitLab counterpart
- `/reverse` - uses the GitHub API