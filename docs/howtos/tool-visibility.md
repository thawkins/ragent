# Tool Visibility

`ragent` can hide or expose whole tool families through persistent visibility
switches. This keeps the model's advertised tool list focused without removing
the tools from the binary.

## Switches

The available switches are:

| Switch | Default | Family |
|--------|---------|--------|
| `office` | `off` | Office/PDF tools |
| `github` | `off` | GitHub issue and PR tools |
| `gitlab` | `off` | GitLab issue, MR, pipeline, and job tools |
| `teams` | `off` | Team coordination tools |
| `agents` | `off` | Autonomous agent task tools |
| `plan` | `off` | Plan-mode tools |
| `codeindex` | `on` | Code index tools |
| `masterfetch` | `on` | MasterFetch web-access tools (`mf_fetch`, `mf_search`, `mf_crawl`, `mf_screenshot`, `mf_cache_clear`, `mf_version`) |
| `browser` | `on` | Browser automation tool (`browser` — Chrome DevTools Protocol) |
| `finance` | `on` | Finance/stock/currency tools (`stock_quote`, `stock_history`, `stock_fundamentals`, `stock_search`, `stock_options`, `stock_recommendations`, `currency_rate`, `currency_history`) |

When a switch is `off`, tools in that family are excluded from:

1. The tool list advertised in the system prompt
2. The tool schema list sent to the provider

The tools remain registered, but the model is not told to use them.

## Slash commands

Use `/tools` from the TUI:

```text
/tools
/tools help
/tools <switch>
/tools <switch> on
/tools <switch> off
```

Examples:

```text
/tools github on
/tools office on
/tools teams off
/tools agents off
/tools plan off
/tools codeindex off
/tools masterfetch off
/tools browser off
/tools finance off
```

Changes are written to `.ragent/ragent.json` when a project config directory is
present, or to the user config otherwise.

## Config file

You can also configure visibility directly in `ragent.json`:

```json
{
  "tool_visibility": {
    "office": true,
    "github": false,
    "gitlab": false,
    "teams": false,
    "agents": false,
    "plan": false,
    "codeindex": true,
    "masterfetch": true,
    "browser": true,
    "finance": true
  }
}
```

## Interaction with `hidden_tools`

`hidden_tools` is still supported. It remains additive:

- `tool_visibility` hides whole families
- `hidden_tools` hides specific tool names

If either mechanism hides a tool, that tool stays out of the advertised tool
set.

## Code index compatibility

`/codeindex on` and `/codeindex off` also keep `tool_visibility.codeindex` in
sync, so the code index command and the tool visibility command do not drift.

## Finance visibility note

The `finance` switch was added in v1.0.45. It controls the visibility of all
`stock_*` and `currency_*` tools. When set to `off`, the tools are hidden from
the model's advertised tool list but remain registered and callable. The
finance tools are visible by default (`on`). Note that the `/tools` TUI command
does not currently include `finance` in its interactive switch list, but the
switch can be set via `ragent.json` as shown above.
