# Web source

- URL: https://github.com/webmaxru/ai-native-dev
- Title: GitHub - webmaxru/ai-native-dev: Plugin marketplace for AI-native development agent skills
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:12:52.636106726+00:00
- Relevance: High - title matches query


```text
The `webmaxru/ai-native-dev` repository, maintained by Maxim Salnikov, is a plugin marketplace for AI-native development agent skills that aggregates plugins from three sources: its own `ai-native-dev-skills` plugin (Agent Package Manager, Agent Skill Deploy, ARD Registry Builder, GitHub Agentic Workflows), `web-ai-skills` from `webmaxru/web-ai-agent-skills` (Prompt API, Language Detector, Translator, Writing Assistance, Proofreader, WebMCP, WebNN), and `enonic-skills` from `webmaxru/enonic-agent-skills` (Enonic CMS skills). Installing the marketplace via `/plugin marketplace add webmaxru/ai-native-dev` in GitHub Copilot CLI or Claude Code, or through VS Code’s `chat.plugins.marketplaces` setting, gives the coding agent every plugin and skill without per-skill setup, while individual skills can also be installed via APM or `npx skills add`. A GitHub Pages catalog aggregates skill metadata from all three repositories at build time, rebuilt locally with `npm run build:website`, automatically on push to `main` via GitHub Actions, and weekly via cron; the project is MIT-licensed.
```
