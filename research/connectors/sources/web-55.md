# Web source

- URL: https://github.com/Gowindude/claude-desktop-code-bridge
- Title: GitHub - Gowindude/claude-desktop-code-bridge: Bidirectional MCP bridge between Claude Desktop and Claude Code (two...
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:51.390857776+00:00
- Relevance: Medium - multiple title terms match query


```text
GitHub’s Gowindude/claude-desktop-code-bridge is a bidirectional MCP bridge where Claude Desktop writes a plan and Claude Code picks it up, implements it, streams progress, and submits a completion summary; Desktop then approves or rejects with a reason, and Code revises on rejection or finalizes on approval, with no human in the execution loop. The two sides communicate only through a shared SQLite database (`bridge.db`) and markdown files under `~/.claude-bridge/` (`plans/<project_id>.md` and `status/<project_id>_status.md`), via two stdio FastMCP servers, `desktop_server.py` and `code_server.py`, which share `db.py` and `models.py`. It requires Python 3.11+ and dependencies `mcp[cli]`, `fastmcp`, `aiosqlite`, `aiofiles`, and `pydantic>=2.0`; a dedicated venv is strongly recommended, especially on Windows. The lifecycle is `pending → in_progress → awaiting_approval → approved → complete`, with `rejected` handled by `bridge_reset_for_revision`, and the project also provides a FastAPI server at `http://localhost:7823` plus Windows scripts (`run_loop.bat`, `start_bridge.vbs`, etc.) to keep Claude Code running in the background.
```
