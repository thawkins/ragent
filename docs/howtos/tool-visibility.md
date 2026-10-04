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

When a switch is `off`, tools in that family are excluded from:

1. The tool list advertised in the system prompt
2. The tool schema list sent to the provider

The tools remain registered, but the model is not told to use them. `/tools list`
prints `Visible Tools (N total, M disabled)` and then a
`Disabled by visibility (M)` section listing every hidden-but-still-registered
tool (it is the complement of the model-facing tool list, backed by
`ToolRegistry::hidden_definitions()`), so a family switched off stays visible in
the report instead of vanishing.

## Slash commands

Use these from the TUI (`/tools` on its own prints the help):

```text
/tools
/tools list
/tools help
/tools <switch>
/tools <switch> on
/tools <switch> off
```

`/tools list` (alias `/tools show`) renders the family visibility table plus every
visible and disabled tool.

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
    "browser": true
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

