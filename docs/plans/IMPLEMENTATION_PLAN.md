# ragent Implementation Plan
**Document Type:** Implementation Roadmap  
**Created:** April 1, 2026  
**Owner:** swarm-s6  
**Status:** Draft  
**Based On:** GAP_ANALYSIS_AND_RECOMMENDATIONS.md (swarm-s5)

---

## Executive Summary

This implementation plan transforms the competitive gap analysis into an actionable development roadmap. The plan organizes 40+ recommended features into **5 major milestones** spanning **12-18 months**, with a focus on delivering quick wins early while building toward transformative capabilities.

### Strategic Priorities
1. **Quick Wins First** (Milestone 1): High-impact, low-complexity features to demonstrate momentum
2. **Competitive Parity** (Milestones 2-3): Close critical gaps with ClaudeCode, OpenCode, Copilot CLI
3. **Unique Advantages** (Milestone 4): Strengthen ragent's differentiators (performance, multi-provider, teams)
4. **Ecosystem Integration** (Milestone 5): MCP protocol, extensibility, enterprise features

### Timeline Overview
- **Milestone 1** (Quick Wins): 6-8 weeks
- **Milestone 2** (Autonomous Operation): 8-10 weeks
- **Milestone 3** (Project Intelligence): 8-10 weeks
- **Milestone 4** (Advanced Features): 10-12 weeks
- **Milestone 5** (Ecosystem & Polish): 12-16 weeks
- **Total**: 44-56 weeks (approx. 12-14 months for core features)

---

## Milestone 1: Quick Wins & Critical Parity
**Duration:** 6-8 weeks  
**Goal:** Deliver high-impact, low-complexity features to close immediate gaps and build momentum  
**Success Metrics:**
- Cost transparency visible to 100% of users
- Permission prompts reduced by 40%
- Git context available in all sessions
- ThinkTool improves agent reasoning quality by 20% (measured by fewer errors)

### Task 1.1: Cost Tracking System
**Priority:** P0 (Critical)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Implement comprehensive token counting and cost tracking across all providers.

**Acceptance Criteria:**
- [ ] Token counting for all message exchanges (input + output)
- [ ] USD cost calculation using provider-specific pricing tables
- [ ] `/cost` command shows:
  - Total tokens (input/output)
  - Total USD cost
  - Cost breakdown by provider (if multi-provider used)
  - Session duration
- [ ] Real-time cost display in TUI status bar (optional)
- [ ] Session cost logged to history/analytics file
- [ ] Support for all providers: Anthropic, OpenAI, Ollama, Azure, etc.

**Technical Approach:**
1. Create `TokenTracker` struct to accumulate counts per message
2. Add pricing table as `HashMap<ProviderModel, Pricing>` in config
3. Update each provider client to return token counts in response
4. Create `/cost` command handler
5. Add cost summary to session cleanup

**Risks:**
- Pricing tables may become outdated (mitigation: auto-update from public APIs if available)
- Streaming models may estimate tokens instead of exact counts (mitigation: accept estimates)

---

### Task 1.2: Git Context Auto-Injection
**Priority:** P0 (Critical)  
**Effort:** 3 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Automatically inject git repository context into system prompts to improve agent awareness.

**Acceptance Criteria:**
- [ ] On session start, run 5 git commands in parallel:
  - `git rev-parse --abbrev-ref HEAD` (current branch)
  - `git symbolic-ref refs/remotes/origin/HEAD` (main branch)
  - `git status --short` (modified files)
  - `git log --oneline -n5` (recent commits)
  - `git shortlog -sn --all --no-merges` (author stats, top 5)
- [ ] Inject results into system prompt context section
- [ ] Truncate total git context to max 200 lines
- [ ] Cache results, invalidate on `/clear` command
- [ ] Gracefully handle non-git directories (skip injection)
- [ ] Add `--no-git-context` flag to disable

**Technical Approach:**
1. Create `GitContextProvider` module
2. Use `tokio::spawn` for parallel command execution (max 1 second timeout each)
3. Parse and format output into structured context string
4. Inject into `SystemPrompt` builder
5. Add memoization cache with session lifetime

**Risks:**
- Large repositories may have slow git commands (mitigation: timeout at 1 second)
- Private repositories may expose sensitive commit messages (mitigation: document behavior, allow disabling)

---

### Task 1.3: README Auto-Injection
**Priority:** P1 (High)  
**Effort:** 2 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Automatically read and inject README.md into context for project awareness.

**Acceptance Criteria:**
- [ ] Check for `README.md` in working directory on session start
- [ ] If found, read file content
- [ ] Truncate to 5000 characters if larger
- [ ] Inject into system prompt context
- [ ] Cache with session lifetime
- [ ] Add `--no-readme` flag to disable

**Technical Approach:**
1. Add `README.md` check to `ContextProvider`
2. Read file asynchronously
3. Truncate intelligently (preserve sections if possible)
4. Add to system prompt after AGENTS.md

**Risks:**
- Very large READMEs consume tokens (mitigation: strict 5000 char limit)

---

### Task 1.4: Safe Command Whitelist
**Priority:** P1 (High)  
**Effort:** 3 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Implement whitelist of safe read-only commands that bypass permission prompts.

**Acceptance Criteria:**
- [ ] Define default whitelist in config:
  - Git: `git status`, `git diff`, `git log`, `git branch`, `git show`
  - Info: `pwd`, `ls`, `tree`, `date`, `which`, `whoami`, `uname`
  - Read: `cat`, `head`, `tail`, `less`, `more`, `wc`
- [ ] Whitelist commands skip permission prompt
- [ ] Commands with arguments still validated (e.g., `cat /etc/passwd` blocked if outside project)
- [ ] User can extend whitelist in `ragent.json`
- [ ] Audit log still records whitelisted commands

**Technical Approach:**
1. Add `safe_commands: Vec<String>` to config
2. Update permission system to check whitelist before prompting
3. Apply argument validation (no directory escapes)
4. Log all commands regardless of whitelist status

**Risks:**
- False sense of security if whitelist too broad (mitigation: conservative defaults)
- Users may add unsafe commands (mitigation: documentation warnings)

---

### Task 1.5: ThinkTool for Agent Reasoning
**Priority:** P1 (High)  
**Effort:** 2 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Add internal-only tool for agents to perform explicit reasoning before acting.

**Acceptance Criteria:**
- [ ] Create `think` tool visible to agent
- [ ] Accepts `thoughts: string` parameter
- [ ] Not displayed to user in UI (logged only)
- [ ] Returns success acknowledgment
- [ ] Logged to debug log with timestamp
- [ ] Optional: track think frequency per session

**Technical Approach:**
1. Add `ThinkTool` to tool registry
2. Tool function just logs and returns success
3. Update agent system prompt to encourage use before complex actions
4. Add debug log entry with category `agent_reasoning`

**Risks:**
- Agents may overuse (wasting tokens) or underuse (mitigation: prompt engineering)

---

### Milestone 1 Testing & Integration
**Effort:** 3 days  
**Tasks:**
- [ ] Integration tests for cost tracking across providers
- [ ] Test git context with various repo states (clean, dirty, detached HEAD)
- [ ] Test README truncation logic
- [ ] Test whitelist with edge cases (arguments, pipes, redirects)
- [ ] Test ThinkTool in agent workflows

---

## Milestone 2: Autonomous Operation
**Duration:** 8-10 weeks  
**Goal:** Enable agents to work autonomously with minimal user intervention  
**Success Metrics:**
- 70% of tasks complete without human interruption
- Autopilot mode handles 20-iteration workflows successfully
- Plan approval reduces wasted work by 50%

### Task 2.1: Autopilot Mode
**Priority:** P0 (Critical)  
**Effort:** 10 days  
**Assignee:** TBD  
**Dependencies:** Task 1.5 (ThinkTool helps autopilot quality)

**Description:**
Implement continuous execution mode where agent loops until task completion or error.

**Acceptance Criteria:**
- [ ] Add `--autopilot` CLI flag
- [ ] Add `/autopilot on|off` command to toggle mid-session
- [ ] Loop execution until:
  - Agent explicitly calls completion tool
  - Error occurs (tool failure, API error)
  - Max iterations reached (configurable, default 20)
  - User interrupts (Ctrl+C)
- [ ] Display iteration count in UI: `[Autopilot 5/20]`
- [ ] Show iteration results after each cycle
- [ ] Add `max_autopilot_iterations` to config
- [ ] Add `autopilot_delay_ms` for rate limiting (default 500ms)
- [ ] Proper signal handling for graceful shutdown

**Technical Approach:**
1. Create `AutopilotController` module
2. Wrap agent message loop in iteration counter
3. Add completion detection logic (parse agent responses for completion signals)
4. Handle Ctrl+C with tokio signal handlers
5. Update TUI to show autopilot status

**Risks:**
- Infinite loops consuming API quota (mitigation: strict max iterations, cost monitoring)
- Agents hallucinating completion (mitigation: explicit completion tool required)
- Hard to debug failures mid-autopilot (mitigation: detailed logging per iteration)

---

### Task 2.2: Plan Mode with Approval Checkpoint
**Priority:** P1 (High)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** Task 2.3 (Mode Switching)

**Description:**
Enhance plan agent with structured workflow requiring user approval before implementation.

**Acceptance Criteria:**
- [ ] Add `/plan` command to enter plan mode
- [ ] Plan agent workflow:
  1. Ask clarifying questions
  2. Generate structured plan (markdown format)
  3. Present plan to user
  4. Wait for approval: `/approve`, `/reject [feedback]`, `/revise [changes]`
  5. On approval, switch to `coder` agent for implementation
- [ ] Plan template includes:
  - Objectives
  - Approach
  - Tasks breakdown
  - Risks & assumptions
  - Success criteria
- [ ] Plans saved to session history
- [ ] User can reference plan during implementation

**Technical Approach:**
1. Update `plan` agent system prompt with approval workflow
2. Add approval commands to command registry
3. Create plan state machine: `Planning → Review → Approved|Rejected`
4. Integrate with mode switching (Task 2.3)
5. Store plan in session context for later reference

**Risks:**
- Users skip plan approval (mitigation: make approval mandatory in plan mode)
- Plans too vague or too detailed (mitigation: prompt engineering, examples)

---

### Task 2.3: Agent Mode Switching
**Priority:** P1 (High)  
**Effort:** 8 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Enable switching between agent modes mid-session while preserving context.

**Acceptance Criteria:**
- [ ] Add `/mode <agent_name>` command
- [ ] Available modes:
  - `general` (default)
  - `plan` (read-only analysis)
  - `coder` (implementation)
  - `debug` (troubleshooting)
  - `reviewer` (code review)
- [ ] Read-only mode (`plan`) restricts tools:
  - Allow: `read`, `grep`, `glob`, `list`, `bash` (read-only commands only)
  - Block: `write`, `edit`, `create`, `rm`, `team_*`
- [ ] Mode switch preserves message history
- [ ] System prompt updated on switch
- [ ] Display current mode in TUI status bar
- [ ] Mode-specific tool access controls

**Technical Approach:**
1. Create `ModeManager` module
2. Define mode configurations with tool access lists
3. Update tool dispatcher to check current mode
4. Add mode switch handler to session controller
5. Update system prompt injection based on mode
6. Add mode indicator to UI

**Risks:**
- Confusion about which tools available in each mode (mitigation: clear documentation, `/help` shows mode-specific tools)
- Agents request unavailable tools (mitigation: system prompt explains restrictions)

---

### Task 2.4: Completion Detection Tool
**Priority:** P1 (High)  
**Effort:** 2 days  
**Assignee:** TBD  
**Dependencies:** Task 2.1 (Autopilot Mode)

**Description:**
Add explicit tool for agents to signal task completion (required for autopilot).

**Acceptance Criteria:**
- [ ] Create `task_complete` tool
- [ ] Parameters:
  - `summary: string` (what was accomplished)
  - `status: enum(success, partial, failed)`
  - `artifacts: Vec<string>` (files created/modified)
- [ ] Calling tool exits autopilot loop gracefully
- [ ] Summary displayed to user
- [ ] Logged to session history

**Technical Approach:**
1. Add `TaskCompleteTool` to registry
2. Set autopilot exit flag on tool invocation
3. Format completion summary for display
4. Update agent prompt to use this tool explicitly

**Risks:**
- Agents forget to call tool (mitigation: prompt engineering, examples)

---

### Milestone 2 Testing & Integration
**Effort:** 5 days  
**Tasks:**
- [ ] Test autopilot with 20+ iteration workflows
- [ ] Test plan approval workflow end-to-end
- [ ] Test mode switching with tool access restrictions
- [ ] Test completion tool in various scenarios
- [ ] Integration test: plan → approve → autopilot implementation

---

## Milestone 3: Project Intelligence
**Duration:** 8-10 weeks  
**Goal:** Enable agents to learn and remember project-specific context across sessions  
**Success Metrics:**
- 80% reduction in repeated explanations across sessions
- Context loading time < 200ms
- AGENTS.md adoption in 60% of projects

### Task 3.1: Hierarchical AGENTS.md Discovery
**Priority:** P0 (Critical)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** Task 3.5 (Ripgrep Integration)

**Description:**
Discover and list all AGENTS.md files throughout project directory tree.

**Acceptance Criteria:**
- [ ] Use ripgrep to find all `AGENTS.md` files recursively
- [ ] Search paths:
  - Working directory and subdirectories
  - `.agents/` directories
  - `docs/` directories
- [ ] List discovered file paths in system prompt (not full content)
- [ ] Load root `AGENTS.md` fully (if exists)
- [ ] Other files loaded on-demand by agent (via `read` tool)
- [ ] Cache discovery results per session
- [ ] Invalidate cache on `/refresh` or `/clear`
- [ ] Add `--no-agents-discovery` flag to disable

**Technical Approach:**
1. Create `AgentsDiscovery` module using `ripgrep` crate
2. Run discovery async on session start
3. Format discovered paths as bullet list in context
4. Load root file content inline
5. Memoize results in session state

**Risks:**
- Large codebases with many AGENTS.md files (mitigation: limit to first 50 files)
- Slow discovery in huge repos (mitigation: 2 second timeout)

---

### Task 3.2: Memory Write Tool
**Priority:** P0 (Critical)  
**Effort:** 4 days  
**Assignee:** TBD  
**Dependencies:** Task 3.1 (AGENTS.md Discovery)

**Description:**
Enable agents to persist learnings by appending to AGENTS.md.

**Acceptance Criteria:**
- [ ] Create `memory_write` tool
- [ ] Parameters:
  - `content: string` (what to remember)
  - `category: enum(build, test, style, architecture, notes)`
  - `scope: enum(project, directory)` (default: directory)
- [ ] Asks user permission before writing
- [ ] Appends to `AGENTS.md` in current working directory
- [ ] Creates file with template if doesn't exist
- [ ] Formats content with timestamp and category
- [ ] Logs memory writes to session history

**Technical Approach:**
1. Add `MemoryWriteTool` to registry
2. Create AGENTS.md template with sections
3. Append content under appropriate section
4. Add permission prompt before writing
5. Update agent prompt to encourage using tool for learnings

**Risks:**
- Agents overuse and bloat AGENTS.md (mitigation: permission prompts, encourage conciseness)
- Memory conflicts across multiple sessions (mitigation: file locking, append-only)

---

### Task 3.3: `/init` Command for Project Analysis
**Priority:** P1 (High)  
**Effort:** 8 days  
**Assignee:** TBD  
**Dependencies:** Task 3.2 (Memory Write Tool)

**Description:**
Analyze project structure and generate comprehensive AGENTS.md automatically.

**Acceptance Criteria:**
- [ ] `/init` command triggers project analysis
- [ ] Analysis includes:
  - Project structure (directory tree)
  - Build system detection (Cargo.toml, package.json, Makefile, etc.)
  - Test framework detection
  - Language and framework identification
  - Coding style from existing files
  - Common patterns and conventions
- [ ] Generates AGENTS.md with sections:
  - Project overview
  - Build commands
  - Test commands
  - Code style guidelines
  - Architecture notes
  - Common tasks
- [ ] Shows preview to user before writing
- [ ] User can edit before saving
- [ ] Optionally analyze subdirectories separately

**Technical Approach:**
1. Create `ProjectAnalyzer` module
2. Use pattern matching to detect build systems
3. Sample code files for style analysis
4. Use LLM to generate summary and recommendations
5. Format as structured AGENTS.md
6. Present for user approval before writing

**Risks:**
- Analysis may be inaccurate for unusual projects (mitigation: user review required)
- Large projects may take long to analyze (mitigation: progressive analysis, timeout)

---

### Task 3.4: Context Memoization System
**Priority:** P1 (High)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** Task 1.2 (Git Context), Task 3.1 (AGENTS.md Discovery)

**Description:**
Cache expensive context operations to reduce latency on every message.

**Acceptance Criteria:**
- [ ] Memoize results for:
  - Git context (status, log, branch)
  - AGENTS.md discovery
  - Directory tree
  - README.md content
- [ ] Cache lifetime: session duration
- [ ] Invalidate on:
  - `/clear` command
  - `/refresh` command
  - Explicit file write operations
- [ ] Cache size limits to prevent memory bloat
- [ ] Metrics: cache hit rate logged

**Technical Approach:**
1. Create `ContextCache` with TTL and size limits
2. Use `Arc<RwLock<HashMap>>` for thread-safe caching
3. Integrate into `ContextProvider`
4. Add cache stats to `/status` command
5. Implement cache eviction (LRU)

**Risks:**
- Stale cache causes incorrect context (mitigation: invalidation on writes)
- Memory growth in long sessions (mitigation: size limits, LRU eviction)

---

### Task 3.5: Ripgrep Integration
**Priority:** P2 (Medium)  
**Effort:** 3 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Replace grep tool with ripgrep for 10-100x faster code searches.

**Acceptance Criteria:**
- [ ] Detect ripgrep binary (`rg`) on system
- [ ] Use ripgrep if available, fallback to grep if not
- [ ] Support ripgrep features:
  - Respect .gitignore
  - Smart case matching
  - Multi-line search
  - JSON output for structured parsing
- [ ] Update `grep` tool to use ripgrep backend
- [ ] Add ripgrep to recommended dependencies

**Technical Approach:**
1. Check for `rg` binary in PATH on startup
2. Update `GrepTool` to dispatch to ripgrep or grep
3. Parse ripgrep JSON output for better results
4. Document ripgrep installation in README

**Risks:**
- Users without ripgrep get degraded performance (mitigation: clear installation instructions)

---

### Milestone 3 Testing & Integration
**Effort:** 5 days  
**Tasks:**
- [ ] Test AGENTS.md discovery with nested projects
- [ ] Test memory write with concurrent sessions
- [ ] Test `/init` on various project types (Rust, Python, Node, Go)
- [ ] Test context memoization and invalidation
- [ ] Benchmark ripgrep vs grep performance

---

## Milestone 4: Advanced Features & Differentiation
**Duration:** 10-12 weeks  
**Goal:** Implement features that differentiate ragent from competitors  
**Success Metrics:**
- Persistent shell enables Python/Node workflows
- GitHub integration completes full dev workflow
- Advanced security prevents 100% of common attack vectors

### Task 4.1: Persistent Shell Implementation
**Priority:** P0 (Critical)  
**Effort:** 15 days  
**Assignee:** TBD  
**Dependencies:** Task 1.4 (Safe Command Whitelist for safety)

**Description:**
Maintain single shell process per session to preserve environment state.

**Acceptance Criteria:**
- [ ] Spawn persistent shell (bash/zsh/powershell) on session start
- [ ] Use PTY for interactive command support
- [ ] Preserve state between commands:
  - Environment variables
  - Working directory
  - Virtual environments (Python venv, Node nvm)
  - Shell functions and aliases
- [ ] Capture command output to temp files (handle large outputs)
- [ ] Command queuing to serialize execution
- [ ] Timeout protection per command (default 300s, configurable)
- [ ] Graceful cleanup on session end
- [ ] Support Ctrl+C to interrupt running command
- [ ] Display current working directory in status bar
- [ ] Display active virtual environment in status bar

**Technical Approach:**
1. Use `portable-pty` crate for cross-platform PTY
2. Create `PersistentShell` wrapper around PTY
3. Implement command execution:
   - Write command to PTY stdin
   - Wait for prompt pattern (configurable)
   - Read output from PTY stdout
4. Add command queue with async mutex
5. Track CWD by parsing `pwd` output after each command
6. Track environment by parsing `env` output periodically
7. Handle shell-specific quirks (bash vs zsh vs powershell)

**Risks:**
- PTY handling complexity (mitigation: extensive testing, fallback to non-persistent)
- Command output parsing unreliable (mitigation: unique prompt markers)
- Zombie shell processes (mitigation: proper cleanup, heartbeat checks)
- Windows compatibility issues (mitigation: separate Windows shell handler)

---

### Task 4.2: Shell State Display
**Priority:** P2 (Medium)  
**Effort:** 2 days  
**Assignee:** TBD  
**Dependencies:** Task 4.1 (Persistent Shell)

**Description:**
Show shell state information in UI status bar.

**Acceptance Criteria:**
- [ ] Display current working directory
- [ ] Display active virtual environment (venv, conda, nvm, etc.)
- [ ] Display key environment variables (configurable list)
- [ ] Update in real-time after command execution
- [ ] Color coding for different states

**Technical Approach:**
1. Parse shell state from persistent shell output
2. Update TUI status bar after each command
3. Add config for which env vars to display

**Risks:**
- None significant

---

### Task 4.3: GitHub OAuth & API Client
**Priority:** P1 (High)  
**Effort:** 8 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Implement GitHub authentication and API client for repository operations.

**Acceptance Criteria:**
- [ ] OAuth device flow authentication
- [ ] Store credentials securely (OS keyring)
- [ ] Token refresh logic
- [ ] API client supports:
  - Repository info
  - Issues (list, create, read, update, close)
  - Pull requests (list, create, read, comment, merge)
  - Files (read, write, commit)
- [ ] Rate limiting handling
- [ ] Error handling for API failures
- [ ] `/gh login` and `/gh logout` commands

**Technical Approach:**
1. Use `octocrab` crate for GitHub API
2. Implement device flow with user code display
3. Store token in OS keyring using `keyring` crate
4. Create `GitHubClient` wrapper
5. Add commands for authentication

**Risks:**
- Token security (mitigation: OS keyring, encrypted storage)
- Rate limiting (mitigation: respect headers, queue requests)

---

### Task 4.4: GitHub Issue Management
**Priority:** P2 (Medium)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** Task 4.3 (GitHub API Client)

**Description:**
Enable full issue workflow from terminal.

**Acceptance Criteria:**
- [ ] Tools/commands:
  - `github_issue_create(title, body, labels)` → issue URL
  - `github_issue_list(state, labels, assignee)` → issue list
  - `github_issue_read(number)` → issue details
  - `github_issue_comment(number, body)` → comment URL
  - `github_issue_close(number)` → success
- [ ] Support issue templates
- [ ] Support label autocomplete
- [ ] Support assignee autocomplete
- [ ] Format output in readable markdown

**Technical Approach:**
1. Add GitHub tools to tool registry
2. Use `GitHubClient` from Task 4.3
3. Format responses for agent consumption
4. Add user-facing commands as wrappers

**Risks:**
- None significant

---

### Task 4.5: GitHub PR Management
**Priority:** P2 (Medium)  
**Effort:** 8 days  
**Assignee:** TBD  
**Dependencies:** Task 4.3 (GitHub API Client)

**Description:**
Enable full PR workflow from terminal.

**Acceptance Criteria:**
- [ ] Tools/commands:
  - `github_pr_create(title, body, base, head)` → PR URL
  - `github_pr_list(state, base)` → PR list
  - `github_pr_read(number)` → PR details + diff
  - `github_pr_comment(number, body)` → comment URL
  - `github_pr_review(number, body, event)` → review URL
  - `github_pr_merge(number, method)` → success
- [ ] Support PR templates
- [ ] Support reviewer assignment
- [ ] Support merge methods (merge, squash, rebase)
- [ ] Display diff in readable format

**Technical Approach:**
1. Add GitHub PR tools to registry
2. Fetch and format PR diffs
3. Implement merge conflict detection
4. Add user-facing commands

**Risks:**
- Large diffs may consume many tokens (mitigation: truncation, summary)

---

### Task 4.6: Advanced Security Features
**Priority:** P1 (High)  
**Effort:** 6 days  
**Assignee:** TBD  
**Dependencies:** Task 1.4 (Safe Command Whitelist)

**Description:**
Implement comprehensive security protections against unsafe operations.

**Acceptance Criteria:**
- [ ] **Banned Command List:**
  - Block: `curl`, `wget`, `nc`, `telnet`, `ssh`, `scp`, `rsync`
  - Block: `chrome`, `firefox`, `safari`, `lynx`, `w3m`
  - Block: `rm -rf /`, `dd`, `mkfs`, format commands
  - Configurable in `ragent.json`
- [ ] **Directory Escape Guard:**
  - Prevent `cd /` and escapes from project root
  - Configurable boundary (default: git root or working dir)
  - Override with explicit permission per command
- [ ] **Command Prefix Permissions:**
  - Allow granting permission by prefix: `git` → all git commands
  - Store prefix grants in session state
  - Add `/allow git` command
- [ ] **Permission Memory:**
  - Remember approved commands for session
  - Optionally persist to project config
- [ ] Audit log for all security decisions

**Technical Approach:**
1. Create `SecurityManager` module
2. Load banned commands from config
3. Add directory boundary checking
4. Implement prefix permission matching
5. Integrate with existing permission system
6. Add audit logging

**Risks:**
- False positives blocking legitimate commands (mitigation: clear override mechanism)
- Users disabling security (mitigation: documentation warnings, audit logs)

---

### Milestone 4 Testing & Integration
**Effort:** 7 days  
**Tasks:**
- [ ] Test persistent shell with Python venv, Node nvm, Ruby rbenv
- [ ] Test shell state across directory changes
- [ ] Test GitHub OAuth flow
- [ ] Test GitHub issue/PR workflows end-to-end
- [ ] Security testing: attempt banned commands, directory escapes
- [ ] Integration test: PR creation → review → merge workflow

---

## Milestone 5: Ecosystem Integration & Polish
**Duration:** 12-16 weeks  
**Goal:** Complete MCP integration, extensibility system, and enterprise features  
**Success Metrics:**
- 90% of popular MCP servers supported
- Skills system adopted by power users
- Auto-update reduces manual maintenance

### Task 5.1: Complete MCP Client Implementation
**Priority:** P0 (Critical)  
**Effort:** 15 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Implement full Model Context Protocol support for ecosystem compatibility.

**Acceptance Criteria:**
- [ ] Support all MCP transport types:
  - stdio (local process)
  - SSE (Server-Sent Events)
  - HTTP (REST endpoints)
- [ ] Server discovery and connection
- [ ] Dynamic tool registration from MCP servers
- [ ] Tool invocation with parameter mapping
- [ ] Resource management (files, data sources)
- [ ] Prompt template integration
- [ ] Error handling and retries
- [ ] Server health monitoring
- [ ] Configuration in `ragent.json`:
  ```json
  "mcp_servers": [
    {
      "name": "filesystem",
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/alice"]
    }
  ]
  ```
- [ ] `/mcp list` command to show connected servers
- [ ] `/mcp reload` to reconnect servers

**Technical Approach:**
1. Implement MCP protocol types and serialization
2. Create transport layer (stdio, SSE, HTTP)
3. Implement server lifecycle management
4. Dynamic tool registration system
5. Map MCP tools to ragent tool interface
6. Add configuration parsing
7. Create MCP commands

**Risks:**
- MCP spec changes (mitigation: follow official SDK patterns)
- Server instability affecting ragent (mitigation: isolation, timeouts)
- Complex authentication flows (mitigation: defer OAuth to Task 5.2)

---

### Task 5.2: MCP OAuth Support
**Priority:** P2 (Medium)  
**Effort:** 6 days  
**Assignee:** TBD  
**Dependencies:** Task 5.1 (MCP Client)

**Description:**
Support OAuth authentication for MCP servers requiring credentials.

**Acceptance Criteria:**
- [ ] Implement OAuth callback server (localhost)
- [ ] Device flow authentication
- [ ] Credential storage (OS keyring)
- [ ] Token refresh logic
- [ ] Support providers: Google, GitHub, Microsoft
- [ ] Pass tokens to MCP servers securely

**Technical Approach:**
1. Create `OAuthManager` module
2. Implement HTTP callback server
3. Store credentials in OS keyring
4. Integrate with MCP server authentication

**Risks:**
- Port conflicts (mitigation: random port selection)
- Token security (mitigation: OS keyring, encryption)

---

### Task 5.3: Skills System
**Priority:** P2 (Medium)  
**Effort:** 8 days  
**Assignee:** TBD  
**Dependencies:** Task 3.1 (AGENTS.md Discovery pattern)

**Description:**
Implement modular skills system for context injection.

**Acceptance Criteria:**
- [ ] Discover `SKILL.md` files in:
  - `.claude/` directory
  - `.agents/skills/` directory
  - Project directories
- [ ] Parse YAML frontmatter:
  ```yaml
  ---
  name: python-testing
  description: pytest best practices
  keywords: [python, test, pytest]
  ---
  ```
- [ ] Load relevant skills based on:
  - User query keywords
  - File types in context
  - Project language
- [ ] Inject skill content into context
- [ ] `/skills list` command
- [ ] `/skills load <name>` command
- [ ] Skill marketplace/sharing support (future)

**Technical Approach:**
1. Create `SkillsManager` module
2. Use ripgrep for discovery
3. Parse YAML frontmatter with `serde_yaml`
4. Implement relevance matching (keyword matching initially)
5. Inject skills into system prompt
6. Add commands for skill management

**Risks:**
- Too many skills bloat context (mitigation: relevance filtering, limits)
- Skill quality varies (mitigation: curation, validation)

---

### Task 5.4: Hooks System
**Priority:** P3 (Low)  
**Effort:** 6 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Allow users to define custom hooks for tool execution and session lifecycle.

**Acceptance Criteria:**
- [ ] Hook types:
  - `pre_tool` (before tool execution)
  - `post_tool` (after tool execution)
  - `session_start` (session initialization)
  - `session_end` (session cleanup)
- [ ] Hooks defined in `ragent.json`:
  ```json
  "hooks": {
    "pre_tool": {
      "bash": "echo 'About to run command'"
    },
    "post_tool": {
      "write": "./scripts/validate_write.sh {path}"
    }
  }
  ```
- [ ] Hook execution with parameters
- [ ] Hook failures can block operations
- [ ] Timeout protection (default 5s)

**Technical Approach:**
1. Create `HookManager` module
2. Parse hook configuration
3. Execute hooks at appropriate lifecycle points
4. Pass tool parameters to hook scripts
5. Handle hook failures gracefully

**Risks:**
- Slow hooks degrade performance (mitigation: timeouts, warnings)
- Hook errors confusing to users (mitigation: clear error messages)

---

### Task 5.5: Auto-Update Mechanism
**Priority:** P3 (Low)  
**Effort:** 5 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Implement automatic update checking and installation.

**Acceptance Criteria:**
- [ ] Check for updates on startup (async, non-blocking)
- [ ] Query GitHub releases API for latest version
- [ ] Compare with current version (semantic versioning)
- [ ] Notify user if update available
- [ ] `/update` command to install latest version
- [ ] Download release binary for current platform
- [ ] Verify checksum
- [ ] Replace binary (self-update)
- [ ] Config option to disable auto-check

**Technical Approach:**
1. Create `UpdateManager` module
2. Query GitHub releases API on startup (background task)
3. Download binary to temp location
4. Verify SHA256 checksum
5. Use `self_update` crate for binary replacement
6. Add `/update` command

**Risks:**
- Network failures on startup (mitigation: timeout, ignore errors)
- Binary replacement fails (mitigation: backup current binary)
- Checksum verification failures (mitigation: abort update, clear error)

---

### Task 5.6: `/doctor` Diagnostic Command
**Priority:** P3 (Low)  
**Effort:** 3 days  
**Assignee:** TBD  
**Dependencies:** None

**Description:**
Health check command to diagnose common configuration issues.

**Acceptance Criteria:**
- [ ] Check configuration:
  - `ragent.json` valid syntax
  - API keys configured
  - Provider endpoints reachable
- [ ] Check dependencies:
  - Git installed
  - Ripgrep installed (optional)
  - Shell available
- [ ] Check permissions:
  - Working directory writable
  - Config file readable
- [ ] Report findings with color coding:
  - ✅ Green: OK
  - ⚠️ Yellow: Warning
  - ❌ Red: Error
- [ ] Suggestions for fixes

**Technical Approach:**
1. Create `Doctor` module with diagnostic checks
2. Run checks in parallel
3. Format results for display
4. Add `/doctor` command

**Risks:**
- None significant

---

### Milestone 5 Testing & Integration
**Effort:** 6 days  
**Tasks:**
- [ ] Test MCP with stdio, SSE, HTTP servers
- [ ] Test MCP OAuth flow with real providers
- [ ] Test skills discovery and relevance matching
- [ ] Test hooks system with various configurations
- [ ] Test auto-update on all platforms
- [ ] Test `/doctor` with various configuration states

---

## Phase Summary & Effort Estimation

### Total Effort by Milestone
| Milestone | Tasks | Development Days | Testing Days | Total Weeks |
|-----------|-------|------------------|--------------|-------------|
| M1: Quick Wins | 5 | 15 | 3 | 3.6 |
| M2: Autonomous | 4 | 25 | 5 | 6 |
| M3: Intelligence | 5 | 25 | 5 | 6 |
| M4: Advanced | 6 | 44 | 7 | 10.2 |
| M5: Ecosystem | 6 | 43 | 6 | 9.8 |
| **Total** | **26** | **152** | **26** | **35.6** |

**Note:** Assuming 5-day work weeks, 35.6 weeks ≈ **8-9 months** for core development.

### Adjusted Timeline with Parallelization
With 2 developers working in parallel:
- M1: 4 weeks
- M2: 5 weeks (some dependencies)
- M3: 5 weeks (some dependencies)
- M4: 6 weeks (persistent shell is complex)
- M5: 7 weeks (MCP is complex)

**Total: ~27 weeks (6-7 months) with 2 developers**

---

## Dependencies & Critical Path

### Critical Path (Must be sequential)
1. Task 1.1 (Cost Tracking) → M2 Autopilot (need cost monitoring)
2. Task 2.3 (Mode Switching) → Task 2.2 (Plan Mode depends on modes)
3. Task 3.1 (AGENTS.md Discovery) → Task 3.2 (Memory Write needs discovery)
4. Task 3.1 → Task 3.3 (`/init` needs discovery)
5. Task 4.3 (GitHub API) → Task 4.4, 4.5 (Issues/PRs depend on API)
6. Task 5.1 (MCP Client) → Task 5.2 (OAuth depends on client)

### Parallel Opportunities
**Milestone 1:** All tasks can be done in parallel (no dependencies)

**Milestone 2:**
- Task 2.1 (Autopilot) and Task 2.4 (Completion Tool) can be parallel
- Task 2.3 (Mode Switching) must precede Task 2.2 (Plan Mode)

**Milestone 3:**
- Task 3.1 (Discovery) and Task 3.5 (Ripgrep) first
- Then Task 3.2, 3.3, 3.4 in parallel

**Milestone 4:**
- Task 4.1, 4.2 (Shell) in parallel with Task 4.3 (GitHub API)
- Task 4.4, 4.5 after GitHub API complete
- Task 4.6 (Security) can be parallel with any

**Milestone 5:**
- All tasks can be done in parallel except Task 5.2 depends on Task 5.1

---

## Risk Management

### High-Risk Tasks
1. **Task 4.1 (Persistent Shell)** — Complexity: High
   - **Risks:** PTY handling, cross-platform compatibility, zombie processes
   - **Mitigation:** Prototype early, extensive testing, fallback to non-persistent
   - **Contingency:** If blocked, defer to later milestone and prioritize other features

2. **Task 2.1 (Autopilot Mode)** — Complexity: Medium, Impact: Critical
   - **Risks:** Infinite loops, cost overruns, poor completion detection
   - **Mitigation:** Strict iteration limits, cost monitoring, explicit completion tool
   - **Contingency:** Add manual checkpoints if pure autopilot proves unstable

3. **Task 5.1 (MCP Client)** — Complexity: High
   - **Risks:** Protocol complexity, spec changes, server instability
   - **Mitigation:** Follow official SDKs, isolation, timeouts
   - **Contingency:** Implement stdio transport first, defer SSE/HTTP if needed

### Medium-Risk Tasks
- **Task 4.3-4.5 (GitHub Integration):** API rate limiting, token security
- **Task 3.3 (`/init` Command):** Inaccurate project analysis
- **Task 5.3 (Skills System):** Context bloat, relevance matching accuracy

### Low-Risk Tasks
- All Milestone 1 tasks (well-defined, low complexity)
- Security features (clear requirements, existing patterns)
- Polish features (Milestone 5, low impact)

---

## Success Criteria by Milestone

### Milestone 1 Success Criteria
✅ Cost tracking shows accurate USD costs for all providers  
✅ Git context appears in system prompt for all sessions  
✅ Permission prompts reduced by 40% with safe whitelist  
✅ ThinkTool used by agents in >50% of complex operations  
✅ README auto-injection working in 90%+ of projects  

### Milestone 2 Success Criteria
✅ Autopilot mode completes 20-iteration workflow without errors  
✅ Plan approval workflow reduces wasted work by 50% (user survey)  
✅ Mode switching preserves context and restricts tools correctly  
✅ 70% of tasks complete in autopilot without interruption  

### Milestone 3 Success Criteria
✅ AGENTS.md discovery finds files in nested projects  
✅ Memory write tool used by agents to persist learnings  
✅ `/init` generates accurate AGENTS.md for 80% of project types  
✅ Context loading time < 200ms with memoization  
✅ Ripgrep searches 10x faster than grep  

### Milestone 4 Success Criteria
✅ Persistent shell preserves Python venv and Node nvm state  
✅ GitHub integration completes issue → PR → merge workflow  
✅ Security prevents 100% of banned commands and directory escapes  
✅ Shell state displayed accurately in UI  

### Milestone 5 Success Criteria
✅ MCP client connects to 90% of popular MCP servers  
✅ Skills system loads relevant skills for project context  
✅ Auto-update successfully updates binary on all platforms  
✅ `/doctor` identifies and reports common issues  

---

## Resource Requirements

### Team Composition
**Recommended:** 2 full-time developers for 6-7 months

**Option 1: 2 Generalist Developers**
- Developer A: Milestone 1 + Milestone 3 + Milestone 5.1-5.3
- Developer B: Milestone 2 + Milestone 4 + Milestone 5.4-5.6

**Option 2: Specialist Split**
- Backend/Core Developer: Persistent shell, MCP, autopilot
- Integration Developer: GitHub, security, skills, hooks

### Infrastructure
- GitHub repository for issue tracking
- CI/CD for automated testing (GitHub Actions)
- Test environments: Linux, macOS, Windows
- API access: Anthropic, OpenAI, GitHub for testing

### External Dependencies
- Ripgrep binary (optional but recommended)
- Git binary (required)
- MCP server ecosystem (external, no control)
- GitHub API (external, rate limited)

---

## Rollout Strategy

### Alpha Release (After Milestone 1)
**Target:** Internal testing, power users  
**Duration:** 2 weeks  
**Features:** Cost tracking, git context, README, whitelist, ThinkTool  
**Goal:** Validate quick wins, gather feedback  

### Beta Release (After Milestone 2)
**Target:** Early adopters, community  
**Duration:** 4 weeks  
**Features:** + Autopilot, plan mode, mode switching  
**Goal:** Test autonomous operation, identify edge cases  

### Release Candidate (After Milestone 3)
**Target:** Wider community  
**Duration:** 4 weeks  
**Features:** + Project intelligence, memory system, `/init`  
**Goal:** Validate production readiness, gather adoption metrics  

### v1.0 Production (After Milestone 4)
**Target:** General availability  
**Features:** + Persistent shell, GitHub integration, advanced security  
**Goal:** Feature parity with competitors, production-ready  

### v1.5 Enhancement (After Milestone 5)
**Target:** Enterprise users, power users  
**Features:** + Full MCP, skills, hooks, auto-update  
**Goal:** Ecosystem integration, extensibility  

---

## Metrics & KPIs

### Development Metrics
- **Velocity:** Story points per sprint (2-week sprints)
- **Code quality:** Test coverage >80% for new features
- **Bug rate:** <5 critical bugs per milestone
- **Performance:** No regression in execution speed

### Product Metrics
- **Adoption:** GitHub stars, downloads, active users
- **Engagement:** Session duration, commands per session
- **Satisfaction:** User surveys, NPS score
- **Cost transparency:** 100% of users can view costs

### Competitive Metrics
- **Feature parity:** Close 80% of critical gaps within 6 months
- **Performance:** Maintain 5-10x speed advantage
- **Reliability:** 99.9% uptime for persistent shell

---

## Open Questions & Decisions Needed

### Technical Decisions
1. **Persistent Shell:** PTY vs subprocess approach?
   - **Recommendation:** PTY for full interactivity, accept complexity
   
2. **MCP Implementation:** Build from scratch vs adapt existing SDK?
   - **Recommendation:** Adapt TypeScript SDK patterns in Rust, don't reinvent
   
3. **Skills Relevance:** Embeddings vs keyword matching?
   - **Recommendation:** Start with keywords (low complexity), add embeddings later

4. **Cost Tracking:** Real-time vs post-session?
   - **Recommendation:** Real-time in status bar, detailed summary post-session

### Product Decisions
1. **Autopilot Defaults:** Enable by default or opt-in?
   - **Recommendation:** Opt-in via `--autopilot` flag, too risky as default
   
2. **Security Defaults:** Strict or permissive?
   - **Recommendation:** Strict (banned commands, directory guards), allow override
   
3. **MCP Servers:** Bundle any servers by default?
   - **Recommendation:** No bundling, provide curated list in docs

4. **Pricing:** Open-source vs commercial features?
   - **Recommendation:** Keep core open-source, consider enterprise add-ons later

---

## Conclusion

This implementation plan provides a **clear, actionable roadmap** to close competitive gaps while strengthening ragent's unique advantages. The phased approach delivers value incrementally:

1. **Quick wins** (M1) demonstrate momentum and close critical gaps
2. **Autonomous operation** (M2) achieves competitive parity with market leaders
3. **Project intelligence** (M3) improves long-term usability and context management
4. **Advanced features** (M4) address the persistent shell UX gap and add differentiation
5. **Ecosystem integration** (M5) ensures ragent works seamlessly with the broader AI tooling ecosystem

With **2 developers over 6-7 months**, ragent can achieve feature parity with ClaudeCode, OpenCode, and GitHub Copilot CLI while maintaining its core advantages in performance, transparency, and multi-provider support.

**Next Steps:**
1. Review and approve this implementation plan
2. Assign milestone owners
3. Set up project tracking (GitHub Projects or similar)
4. Begin Milestone 1 development
5. Establish testing and CI/CD infrastructure

---

**Document End**
