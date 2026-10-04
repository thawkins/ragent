# Web source

- URL: https://dudarik.com/en/blog/role-specific-plugins
- Title: Role-Specific Plugins for Codex CLI: Templates for Domain-Specific Agents
- Author(s): -
- Language: English
- Published (UTC): 2026-08-17T09:05:52+00:00
- Captured (UTC): 2026-10-02T13:28:00.673882620+00:00
- Relevance: Medium - multiple title terms match query


```text
OpenAI’s role-specific-plugins repository for Codex CLI provides standardized, reusable templates that package domain-specific skills, connector bindings, and starter configurations to reduce manual prompt engineering, MCP setup, and context injection. It ships six role-specific knowledge-work plugins—including sales/, data-analytics/, product-design/, financial-markets/, and Creative Production—and supports connections to 62 popular applications with over 110 capabilities under the MIT License; each plugin uses a layout with .codex-plugin/plugin.json, .app.json, .mcp.json, skills/, assets/, and README.md. The CLI loads these components so agents can run role-specific workflows such as sales meeting prep, data exploration/reporting, product specs/UI critiques, public-equity research/valuation, and marketing content creation. Limitations include manual connector/credential configuration (e.g., REPLACE_WITH_SALESFORCE_APP_OR_CONNECTOR_ID), scope limited to knowledge work rather than DevOps/CI-CD/general software engineering, template skills requiring adaptation, a still-maturing ecosystem (369 stars), no built-in compliance guardrails, and performance dependent on connector configuration, network latency, and Codex CLI version.
```
