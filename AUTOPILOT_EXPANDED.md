## 11. Autopilot Mode

### 11.1 Overview

Autopilot mode enables **autonomous, hands-free operation** where the agent iteratively solves problems without requiring user confirmation for each tool execution. The agent can read files, execute commands, modify code, search the codebase, and handle multi-step workflows independently. Autopilot is ideal for long-running tasks, CI/CD pipelines, and scenarios where human oversight is infeasible.

Unlike interactive mode (where every tool invocation requires `/approve`), autopilot allows the agent to:
- Execute tools without permission dialogs
- Loop and iterate based on results
- Make autonomous decisions about next steps
- Work in headless/unattended environments

**Crucially**, autopilot respects the permission system — tools still follow the deny/allow/ask rules configured in `ragent.json`. Only "ask" permissions are auto-approved; "deny" rules still prevent execution.

### 11.2 Enabling & Disabling Autopilot

| Command | Description |
|---------|-------------|
| `/autopilot on [--max-tokens N] [--max-time N] [--max-iterations N]` | Enable autonomous operation with optional constraints |
| `/autopilot off` | Disable autonomous operation |
| `/autopilot status` | Show current autopilot status and remaining budget |

#### Constraints

**Token Budget** (`--max-tokens`):
- Limits total input + output tokens for the entire autopilot session
- Includes all LLM calls, tool results, and context
- Default: unlimited (no budget)
- Example: `--max-tokens 50000` limits to 50k tokens total

**Time Budget** (`--max-time`):
- Limits wall-clock execution time in seconds
- Default: unlimited
- Example: `--max-time 300` limits to 5 minutes

**Iteration Limit** (`--max-iterations`):
- Limits the number of agent loop cycles (LLM call + tool execution)
- Each LLM response that issues tool calls counts as one iteration
- Default: unlimited
- Example: `--max-iterations 10` allows max 10 LLM→tool rounds

### 11.3 Autopilot Behavior

#### State Machine

```
┌─────────────────┐
│  Autopilot OFF  │
└────────┬────────┘
         │
         │ /autopilot on
         ▼
┌─────────────────────────────────────────┐
│    Autopilot ON (running)               │
│  ┌─────────────────────────────────┐   │
│  │ 1. Send prompt to LLM           │   │
│  │ 2. Receive response & tool list │   │
│  │ 3. Execute tools (no prompt)    │   │
│  │ 4. Collect results              │   │
│  │ 5. Inject results back to LLM   │   │
│  │ 6. Repeat until goal reached    │   │
│  └─────────────────────────────────┘   │
│  (constrained by token/time/iteration)  │
└────────┬────────────────────────────────┘
         │
    ┌────┴────┬──────────────┬──────────────┐
    │          │              │              │
    ▼          ▼              ▼              ▼
  GOAL     TOKEN LIMIT   TIME LIMIT    ITERATION
 REACHED   EXCEEDED      EXCEEDED      LIMIT HIT
    │          │              │              │
    └──────────┴──────────────┴──────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Autopilot OFF (result summary shown)   │
└─────────────────────────────────────────┘
```

#### Permission System Integration

Autopilot respects the three-tier permission model:

| Config | Autopilot Behavior |
|--------|-------------------|
| `"allow"` | Tool executes immediately without prompt |
| `"ask"` | Auto-approved in autopilot mode (would be prompted in interactive) |
| `"deny"` | Tool is blocked, autopilot cannot override |

**Example permission config:**

```json
{
  "permissions": [
    { "permission": "bash:execute", "pattern": "rm -rf", "action": "deny" },
    { "permission": "bash:execute", "pattern": "src/**/*.rs", "action": "allow" },
    { "permission": "file:write", "pattern": "tests/**", "action": "ask" }
  ]
}
```

In autopilot:
- Dangerous `rm -rf` commands are **never** executed (blocked by deny)
- Rust file edits in `src/` **execute** immediately (allowed)
- File writes in `tests/` are **auto-approved** (ask → auto-approve in autopilot)

### 11.4 Use Cases & Examples

#### Use Case 1: Automated Code Refactoring

**Scenario**: You want the agent to refactor a codebase to use a new async pattern across 50 files, without stopping at each file.

```
User: I need to convert all our synchronous I/O to async/await. Use tokio.

/autopilot on --max-tokens 100000 --max-time 600

Agent (autopilot):
1. Search codebase for sync I/O patterns
2. Identify 47 files needing updates
3. Create a refactoring plan
4. Edit each file (10+ tool calls)
5. Run tests incrementally
6. Report results

→ After 8 minutes and ~80k tokens, autopilot exits with summary
```

**Why autopilot**: Asking for permission at each file would require 47+ user interactions. Autopilot completes the entire task unattended.

---

#### Use Case 2: CI/CD Integration

**Scenario**: A GitHub Action calls ragent via HTTP to review a pull request, run linters, and suggest fixes.

```bash
# .github/workflows/auto-review.yml
- run: |
    ragent run "Review this PR for style, security, and performance" \
      --agent code-review \
      --yes  # Equivalent to autopilot with full auto-approval
```

**Why autopilot**: The CI pipeline can't interact with the user. The agent needs to complete the review, run tools, and exit with a report.

---

#### Use Case 3: Bug Hunt & Root Cause Analysis

**Scenario**: An error is reported in production. You give the agent 5 minutes to investigate, gather logs, reproduce locally, and pinpoint the bug.

```
User: A timeout error in the database layer. Find the root cause.

/autopilot on --max-time 300 --max-iterations 15

Agent (autopilot):
1. Search for timeout-related code
2. Check database connection pools
3. Review recent commits
4. Run local tests with verbose logging
5. Analyze performance metrics
6. Propose fix

→ After 4 minutes, delivers findings
```

**Why autopilot**: Rapid investigation without user delays. Time limit ensures it doesn't spiral.

---

#### Use Case 4: Documentation Generation

**Scenario**: Auto-generate API documentation, code examples, and changelog from source code.

```
User: Generate complete API docs for the new web service module.

/autopilot on --max-tokens 80000

Agent (autopilot):
1. Index web service module
2. Extract function signatures
3. Read docstrings
4. Generate Markdown docs
5. Create code examples
6. Build table of contents
7. Write to docs/ folder

→ Documentation complete in 1-2 minutes
```

**Why autopilot**: Repetitive, deterministic task. No user input needed until final review.

---

#### Use Case 5: Test Suite Expansion

**Scenario**: Add comprehensive test coverage for a new feature without manual review at each step.

```
User: Add unit and integration tests for the new payment module.

/autopilot on --max-iterations 20 --max-tokens 60000

Agent (autopilot):
1. Analyze payment module structure
2. Identify untested code paths
3. Write unit tests for each function
4. Create integration test scenarios
5. Run tests and fix failures
6. Check coverage metrics
7. Report summary

→ New test suite ready in 3-4 minutes
```

**Why autopilot**: Test writing is iterative and self-validating. Autopilot loops until coverage targets are met.

---

### 11.5 Practical Examples

#### Example 1: Simple Refactoring (Token-Limited)

```
/autopilot on --max-tokens 25000

User: Convert three JavaScript files to TypeScript: app.js, api.js, utils.js

Agent:
  Step 1: Read app.js, convert, write app.ts
  Step 2: Read api.js, convert, write api.ts
  Step 3: Read utils.js, convert, write utils.ts
  Step 4: Run TypeScript compiler, report errors
  Step 5: Summarize changes

[After ~15k tokens, autopilot exits]

✅ All files converted and type-checked
```

---

#### Example 2: Headless CI/CD (Full Auto-Approval)

```bash
# Running ragent in a CI pipeline
ragent run "Lint all Python files and fix style issues" \
  --agent debug \
  --yes  # Enables auto-approval (equivalent to autopilot)

# Agent automatically:
# - Runs black, flake8, pylint
# - Applies fixes
# - Commits changes if configured
# - Exits with status code
```

---

#### Example 3: Constrained Investigation (Time-Limited)

```
/autopilot on --max-time 120

User: Why is the test suite failing? Investigate and propose fixes.

Agent (max 2 minutes):
  • Read failing test logs
  • Search for related code
  • Identify root cause
  • Suggest minimal fix
  
→ Report after 90 seconds
→ You review and decide next steps
```

---

### 11.6 Status & Monitoring

The `/autopilot status` command shows:

```
Autopilot Status
═══════════════════════════════════════
Status:          RUNNING
Elapsed Time:    2m 15s / 5m 0s (45% used)
Tokens Used:     28,450 / 50,000 (57% budget)
Iterations:      8 / 15 (53% limit)

Active Constraints:
  • Max Time:      5m 0s
  • Max Tokens:    50,000
  • Max Iterations: 15

Last Tool:       bash (executed 12s ago)
Next Loop:       Pending LLM response...
═══════════════════════════════════════
```

### 11.7 Safety & Best Practices

#### Do's ✅

- **Use constraints** — Always set `--max-tokens` and `--max-time` for bounded tasks
- **Start small** — Test autopilot on low-risk tasks first (documentation, tests)
- **Review results** — Even in autopilot, review the final output before merging/deploying
- **Trust permissions** — Rely on `ragent.json` deny rules to block dangerous operations
- **Monitor status** — Check `/autopilot status` periodically for long-running tasks

#### Don'ts ❌

- **Unlimited autopilot on unknown code** — Always set time/token limits
- **Disable safety rules** — Never use `/yolo` with autopilot (see Section 11.8)
- **Fire-and-forget** — Don't assume autopilot completed successfully without checking logs
- **Mix autopilot + network access** — Limit web access in constrained tasks
- **Assume permission auto-approval** — "ask" rules auto-approve in autopilot, but audit your config

### 11.8 YOLO Mode (Unrestricted Execution)

| Command | Description |
|---------|-------------|
| `/yolo` | Toggle YOLO mode (bypass all command validation and tool restrictions) |

#### ⚠️ WARNING: YOLO Mode Disables All Safety

YOLO mode bypasses:
- Bash validation (no `-e` safety checks)
- Permission rules (all tools allowed)
- Path guards (can write anywhere)
- Confirmation dialogs

**When to use YOLO:**
- Trusted code in isolated environments
- Local development with no external access
- Testing dangerous operations that require full freedom

**How to use with autopilot:**
```
/yolo                    # Enable YOLO mode
/autopilot on            # Autopilot + YOLO = complete freedom
```

**⚠️ Critical**: YOLO + Autopilot = maximum risk. Only use on isolated test systems.

---

### 11.9 Configuration

Autopilot behavior can be pre-configured in `ragent.json`:

```json
{
  "autopilot": {
    "enabled": false,
    "default_max_tokens": 50000,
    "default_max_time": 300,
    "default_max_iterations": 20,
    "auto_approve_ask_permissions": true
  }
}
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | bool | `false` | Start ragent in autopilot mode |
| `default_max_tokens` | int | null | Default token budget (null = unlimited) |
| `default_max_time` | int | null | Default time limit in seconds |
| `default_max_iterations` | int | null | Default iteration limit |
| `auto_approve_ask_permissions` | bool | `true` | Auto-approve "ask" rules in autopilot mode |

---
