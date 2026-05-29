# Release

## Current Version: 0.1.0-alpha.101

### Fixed
- fixed AGENTS.md load path

## Previous: 0.1.0-alpha.100

### Added
- **Sub-agent suspend/resume/kill lifecycle** — New `suspend_task()`, `resume_task()`, and `kill_task()` methods on the task manager. Sub-agents can now be paused, resumed, or forcibly terminated with a 10-second force-kill escalation timeout. New `TaskStatus::Suspended` and `TaskStatus::Terminating` states, plus `SubagentSuspended`, `SubagentResumed`, and `SubagentKilled` events.
- **Teammate suspend/resume events** — `TeammateSuspended` and `TeammateResumed` events for team coordination, enabling lead agents to pause and resume teammates.
- **Enhanced active-agents panel** — TUI active agents panel now shows per-agent status (running/suspended), supports suspend/resume/kill actions, and renders agent step counts and elapsed time.
- **Enhanced teams panel** — TUI teams panel shows teammate statuses, suspend/resume buttons, and per-teammate progress indicators.
- **SSE events for sub-agent lifecycle** — Server-sent events now stream `SubagentSuspended`, `SubagentResumed`, `SubagentKilled`, `TeammateSuspended`, and `TeammateResumed` event types.

### Changed
- **Permission check indentation fix** — Re-indented `check_permission_with_prompt()` to correct a long-standing indentation issue that made the function body appear nested inside an outer block.
- **AGENTS.md discovery sorting** — Improved instruction file priority sorting with properly formatted `AGENTS.md` vs `CLAUDE.md` ordering logic.
- **Azure Resource provider refactoring** — Cleaned up `azure_resource.rs` provider implementation.
- **HTTP client retry logic** — Updated `execute_with_retry` in `http_client.rs`.

## Previous: 0.1.0-alpha.99