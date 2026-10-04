# Web source

- URL: https://hyperleap.ai/blog/connect-hyperleap-claude-desktop-mcp
- Title: Connect Hyperleap to Claude Desktop in 5 Minutes (MCP Setup Tutorial)
- Author(s): Gopi Krishna Lakkepuram
- Language: English
- Published (UTC): 2026-05-05T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:28:49.253265791+00:00
- Relevance: High - title matches query


```text
Hyperleap’s native MCP server connects Claude Desktop to live Hyperleap chatbot/CRM data via a local JSON config, an API key, and `npx @hyperleap/mcp-server`; setup takes about five minutes and requires Claude Desktop (not web), Node.js 18+, and a paid Hyperleap plan (Plus $40/mo, Pro $100/mo, Max $200/mo; not free trial by default). It exposes nine read-only tools—`list_leads`, `get_lead_details`, `get_lead_conversations`, `get_conversation`, `get_lead_activities`, `get_lead_notes`, `get_pipeline_stages`, `get_crm_dashboard`, and `extract_lead_insights`—for natural-language queries about leads, conversations, pipeline, notes, and chatbot activity from Website, WhatsApp, Instagram DM, or Facebook Messenger, with no write access or external CRM/email/ad-platform connections. Config paths are `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS and `%APPDATA%\Claude\claude_desktop_config.json` on Windows; API keys are generated in Hyperleap settings and shown once. Queries return data into Claude’s context under Anthropic policies; rate limits require waits of 30–60 seconds, and multiple workspaces need separate named `mcpServers` entries.
```
