
---
description: Audit codebase for coding standards, duplication, logging hygiene, security, test quality, and dependency freshness
---
Run a comprehensive code quality audit of this project. If the user provided a path argument, scope the audit to that subdirectory only: `$ARGUMENTS`

## Step 1: Auto-detect project context

Read the project root to determine language and framework. Check for files like `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`, `build.gradle`, `*.sln`, `Gemfile`, or similar. Also check for `tsconfig.json`, `.eslintrc*`, `eslint.config.*`, `CLAUDE.md`, `AGENTS.md`, `AGENTS-*.md` and any linting/formatting config. Identify:

- Primary language(s) and framework(s)
- Test framework and test directory convention
- Source directory convention
- Any project-specific coding standards from `CLAUDE.md` `AGENTS.md`, `AGENTS-*.md`or linter configs

Store this as a short context string (2-3 sentences). You will pass this to each subagent.

## Step 2: Launch 6 parallel audit agents

Launch ALL 6 of the following Task agents in a SINGLE message so they run in parallel. Agents 1-5 MUST use `subagent_type: "Explore"`. Agent 6 MUST use `subagent_type: "general"` (it needs Bash to run CLI tools). Each agent receives the project context string from Step 1.

If `$ARGUMENTS` is non-empty, tell each agent to restrict its search to that path. Otherwise, agents search the full project source (excluding node_modules, dist, build, vendor, .git, and similar output directories).

**CRITICAL output constraint for EVERY agent**: Return ONLY a structured bullet list of findings. Each finding must follow this exact format:

```
- [HIGH|MEDIUM|LOW] <short description> — <file:line> (and <file:line> if comparing two locations)
```

Maximum 15 findings per agent. No prose, no preamble, no summary paragraph — just the bullet list. If no issues found, return `- No issues found.`

### Agent 1: Coding Standards & Consistency

```
You are auditing a <context>.

Check that the codebase follows consistent coding standards and patterns. Look for:

1. NAMING: Inconsistent naming conventions — mixing camelCase/snake_case/PascalCase where the project uses one style. Check variable names, function names, file names, and export names.
2. PATTERNS: Inconsistent application of project patterns — if the project uses a specific pattern (e.g., singleton init, error handling, module structure), find places that deviate from that pattern without justification.
3. ERROR HANDLING: Inconsistent error handling — some functions throw, some return null, some swallow errors silently. Flag functions that handle errors differently from the dominant pattern.
4. STYLE: Inconsistent code style within the same project — e.g., mixing async/await with .then() chains, mixing arrow functions with function declarations for the same purpose, inconsistent import ordering.

Use Glob to find source files, Read to examine them, Grep to find patterns.
Return findings as: - [HIGH|MEDIUM|LOW] description — file:line (and file:line)
Max 15 findings. No prose.
```

### Agent 2: Code Duplication

```
You are auditing a <context>.

Scan source files (NOT test files) for duplication and repeated patterns. Look for:

1. NEAR-IDENTICAL FUNCTIONS: Functions across different files that do essentially the same thing with minor variations (different variable names, slight parameter differences). Compare function signatures and bodies.
2. COPY-PASTED BLOCKS: 3+ lines of similar code appearing in multiple places that could be extracted into a shared helper.
3. REPEATED LOGIC: The same pattern of API calls, data transformations, or error handling repeated across files. E.g., the same try/catch/log pattern, the same data formatting logic, the same validation sequence.
4. DUPLICATED CONSTANTS: Magic strings or numbers repeated in multiple files instead of being defined in one place.

Use Glob to find source files, Read to examine them, Grep to find repeated string literals and similar function names. Focus on the most impactful duplications first.
Return findings as: - [HIGH|MEDIUM|LOW] description — file:line (and file:line)
Max 15 findings. No prose.
```

### Agent 3: Logging Hygiene & Debug Artifacts

```
You are auditing a <context>.

Check for improper logging practices and leftover debug artifacts. Look for:

1. RAW CONSOLE CALLS: Any use of console.log, console.warn, console.error, console.info, console.debug, or language equivalents (print, println, fmt.Println, etc.) in production source code. These should use a proper structured logging library/service instead. Flag every instance.
2. DEBUG LEFTOVERS: Temporary debugging code left in — commented-out console.log/print statements, TODO/FIXME/HACK/XXX comments that reference debugging, temporary variables only used for logging.
3. SENSITIVE DATA IN LOGS: Log statements that might output tokens, passwords, API keys, user data, or full request/response bodies. Check what variables are being passed to log calls.
4. INCONSISTENT LOG LEVELS: If the project does have a logger, check that log levels are used appropriately — errors logged as info, info logged as debug, etc.

Use Grep to search for console.log, console.warn, console.error, console.info, console.debug, print, println, and similar patterns. Read the surrounding context to assess severity.
Return findings as: - [HIGH|MEDIUM|LOW] description — file:line
Max 15 findings. No prose.
```

### Agent 4: Security — Secrets & Environment Handling

```
You are auditing a <context>.

Check for security issues related to secrets, environment variables, and sensitive data handling. Look for:

1. HARDCODED SECRETS: Any hardcoded API keys, tokens, passwords, connection strings, or credentials in source code. Check for strings that look like tokens (long alphanumeric strings), URLs with embedded credentials, and variables named key/secret/token/password with literal values.
2. ENV VALIDATION: Environment variables accessed directly (process.env.X, os.environ, etc.) without validation or type checking. They should go through a validated config layer, not be read ad-hoc throughout the codebase.
3. GITIGNORE GAPS: Check .gitignore for missing entries — .env files, credential files (*.pem, *.key, service-account.json), data directories with potentially sensitive content, IDE configs that may contain project-specific secrets.
4. SECRET EXPOSURE: Secrets that could leak through error messages, stack traces, API responses, or log output. Check error handlers and catch blocks for what they expose.
5. COMMITTED SECRETS: Check if any .env, credential, or key files exist in the repo (not just gitignored but actually present in tracked files).

Use Glob to find config files, .env files, credential files. Grep for patterns like hardcoded tokens, process.env, os.environ. Read .gitignore and compare against actual files.
Return findings as: - [HIGH|MEDIUM|LOW] description — file:line
Max 15 findings. No prose.
```

### Agent 5: Test Quality & Coverage

```
You are auditing a <context>.

Evaluate test quality, coverage gaps, and superfluous tests. Look for:

1. COVERAGE GAPS: Source files or modules that have NO corresponding test file. List each untested source file. Cross-reference by checking if each source file's exports are imported in any test file.
2. SUPERFLUOUS TESTS: Tests that don't actually assert anything meaningful — tests that only check truthiness, tests that are essentially duplicates of other tests with trivially different inputs, tests that mock so heavily they only test the mocks, tests that test language/framework behavior rather than application logic.
3. DUPLICATED TEST SETUP: Identical or near-identical beforeEach/beforeAll/setup blocks across multiple test files that should be extracted into shared test helpers. Repeated mock definitions that could be centralized.
4. FRAGILE TESTS: Tests tightly coupled to implementation details — testing private internals, asserting on exact string output that will break on formatting changes, testing mock call counts instead of behavior.
5. MISSING EDGE CASES: Test files that only test the happy path for functions that have obvious error/edge cases (null inputs, empty arrays, network failures, etc.).

Use Glob to find all test files and all source files. Read test files to assess quality. Grep to check which source exports are referenced in test files.
Return findings as: - [HIGH|MEDIUM|LOW] description — file:line
Max 15 findings. No prose.
```

### Agent 6: Dependency Freshness & Security

**IMPORTANT: This agent MUST use `subagent_type: "general-purpose"` (NOT "Explore") because it needs Bash access to run CLI tools.**

```
You are auditing a <context>.

Check dependency freshness, security vulnerabilities, and lockfile health.
The project context tells you which ecosystems are in use. For each detected
ecosystem, run the appropriate checks:

**For Node.js (package.json detected):**
1. Run `npm audit --json 2>/dev/null` — flag critical and high vulnerabilities as HIGH, moderate as MEDIUM
2. Run `npm outdated --json 2>/dev/null` — flag packages 2+ major versions behind as MEDIUM, 1 major as LOW, minor/patch as LOW
3. Verify package-lock.json exists and is committed. If missing, flag as MEDIUM.

**For Python (pyproject.toml or requirements.txt detected):**
1. Run `pip-audit --format=json 2>/dev/null` — flag critical/high as HIGH, moderate as MEDIUM
2. Run `pip list --outdated --format=json 2>/dev/null` — flag packages 2+ major versions behind as MEDIUM, others as LOW
3. Verify lock file (poetry.lock, requirements.txt with pinned versions) exists. If missing, flag as MEDIUM.

**For Rust (Cargo.toml detected):**
1. Run `cargo audit --json 2>/dev/null` — flag critical/high as HIGH, moderate as MEDIUM
2. Run `cargo outdated --root-deps-only 2>/dev/null` — flag 2+ major versions behind as MEDIUM, others as LOW
3. Verify Cargo.lock exists and is committed. If missing, flag as MEDIUM.

**For Go (go.mod detected):**
1. Run `govulncheck ./... 2>/dev/null` — flag vulnerabilities as HIGH
2. Run `go list -m -u all 2>/dev/null` — flag major updates as MEDIUM, minor/patch as LOW
3. Verify go.sum exists and is committed. If missing, flag as MEDIUM.

If a CLI tool is not installed or fails, note it as:
- [LOW] <tool> not available — unable to check <ecosystem> <check type>

Do NOT flag devDependencies outdated versions as MEDIUM or HIGH — cap at LOW.
Do NOT flag pre-release or alpha/beta versions as outdated.
Prioritize HIGH and MEDIUM findings first. Only include LOW (minor/patch outdated) findings if you have remaining slots in the 15-finding cap.

Return findings as: - [HIGH|MEDIUM|LOW] description — package@version (current -> latest)
For lockfile or tooling issues, use a descriptive identifier instead of package@version.
Max 15 findings. No prose.
```

## Step 3: Aggregate and report

After ALL 6 agents return their findings:

1. Collect all findings into a single list
2. Deduplicate: if two agents flagged the same file:line or essentially the same issue, merge them into one finding and note which categories flagged it
3. Group by severity: HIGH first, then MEDIUM, then LOW
4. Within each severity group, organize by category: Standards, Duplication, Logging, Security, Testing, Dependencies

Output the final report in this exact format:

The report should be output to `docs/reports/code-audit.md`

```markdown
# Code Audit Report

**Project**: <detected project type>
**Scope**: <full project or scoped path>
**Findings**: <N total> (<H> high, <M> medium, <L> low)

---

## High Priority

### <Category>
- <finding> — `file:line` or `package@version (current -> latest)`
- <finding> — `file:line` or `package@version (current -> latest)`

## Medium Priority

### <Category>
- <finding> — `file:line` or `package@version`

## Low Priority

### <Category>
- <finding> — `file:line` or `package@version`
```

If there are no findings at a severity level, omit that entire section. Do NOT add any recommendations or action items — just the findings.

## Step 4: Generate implementation plan

After presenting the audit report from Step 3, generate an implementation plan to fix the findings. Categorize each finding by estimated effort and provide a suggested fix order.

1. Take the aggregated findings from Step 3
2. Group each finding into one of three effort categories:
   - **Quick Wins** (< 30 min each): simple renames, import reordering, adding missing gitignore entries, removing debug artifacts, updating patch/minor dependency versions
   - **Medium Effort** (30 min - 2 hours each): extracting shared helpers, deduplicating functions, standardizing patterns, adding missing test files, upgrading major dependency versions, fixing missing lockfiles
   - **Complex** (> 2 hours each): architectural refactors, major security fixes, cross-cutting pattern changes, resolving vulnerability chains requiring coordinated dependency upgrades
3. For each finding, identify the file(s) involved and write a brief fix description
4. Determine a suggested fix order: utilities/shared code first, then refactors, then tests, then cosmetic changes
5. Add a verification checklist

Output the implementation plan in this exact format, appended after the audit report:

```markdown
# Implementation Plan

## Quick Wins (< 30 min each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | <finding description> | `file:line` | <brief fix description> |

## Medium Effort (30 min - 2 hours each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | <finding description> | `file:line` | <brief fix description> |

## Complex (> 2 hours)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | <finding description> | `file:line` | <brief fix description> |

## Suggested Fix Order
1. <category>: <rationale>
2. ...

## Verification
- [ ] TypeScript / language compiler reports 0 errors
- [ ] All tests pass
- [ ] New tests cover previously-untested files
```

If a category has no findings, omit that table. The user can then discuss the plan and ask you to implement it.
