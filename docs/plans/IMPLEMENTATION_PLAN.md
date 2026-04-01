# ragent Implementation Plan & Roadmap
**Task ID:** s6  
**Prepared by:** swarm-s6 (tm-006)  
**Team:** swarm-20260330-191253  
**Date:** March 30, 2026  

---

## Executive Summary

This implementation plan transforms the gap analysis findings into an actionable roadmap with concrete milestones, task breakdowns, and sequencing. The plan balances enterprise readiness, competitive parity, and strategic differentiation through 5 major phases spanning 18-24 months.

**Strategic Focus:**
1. **Enterprise Readiness First** — Security, observability, and compliance to unlock B2B market
2. **Leverage Team Orchestration Advantage** — Enhance existing differentiator before competitors catch up
3. **Accessibility & i18n** — Legal compliance and 3x market expansion
4. **Code Intelligence Parity** — Match Aider/OpenCode on LSP and indexing
5. **Innovation Leadership** — Voice, hooks, mobile to stay ahead

**Key Metrics:**
- **Total Effort:** ~18-24 person-months over 2 years
- **Critical Path:** Security fixes → Observability → RBAC → SOC 2
- **First Value:** 4-6 weeks (security fixes + first-run wizard)
- **Enterprise Readiness:** 6-8 months (Phase 1)
- **Full Feature Parity:** 12-18 months (Phase 1-3)

---

## Table of Contents

1. [Strategic Phases Overview](#1-strategic-phases-overview)
2. [Phase 1: Enterprise Readiness](#phase-1-enterprise-readiness-q2-2026)
3. [Phase 2: Developer Experience](#phase-2-developer-experience-q3-2026)
4. [Phase 3: Global Expansion](#phase-3-global-expansion-q4-2026)
5. [Phase 4: Compliance & Certification](#phase-4-compliance--certification-q1-2027)
6. [Phase 5: Advanced Features](#phase-5-advanced-features-2027)
7. [Dependency Map](#7-dependency-map)
8. [Resource Planning](#8-resource-planning)
9. [Risk Management](#9-risk-management)
10. [Success Criteria](#10-success-criteria)

---

## 1. Strategic Phases Overview

| Phase | Timeline | Focus | Key Deliverables | Business Impact |
|-------|----------|-------|------------------|-----------------|
| **Phase 1** | Q2 2026 (6-8 months) | Enterprise Readiness | Security, Observability, Cost Tracking, Accessibility | Unlock B2B sales, pass security reviews |
| **Phase 2** | Q3 2026 (7-10 months) | Developer Experience | LSP Docs, Indexing, RBAC, UX Polish | Competitive parity, user retention |
| **Phase 3** | Q4 2026 (4-6 months) | Global Expansion | i18n, Hooks, Performance | 3x addressable market, extensibility |
| **Phase 4** | Q1 2027 (3-4 months) | Compliance | SOC 2, ISO 27001, GDPR | Regulated industry access |
| **Phase 5** | 2027+ (Ongoing) | Innovation | Voice, Worktree, Mobile | Market leadership, future-proofing |

**Total Duration:** 18-24 months  
**Estimated Effort:** 3-4 full-time engineers  
**Investment:** $500K-$750K (fully-loaded costs)

---

# Phase 1: Enterprise Readiness (Q2 2026)

**Duration:** 6-8 months  
**Effort:** 8-10 person-months  
**Priority:** P0 (Critical Blockers)  
**Goal:** Pass enterprise security reviews, enable cost management, achieve WCAG 2.1 Level AA compliance

## Milestone 1.1: Security Hardening (Weeks 1-6)

### Objective
Fix all critical security vulnerabilities identified in security review to pass enterprise security audits.

### Features & Tasks

#### Feature 1.1.1: Secrets Management Overhaul
**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Replace SQLite secrets with OS keychain** (Week 1)
   - [ ] Integrate `keyring` crate (0.13+)
   - [ ] Implement `KeychainSecretsStore` trait
   - [ ] Add macOS Keychain backend via Security framework
   - [ ] Add Windows Credential Manager backend
   - [ ] Add Linux Secret Service backend (libsecret)
   - [ ] Implement fallback to encrypted file for headless environments
   - [ ] Add `RAGENT_SECRETS_BACKEND` env var override

2. **Migrate existing secrets** (Week 2)
   - [ ] Create migration script in `crates/ragent-core/src/secrets/migrate.rs`
   - [ ] Detect existing SQLite secrets on startup
   - [ ] Prompt user for migration confirmation
   - [ ] Copy secrets to keychain with error handling
   - [ ] Archive old `secrets.db` with `.migrated` suffix
   - [ ] Log migration success/failure

3. **Update configuration** (Week 2)
   - [ ] Update `ragent.json` schema to remove `secrets_path`
   - [ ] Add `secrets_backend` config option (keychain, encrypted-file)
   - [ ] Update SPEC.md with new architecture
   - [ ] Update QUICKSTART.md with setup instructions

**Testing Requirements:**
- [ ] Unit tests for each keychain backend
- [ ] Integration test: store/retrieve secret
- [ ] Integration test: migration from SQLite
- [ ] Manual testing on macOS, Windows, Linux
- [ ] Headless environment testing (CI)

**Documentation:**
- [ ] Add `docs/userdocs/secrets-management.md`
- [ ] Update QUICKSTART.md security section
- [ ] Add troubleshooting guide for keychain issues

**Success Criteria:**
- [ ] Zero secrets in SQLite database
- [ ] All backends pass integration tests
- [ ] Migration script tested on 10+ real configs
- [ ] Documentation reviewed by 2 team members

---

#### Feature 1.1.2: TLS Certificate Validation
**Complexity:** Low  
**Effort:** 1 week  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Remove `danger_accept_invalid_certs`** (Days 1-2)
   - [ ] Remove config option from `ProviderConfig`
   - [ ] Remove `reqwest::Client` builder calls with `.danger_accept_invalid_certs(true)`
   - [ ] Add custom root CA support via `RAGENT_CA_BUNDLE` env var
   - [ ] Update error messages for cert validation failures

2. **Implement cert pinning for internal APIs** (Days 3-5)
   - [ ] Add `tls_pinned_certs` config option (base64-encoded DER)
   - [ ] Implement cert pinning in `reqwest` client builder
   - [ ] Add validation logic in `crates/ragent-core/src/http/client.rs`
   - [ ] Document pinning workflow for enterprise deployments

**Testing Requirements:**
- [ ] Unit test: cert validation enabled by default
- [ ] Integration test: reject self-signed cert
- [ ] Integration test: custom CA bundle works
- [ ] Integration test: cert pinning validates correctly

**Documentation:**
- [ ] Add section to `docs/userdocs/enterprise-deployment.md`
- [ ] Document custom CA workflow

**Success Criteria:**
- [ ] No invalid cert acceptance in codebase
- [ ] Custom CA workflow tested in Docker container
- [ ] Security team approval

---

#### Feature 1.1.3: Command Injection Prevention
**Complexity:** Medium  
**Effort:** 1-2 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Shell escaping for `bash` tool** (Week 1)
   - [ ] Replace `Command::new("bash").arg("-c")` with direct command execution
   - [ ] Use `shlex` crate for shell argument parsing
   - [ ] Implement allowlist for safe commands (git, cargo, npm, etc.)
   - [ ] Add `RAGENT_ALLOW_UNSAFE_COMMANDS` flag for power users
   - [ ] Update `bash` tool implementation in `crates/ragent-core/src/tools/bash.rs`

2. **Sanitize user inputs** (Week 2)
   - [ ] Audit all tools for shell execution: `bash`, `grep`, `glob`, `list`
   - [ ] Implement input validation in `ToolContext`
   - [ ] Add max command length limit (10KB)
   - [ ] Add blocklist for dangerous patterns: `rm -rf /`, `dd if=/dev/zero`, etc.
   - [ ] Log all command executions with sanitized inputs

**Testing Requirements:**
- [ ] Unit test: shell metacharacters escaped
- [ ] Integration test: injection attempts blocked
- [ ] Fuzzing test: random shell metacharacters
- [ ] Security review of allowlist/blocklist

**Documentation:**
- [ ] Update SPEC.md with command execution security model
- [ ] Add security FAQ: "How does ragent prevent code execution attacks?"

**Success Criteria:**
- [ ] Zero shell injection vulnerabilities found in penetration testing
- [ ] Allowlist covers 95% of legitimate use cases
- [ ] Security team sign-off

---

#### Feature 1.1.4: SSRF Protection
**Complexity:** Medium  
**Effort:** 1-2 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Implement URL allowlist** (Week 1)
   - [ ] Add `allowed_domains` config option to `ragent.json`
   - [ ] Default allowlist: public APIs (api.openai.com, api.anthropic.com, etc.)
   - [ ] Block private IP ranges: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8
   - [ ] Block link-local: 169.254.0.0/16, fe80::/10
   - [ ] Implement in `crates/ragent-core/src/tools/webfetch.rs`

2. **DNS rebinding protection** (Week 2)
   - [ ] Resolve hostname to IP before connection
   - [ ] Validate IP is not in blocked ranges
   - [ ] Re-resolve periodically for long-lived connections
   - [ ] Add timeout for DNS resolution (5 seconds)
   - [ ] Implement in `http/client.rs`

**Testing Requirements:**
- [ ] Unit test: private IPs blocked
- [ ] Integration test: DNS rebinding attack prevented
- [ ] Integration test: allowed domains work
- [ ] Manual test: attempt to fetch `http://169.254.169.254/` (AWS metadata)

**Documentation:**
- [ ] Add `docs/userdocs/network-security.md`
- [ ] Document allowlist configuration

**Success Criteria:**
- [ ] SSRF vulnerability scanner reports zero findings
- [ ] Works correctly with corporate proxies
- [ ] Security team approval

---

#### Feature 1.1.5: Storage Encryption
**Complexity:** High  
**Effort:** 2-3 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** Feature 1.1.1 (Secrets Management)

**Technical Tasks:**
1. **Encrypt SQLite database at rest** (Week 1)
   - [ ] Integrate `sqlcipher` or `rusqlite` with `sqlite3_key`
   - [ ] Generate encryption key via `RAGENT_DB_KEY` env var or keychain
   - [ ] Implement key derivation: PBKDF2-HMAC-SHA256, 100K iterations
   - [ ] Store salt in database header
   - [ ] Update `crates/ragent-core/src/storage/sqlite.rs`

2. **Encrypt file snapshots** (Week 2)
   - [ ] Use ChaCha20-Poly1305 for snapshot files
   - [ ] Store encrypted snapshots in `.ragent/snapshots/`
   - [ ] Implement transparent decryption on restore
   - [ ] Add integrity verification (HMAC)
   - [ ] Update `crates/ragent-core/src/storage/snapshots.rs`

3. **Key rotation support** (Week 3)
   - [ ] Add `ragent keys rotate` command
   - [ ] Re-encrypt database with new key
   - [ ] Re-encrypt all snapshots
   - [ ] Atomic operation with rollback on failure
   - [ ] Log rotation events to audit log

**Testing Requirements:**
- [ ] Unit test: encryption/decryption round-trip
- [ ] Integration test: database opens with correct key
- [ ] Integration test: database rejects incorrect key
- [ ] Integration test: key rotation preserves data
- [ ] Performance test: encryption overhead <10%

**Documentation:**
- [ ] Add `docs/userdocs/encryption.md`
- [ ] Update QUICKSTART.md with encryption setup
- [ ] Document key rotation workflow

**Success Criteria:**
- [ ] All session data encrypted at rest
- [ ] Zero plaintext secrets in filesystem
- [ ] Performance overhead <10%
- [ ] Security audit approval

---

### Milestone Success Criteria (1.1)
- [ ] All P0 security issues resolved (8 critical vulnerabilities)
- [ ] Security whitepaper draft completed
- [ ] Internal penetration test passed
- [ ] Ready for external security audit

---

## Milestone 1.2: Observability & Telemetry (Weeks 7-12)

### Objective
Implement OpenTelemetry integration for distributed tracing and metrics to enable monitoring, debugging, and cost optimization.

### Features & Tasks

#### Feature 1.2.1: OpenTelemetry Integration
**Complexity:** High  
**Effort:** 3-4 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Core OTLP setup** (Week 1)
   - [ ] Add dependencies: `opentelemetry`, `opentelemetry-otlp`, `tracing-opentelemetry`
   - [ ] Implement `TelemetryConfig` in `ragent.json`:
     ```json
     {
       "telemetry": {
         "enabled": true,
         "endpoint": "http://localhost:4317",
         "service_name": "ragent",
         "sample_rate": 1.0,
         "export_interval_secs": 30
       }
     }
     ```
   - [ ] Initialize OTLP exporter in `crates/ragent-core/src/telemetry/mod.rs`
   - [ ] Configure tracing subscriber with OpenTelemetry layer
   - [ ] Add privacy opt-out via `RAGENT_TELEMETRY=false`

2. **Span instrumentation** (Week 2)
   - [ ] Root span: `ragent.session` (duration from start to end)
     - Attributes: `session_id`, `agent_type`, `model`
   - [ ] Span: `ragent.llm.request` (per LLM call)
     - Attributes: `provider`, `model`, `prompt_tokens`, `completion_tokens`, `total_tokens`, `cached_tokens`
   - [ ] Span: `ragent.tool.execute` (per tool invocation)
     - Attributes: `tool_name`, `success`, `error_message`, `duration_ms`
   - [ ] Span: `ragent.team.task` (per team task)
     - Attributes: `task_id`, `teammate_id`, `status`
   - [ ] Instrument in `crates/ragent-core/src/agent/mod.rs`

3. **Metrics implementation** (Week 3)
   - [ ] Counter: `ragent_llm_requests_total` (labels: provider, model, status)
   - [ ] Counter: `ragent_tokens_used_total` (labels: provider, model, type=[prompt|completion|cached])
   - [ ] Counter: `ragent_api_cost_dollars` (calculated from token usage)
   - [ ] Counter: `ragent_tool_executions_total` (labels: tool_name, status)
   - [ ] Histogram: `ragent_session_duration_seconds` (buckets: 10s, 1m, 10m, 1h, 24h)
   - [ ] Histogram: `ragent_llm_latency_seconds` (buckets: 0.1s, 1s, 5s, 30s, 60s)
   - [ ] Implement in `crates/ragent-core/src/telemetry/metrics.rs`

4. **Prometheus endpoint** (Week 4)
   - [ ] Add `/metrics` HTTP endpoint to `ragent-server`
   - [ ] Expose metrics in Prometheus text format
   - [ ] Add authentication: Bearer token or IP allowlist
   - [ ] Document Prometheus scrape config in `docs/userdocs/observability.md`

**Testing Requirements:**
- [ ] Integration test: spans exported to OTLP collector
- [ ] Integration test: metrics scraped from `/metrics`
- [ ] Integration test: privacy opt-out works
- [ ] Load test: 1000 requests with minimal overhead
- [ ] Manual test: visualize traces in Jaeger

**Documentation:**
- [ ] Create `docs/userdocs/observability.md`
- [ ] Document OTLP collector setup (Jaeger, Tempo, Datadog)
- [ ] Document Prometheus + Grafana setup
- [ ] Add example Grafana dashboard JSON

**Success Criteria:**
- [ ] All LLM requests traced with token counts
- [ ] Metrics exported to Prometheus
- [ ] Traces visualized in Jaeger
- [ ] Performance overhead <5%
- [ ] Enterprise customer can view telemetry in their existing stack

---

#### Feature 1.2.2: Cost Tracking Database Schema
**Complexity:** Medium  
**Effort:** 1-2 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** Feature 1.2.1 (OpenTelemetry)

**Technical Tasks:**
1. **Schema updates** (Week 1)
   - [ ] Add columns to `messages` table:
     - `prompt_tokens INTEGER`
     - `completion_tokens INTEGER`
     - `cached_tokens INTEGER`
     - `estimated_cost_usd REAL`
   - [ ] Create `token_usage` table:
     ```sql
     CREATE TABLE token_usage (
       id INTEGER PRIMARY KEY,
       session_id TEXT NOT NULL,
       message_id INTEGER,
       provider TEXT NOT NULL,
       model TEXT NOT NULL,
       prompt_tokens INTEGER NOT NULL,
       completion_tokens INTEGER NOT NULL,
       cached_tokens INTEGER DEFAULT 0,
       cost_usd REAL NOT NULL,
       timestamp TEXT NOT NULL
     );
     ```
   - [ ] Add migration in `crates/ragent-core/src/storage/migrations/`

2. **Cost calculation** (Week 2)
   - [ ] Implement `CostCalculator` trait in `crates/ragent-core/src/cost/mod.rs`
   - [ ] Add pricing data per provider/model:
     ```rust
     pub struct ModelPricing {
       pub prompt_cost_per_1k: f64,
       pub completion_cost_per_1k: f64,
       pub cached_cost_per_1k: f64,
     }
     ```
   - [ ] Update pricing from provider APIs (cached daily)
   - [ ] Calculate cost on every LLM response
   - [ ] Store in database and emit metric

**Testing Requirements:**
- [ ] Unit test: cost calculation accuracy
- [ ] Integration test: costs persisted correctly
- [ ] Integration test: historical cost queries

**Success Criteria:**
- [ ] Token usage tracked for 100% of LLM requests
- [ ] Cost estimates accurate to ±5%
- [ ] Historical cost data queryable

---

#### Feature 1.2.3: Cost Reporting & Budgets
**Complexity:** Medium  
**Effort:** 2 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** Feature 1.2.2 (Cost Tracking)

**Technical Tasks:**
1. **CLI reporting** (Week 1)
   - [ ] Implement `ragent stats --cost` command
   - [ ] Show breakdown by:
     - Provider/model
     - Session
     - Date range
     - Tool usage
   - [ ] Format as table with `comfy-table`
   - [ ] Add CSV export: `ragent stats --cost --format csv > costs.csv`

2. **Budget enforcement** (Week 2)
   - [ ] Add config options:
     ```json
     {
       "cost_limits": {
         "daily_budget_usd": 50.0,
         "monthly_budget_usd": 500.0,
         "warn_threshold": 0.8,
         "hard_limit": true
       }
     }
     ```
   - [ ] Check budget before LLM request
   - [ ] Warn at 80% threshold
   - [ ] Block requests if hard limit reached (or soft-warn)
   - [ ] Implement in `crates/ragent-core/src/cost/budget.rs`

**Testing Requirements:**
- [ ] Integration test: `ragent stats --cost` output
- [ ] Integration test: budget warning triggered
- [ ] Integration test: hard limit blocks request
- [ ] Manual test: review cost report accuracy

**Documentation:**
- [ ] Add `docs/userdocs/cost-management.md`
- [ ] Document budget configuration
- [ ] Add FAQ: "How do I track API costs?"

**Success Criteria:**
- [ ] Users can see cost breakdown by session/model
- [ ] Budget warnings prevent overspending
- [ ] Export to CSV for accounting

---

### Milestone Success Criteria (1.2)
- [ ] OpenTelemetry integration complete
- [ ] Cost tracking accurate to ±5%
- [ ] Enterprise customers can monitor usage in their dashboards
- [ ] Documentation reviewed by ops team

---

## Milestone 1.3: Audit Logging (Weeks 13-16)

### Objective
Implement tamper-proof audit logging for compliance (SOC 2, HIPAA, ISO 27001).

### Features & Tasks

#### Feature 1.3.1: Structured Audit Log
**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** Feature 1.1.5 (Storage Encryption)

**Technical Tasks:**
1. **Audit log schema** (Week 1)
   - [ ] Create `audit_log` table:
     ```sql
     CREATE TABLE audit_log (
       id INTEGER PRIMARY KEY,
       timestamp TEXT NOT NULL,
       event_type TEXT NOT NULL,
       actor TEXT NOT NULL,
       resource TEXT,
       action TEXT NOT NULL,
       result TEXT NOT NULL,
       metadata TEXT,
       signature TEXT NOT NULL
     );
     ```
   - [ ] Implement append-only constraint (no UPDATE/DELETE)
   - [ ] Add index on `timestamp`, `event_type`, `actor`

2. **Event instrumentation** (Week 2)
   - [ ] Log events:
     - Authentication: `user.login`, `user.logout`, `api.key.created`
     - Authorization: `permission.granted`, `permission.denied`
     - Data access: `file.read`, `file.write`, `file.delete`
     - Configuration: `config.updated`, `secrets.rotated`
     - Team actions: `team.created`, `task.claimed`, `teammate.shutdown`
   - [ ] Implement `AuditLogger` in `crates/ragent-core/src/audit/mod.rs`
   - [ ] Add structured metadata (JSON)

3. **Cryptographic signing** (Week 3)
   - [ ] Sign each log entry with Ed25519
   - [ ] Store public key in config
   - [ ] Verify signature chain on export
   - [ ] Implement in `crates/ragent-core/src/audit/signing.rs`

**Testing Requirements:**
- [ ] Unit test: event serialization
- [ ] Integration test: audit log immutability
- [ ] Integration test: signature verification
- [ ] Security review of signing implementation

**Documentation:**
- [ ] Add `docs/userdocs/audit-logging.md`
- [ ] Document compliance use cases
- [ ] Add export workflow for auditors

**Success Criteria:**
- [ ] All security-relevant events logged
- [ ] Logs tamper-proof (signature verification)
- [ ] Export format compatible with SIEM tools (JSON Lines)
- [ ] Compliance team approval

---

#### Feature 1.3.2: Audit Log Export & Retention
**Complexity:** Low  
**Effort:** 1 week  
**Priority:** P1 (High)  
**Dependencies:** Feature 1.3.1 (Audit Log)

**Technical Tasks:**
- [ ] Implement `ragent audit export` command
- [ ] Export as JSON Lines (one event per line)
- [ ] Filter by date range, event type, actor
- [ ] Add `retention_days` config option (default: 365)
- [ ] Automatic archival to compressed files
- [ ] S3/Azure Blob upload for long-term storage

**Testing Requirements:**
- [ ] Integration test: export all events
- [ ] Integration test: filter by date range
- [ ] Integration test: automatic archival

**Success Criteria:**
- [ ] Auditors can export logs for review
- [ ] Retention policy enforced automatically

---

### Milestone Success Criteria (1.3)
- [ ] Audit logging meets SOC 2 requirements
- [ ] Logs exportable for compliance audits
- [ ] Cryptographic signing prevents tampering

---

## Milestone 1.4: Accessibility (WCAG 2.1 AA) (Weeks 17-22)

### Objective
Achieve WCAG 2.1 Level AA compliance to enable legally-mandated accessibility and expand user base.

### Features & Tasks

#### Feature 1.4.1: Screen Reader Support
**Complexity:** High  
**Effort:** 4-5 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Accessible TUI mode** (Week 1-2)
   - [ ] Add `--accessible` CLI flag (launches simplified UI)
   - [ ] Implement text-only mode: no colors, no graphics, no animations
   - [ ] Use `ratatui` with screen reader compatibility
   - [ ] Add ARIA-like semantic markers in output
   - [ ] Announce focus changes, loading states, errors

2. **Keyboard navigation enhancements** (Week 3)
   - [ ] Tab navigation between all UI elements
   - [ ] Skip to content: Ctrl+J to skip header
   - [ ] Breadcrumbs announced when focus changes
   - [ ] Add `--help` output for every screen

3. **Screen reader testing** (Week 4-5)
   - [ ] Test with NVDA (Windows)
   - [ ] Test with JAWS (Windows)
   - [ ] Test with VoiceOver (macOS)
   - [ ] Test with Orca (Linux)
   - [ ] Document issues and iterate

**Testing Requirements:**
- [ ] Manual testing with screen readers
- [ ] User testing with blind developers (recruit 5+ testers)
- [ ] Integration test: accessible mode launches
- [ ] Accessibility audit by external consultant

**Documentation:**
- [ ] Create `docs/userdocs/accessibility.md`
- [ ] Document screen reader workflows
- [ ] Add keyboard shortcut cheat sheet

**Success Criteria:**
- [ ] 5+ blind users successfully complete onboarding
- [ ] WCAG 2.1 Level AA compliance verified
- [ ] Screen reader announcements logical and complete

---

#### Feature 1.4.2: High-Contrast Theme
**Complexity:** Medium  
**Effort:** 1-2 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
1. **Theme system** (Week 1)
   - [ ] Add `theme` config option: `default`, `high-contrast`, `colorblind`
   - [ ] Implement color scheme in `crates/ragent-tui/src/theme.rs`
   - [ ] High-contrast: black background, white/yellow text, no gradients
   - [ ] Colorblind-friendly: avoid red/green, use blue/orange
   - [ ] Respect terminal's `COLORFGBG` env var

2. **WCAG contrast ratios** (Week 2)
   - [ ] Ensure 7:1 contrast for normal text (AAA)
   - [ ] Ensure 4.5:1 contrast for large text (AA)
   - [ ] Test with `contrast-ratio` tool
   - [ ] Update all UI components

**Testing Requirements:**
- [ ] Manual testing with high-contrast mode
- [ ] Automated contrast ratio checks
- [ ] User testing with low-vision users

**Success Criteria:**
- [ ] All text meets WCAG 2.1 AA contrast requirements
- [ ] Colorblind users can distinguish all UI states

---

#### Feature 1.4.3: Keyboard-Only Navigation
**Complexity:** Low  
**Effort:** 1 week  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Audit all interactive elements for keyboard support
- [ ] Add focus indicators (highlighted border)
- [ ] Implement Shift+Tab for reverse navigation
- [ ] Add Escape key to close modals/dialogs
- [ ] Document all keyboard shortcuts in TUI

**Testing Requirements:**
- [ ] Manual testing: complete full workflow without mouse
- [ ] Integration test: keyboard navigation works

**Success Criteria:**
- [ ] All features accessible via keyboard
- [ ] Focus indicators visible

---

### Milestone Success Criteria (1.4)
- [ ] WCAG 2.1 Level AA compliance certified
- [ ] 10+ blind/low-vision users successfully onboarded
- [ ] Accessibility statement published

---

## Milestone 1.5: First-Run Wizard (Weeks 23-26)

### Objective
Reduce onboarding friction with interactive setup wizard to improve activation rate from ~50% to >80%.

### Features & Tasks

#### Feature 1.5.1: Interactive Setup
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P1 (High)  
**Dependencies:** Feature 1.1.1 (Secrets Management)

**Technical Tasks:**
1. **Wizard UI** (Week 1-2)
   - [ ] Launch wizard on first run (no `ragent.json` detected)
   - [ ] Step 1: Welcome screen + license agreement
   - [ ] Step 2: Provider selection (Anthropic, OpenAI, Ollama, GitHub Copilot)
   - [ ] Step 3: API key entry (masked input) + validation
   - [ ] Step 4: Model selection (show available models for provider)
   - [ ] Step 5: Optional settings (telemetry opt-in, theme, working directory)
   - [ ] Step 6: Summary + confirmation
   - [ ] Implement in `crates/ragent-tui/src/wizard/mod.rs`

2. **Configuration generation** (Week 3)
   - [ ] Generate `ragent.json` from wizard inputs
   - [ ] Store API key in keychain (via Feature 1.1.1)
   - [ ] Test API key with provider health check
   - [ ] Show helpful error messages if setup fails

3. **Guided tutorial** (Week 4)
   - [ ] After wizard: launch interactive tutorial
   - [ ] Tutorial covers:
     - Basic chat
     - File operations
     - Bash tool
     - Session management
     - Team coordination
   - [ ] Record completion in `~/.ragent/tutorial_completed`
   - [ ] Allow skip with `ragent --skip-tutorial`

**Testing Requirements:**
- [ ] Integration test: wizard completes successfully
- [ ] Integration test: wizard handles invalid API key
- [ ] User testing: 20+ users complete wizard
- [ ] Track completion rate

**Documentation:**
- [ ] Update QUICKSTART.md to reference wizard
- [ ] Add wizard screenshots to docs

**Success Criteria:**
- [ ] >80% of new users complete wizard
- [ ] <5 minutes average completion time
- [ ] Zero reports of "couldn't figure out setup"

---

### Milestone Success Criteria (1.5)
- [ ] First-run wizard implemented
- [ ] User activation rate >80%
- [ ] Onboarding friction eliminated

---

## Milestone 1.6: Performance Optimization (Weeks 27-30)

### Objective
Fix critical performance bottlenecks identified in performance review to improve responsiveness and scalability.

### Features & Tasks

#### Feature 1.6.1: Async Blocking Fixes
**Complexity:** High  
**Effort:** 3-4 weeks  
**Priority:** P0 (Critical)  
**Dependencies:** None

**Technical Tasks:**
1. **Tokio blocking audit** (Week 1)
   - [ ] Use `tokio-console` to identify blocking calls in async contexts
   - [ ] Audit all `std::sync::Mutex` usage
   - [ ] Audit all `std::fs` usage
   - [ ] Audit all synchronous SQLite calls

2. **Replace blocking primitives** (Week 2-3)
   - [ ] Replace `std::sync::Mutex` with `tokio::sync::Mutex`
   - [ ] Replace `std::fs` with `tokio::fs`
   - [ ] Wrap SQLite calls in `spawn_blocking`
   - [ ] Update `crates/ragent-core/src/storage/sqlite.rs`

3. **Connection pooling** (Week 4)
   - [ ] Implement SQLite connection pool with `sqlx` or `deadpool`
   - [ ] Configure pool size (default: 10 connections)
   - [ ] Add metrics for pool utilization
   - [ ] Test under high concurrency

**Testing Requirements:**
- [ ] Benchmark: TUI responsiveness (p95 latency <50ms)
- [ ] Load test: 100 concurrent sessions
- [ ] Integration test: no blocking in async context

**Success Criteria:**
- [ ] Zero blocking calls in async context (verified by `tokio-console`)
- [ ] TUI responsive under load
- [ ] Connection pool prevents contention

---

### Milestone Success Criteria (1.6)
- [ ] All critical performance issues resolved
- [ ] TUI responsive (<50ms p95 latency)
- [ ] Ready for load testing

---

## Phase 1 Success Criteria

**Enterprise Readiness Achieved:**
- [ ] Pass external security audit
- [ ] OpenTelemetry integration deployed to 10+ customers
- [ ] Cost tracking prevents budget overruns
- [ ] Audit logging meets SOC 2 requirements
- [ ] WCAG 2.1 AA compliance certified
- [ ] First-run wizard reduces onboarding time by 50%
- [ ] Performance meets SLA targets (<50ms p95)

**Business Impact:**
- [ ] 5+ enterprise POCs in progress
- [ ] 2+ enterprise contracts signed
- [ ] Security whitepaper published
- [ ] Featured in enterprise tool comparisons

**Timeline:** 6-8 months (30 weeks)  
**Effort:** 8-10 person-months

---

# Phase 2: Developer Experience (Q3 2026)

**Duration:** 7-10 months  
**Effort:** 10-12 person-months  
**Priority:** P1 (Competitive Parity)  
**Goal:** Match Aider/OpenCode on code intelligence, improve UX, implement RBAC

## Milestone 2.1: LSP Documentation & Examples (Weeks 1-4)

### Objective
Improve LSP tool adoption from ~20% to >50% of sessions through better documentation and examples.

### Features & Tasks

#### Feature 2.1.1: Comprehensive LSP Guide
**Complexity:** Low  
**Effort:** 2 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Create `docs/userdocs/lsp-guide.md` with:
  - What is LSP and why it's useful
  - Setup per language (Rust, TypeScript, Python, Go, Java)
  - Example workflows (find references, go to definition, diagnostics)
  - Troubleshooting common issues
- [ ] Add LSP setup to first-run wizard (optional)
- [ ] Add in-TUI help: press `F1` → "LSP Tools"

**Testing Requirements:**
- [ ] User testing: 10 developers complete LSP setup
- [ ] Track LSP tool usage in telemetry

**Success Criteria:**
- [ ] LSP tool usage increases to >50% of sessions
- [ ] <5% users report "couldn't configure LSP"

---

#### Feature 2.1.2: Example Workflows & Templates
**Complexity:** Low  
**Effort:** 2 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Create `examples/lsp_workflows/` with:
  - `refactor_with_lsp.md` — Using `lsp_references` to rename symbols
  - `code_navigation.md` — Using `lsp_definition` to explore codebase
  - `diagnostics_workflow.md` — Using `lsp_diagnostics` to fix errors
- [ ] Add sample agent prompts for common LSP tasks
- [ ] Create video tutorial (5 minutes)

**Success Criteria:**
- [ ] 3+ example workflows documented
- [ ] Video tutorial published on YouTube

---

### Milestone Success Criteria (2.1)
- [ ] LSP tool usage >50%
- [ ] Documentation rated 4.5/5 by users

---

## Milestone 2.2: Codebase Indexing (Weeks 5-12)

### Objective
Implement structural indexing to enable context-aware code generation and reduce hallucinations.

### Features & Tasks

#### Feature 2.2.1: Tree-sitter Structural Map
**Complexity:** High  
**Effort:** 4-5 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
1. **Tree-sitter integration** (Week 1-2)
   - [ ] Add `tree-sitter` crate (0.22+)
   - [ ] Add language grammars: Rust, TypeScript, Python, Go, Java
   - [ ] Implement parser in `crates/ragent-code/src/indexing/parser.rs`
   - [ ] Parse file into AST

2. **Structural map extraction** (Week 3-4)
   - [ ] Extract symbols:
     - Functions/methods (name, signature, location)
     - Classes/structs (name, fields, methods)
     - Imports/dependencies
     - Documentation comments
   - [ ] Build hierarchical map (module → class → method)
   - [ ] Store in `structural_map` table:
     ```sql
     CREATE TABLE structural_map (
       id INTEGER PRIMARY KEY,
       file_path TEXT NOT NULL,
       symbol_type TEXT NOT NULL,
       symbol_name TEXT NOT NULL,
       parent_id INTEGER,
       line_start INTEGER NOT NULL,
       line_end INTEGER NOT NULL,
       signature TEXT,
       doc_comment TEXT
     );
     ```

3. **Indexing workflow** (Week 5)
   - [ ] Trigger indexing on:
     - `ragent index` CLI command
     - Background after large file changes
     - Incremental indexing for edited files
   - [ ] Show progress bar in TUI
   - [ ] Store indexed files with checksum (skip if unchanged)

**Testing Requirements:**
- [ ] Unit test: parser handles all supported languages
- [ ] Integration test: index 1000-file codebase in <60 seconds
- [ ] Accuracy test: structural map matches manual review

**Documentation:**
- [ ] Add `docs/userdocs/codebase-indexing.md`
- [ ] Document indexing workflow

**Success Criteria:**
- [ ] Codebase indexed in <60 seconds for 1000 files
- [ ] Structural map accurate (95% precision)
- [ ] Used in >30% of large projects

---

#### Feature 2.2.2: Semantic Search (Phase 1)
**Complexity:** High  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Feature 2.2.1 (Tree-sitter Indexing)

**Technical Tasks:**
1. **Embedding generation** (Week 1-2)
   - [ ] Integrate local embedding model (e.g., `sentence-transformers` via `candle`)
   - [ ] Generate embeddings for:
     - Function signatures + docstrings
     - Class definitions
     - Code blocks
   - [ ] Store in `embeddings` table with vector column

2. **Vector search** (Week 3-4)
   - [ ] Implement k-NN search (cosine similarity)
   - [ ] Add `code_search` tool:
     - Input: natural language query ("function that handles HTTP requests")
     - Output: ranked list of relevant symbols
   - [ ] Integrate with agent context

**Testing Requirements:**
- [ ] Benchmark: search latency <500ms for 10K embeddings
- [ ] Accuracy test: relevant results in top 5

**Success Criteria:**
- [ ] Semantic search works for 5+ languages
- [ ] Reduces hallucinations by 30%

---

### Milestone Success Criteria (2.2)
- [ ] Codebase indexing implemented
- [ ] Used in >30% of large projects
- [ ] Hallucinations reduced by 30%

---

## Milestone 2.3: RBAC & SSO/SAML (Weeks 13-20)

### Objective
Enable multi-user deployments with role-based access control and enterprise authentication.

### Features & Tasks

#### Feature 2.3.1: Role-Based Access Control
**Complexity:** High  
**Effort:** 5-6 weeks  
**Priority:** P1 (High)  
**Dependencies:** Milestone 1.3 (Audit Logging)

**Technical Tasks:**
1. **RBAC schema** (Week 1)
   - [ ] Create tables:
     ```sql
     CREATE TABLE roles (
       id INTEGER PRIMARY KEY,
       name TEXT UNIQUE NOT NULL,
       permissions TEXT NOT NULL -- JSON array
     );
     CREATE TABLE user_roles (
       user_id TEXT NOT NULL,
       role_id INTEGER NOT NULL,
       granted_at TEXT NOT NULL
     );
     ```
   - [ ] Define roles: `admin`, `developer`, `viewer`, `auditor`
   - [ ] Define permissions: `files:read`, `files:write`, `bash:execute`, `config:update`, `audit:export`

2. **Permission enforcement** (Week 2-3)
   - [ ] Implement `PermissionChecker` in `crates/ragent-core/src/rbac/mod.rs`
   - [ ] Check permissions before:
     - Tool execution
     - Configuration changes
     - Audit log export
   - [ ] Return `PermissionDenied` error with helpful message

3. **User management** (Week 4-5)
   - [ ] Implement `ragent users` CLI:
     - `ragent users add <email> --role developer`
     - `ragent users list`
     - `ragent users set-role <email> admin`
     - `ragent users remove <email>`
   - [ ] Integrate with audit logging

4. **Multi-tenancy** (Week 6)
   - [ ] Add `tenant_id` column to all tables
   - [ ] Isolate data by tenant
   - [ ] Implement tenant-aware queries

**Testing Requirements:**
- [ ] Unit test: permission checks work
- [ ] Integration test: viewer cannot execute `bash`
- [ ] Integration test: auditor can export logs
- [ ] Security review of RBAC implementation

**Documentation:**
- [ ] Add `docs/userdocs/rbac.md`
- [ ] Document role definitions
- [ ] Document user management workflow

**Success Criteria:**
- [ ] RBAC enforced across all tools
- [ ] Multi-tenancy works (tested with 3 tenants)
- [ ] Audit trail for permission changes

---

#### Feature 2.3.2: SSO/SAML Integration
**Complexity:** High  
**Effort:** 4-5 weeks  
**Priority:** P1 (High)  
**Dependencies:** Feature 2.3.1 (RBAC)

**Technical Tasks:**
1. **SAML SP implementation** (Week 1-2)
   - [ ] Integrate `samael` crate for SAML 2.0
   - [ ] Implement Service Provider (SP) in `crates/ragent-server/src/auth/saml.rs`
   - [ ] Support IdPs: Okta, Azure AD, Auth0, OneLogin
   - [ ] Add `/saml/login`, `/saml/acs`, `/saml/metadata` endpoints

2. **OIDC integration** (Week 3)
   - [ ] Integrate `openidconnect` crate
   - [ ] Implement OIDC flow (authorization code grant)
   - [ ] Support providers: Okta, Auth0, Keycloak

3. **Session management** (Week 4-5)
   - [ ] Issue JWT tokens after authentication
   - [ ] Store sessions in Redis (for multi-instance deployments)
   - [ ] Implement refresh token rotation
   - [ ] Add session timeout (default: 8 hours)

**Testing Requirements:**
- [ ] Integration test: SAML login flow
- [ ] Integration test: OIDC login flow
- [ ] Manual test: authenticate with Okta, Azure AD
- [ ] Security review of authentication implementation

**Documentation:**
- [ ] Add `docs/userdocs/sso-setup.md`
- [ ] Document IdP configuration steps
- [ ] Add troubleshooting guide

**Success Criteria:**
- [ ] SSO works with Okta, Azure AD, Auth0
- [ ] Session management secure (no token leakage)
- [ ] Tested with 3+ enterprise customers

---

### Milestone Success Criteria (2.3)
- [ ] RBAC implemented and tested
- [ ] SSO/SAML works with major IdPs
- [ ] Multi-user deployments supported

---

## Milestone 2.4: Usage Analytics Dashboard (Weeks 21-26)

### Objective
Provide admins with visibility into team usage, costs, and performance.

### Features & Tasks

#### Feature 2.4.1: Analytics API
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P1 (High)  
**Dependencies:** Milestone 1.2 (Observability)

**Technical Tasks:**
1. **Analytics queries** (Week 1-2)
   - [ ] Aggregate metrics:
     - Sessions per user/day/month
     - Tokens used per user/model
     - Costs per user/project
     - Tool usage frequency
     - Error rates
   - [ ] Implement in `crates/ragent-core/src/analytics/mod.rs`

2. **REST API endpoints** (Week 3)
   - [ ] Add endpoints to `ragent-server`:
     - `GET /api/analytics/usage` — Overall usage stats
     - `GET /api/analytics/costs` — Cost breakdown
     - `GET /api/analytics/users` — Per-user metrics
     - `GET /api/analytics/models` — Model performance
   - [ ] Add authentication (admin role required)

3. **Web dashboard** (Week 4)
   - [ ] Create simple dashboard with `htmx` + `tailwindcss`
   - [ ] Charts: line chart (usage over time), bar chart (cost by user), pie chart (model distribution)
   - [ ] Serve from `ragent-server` at `/dashboard`

**Testing Requirements:**
- [ ] Integration test: API endpoints return correct data
- [ ] Manual test: dashboard displays charts
- [ ] Load test: dashboard handles 100 concurrent users

**Documentation:**
- [ ] Add `docs/userdocs/analytics-dashboard.md`
- [ ] Document dashboard usage

**Success Criteria:**
- [ ] Admins can view usage metrics
- [ ] Dashboard loads in <2 seconds
- [ ] Tested with 10+ enterprise customers

---

### Milestone Success Criteria (2.4)
- [ ] Analytics dashboard implemented
- [ ] Admins can track usage and costs
- [ ] Tested with enterprise customers

---

## Milestone 2.5: UX Improvements (Weeks 27-36)

### Objective
Polish user experience based on UX gap analysis to improve satisfaction and retention.

### Features & Tasks

#### Feature 2.5.1: Feature Discoverability
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
1. **In-app help** (Week 1-2)
   - [ ] Add `F1` key to show context-sensitive help
   - [ ] Help content:
     - Home screen → Overview of ragent
     - Chat screen → Available commands and tools
     - Session list → Session management tips
   - [ ] Implement in `crates/ragent-tui/src/help/mod.rs`

2. **Contextual tips** (Week 2-3)
   - [ ] Show tips in TUI footer:
     - "Try `/simplify` to optimize recent changes"
     - "Use `team_create` for parallel analysis"
     - "Press `?` for keyboard shortcuts"
   - [ ] Rotate tips based on context
   - [ ] Dismiss with `d` key

3. **Command palette** (Week 3-4)
   - [ ] Add `Ctrl+P` to open command palette (fuzzy search)
   - [ ] Search:
     - Tools (`bash`, `read`, `edit`, etc.)
     - Skills (`/debug`, `/simplify`)
     - Sessions (by name or ID)
     - Settings
   - [ ] Implement with `skim` crate

**Testing Requirements:**
- [ ] User testing: 20 users navigate with help system
- [ ] Integration test: F1 help displays
- [ ] Integration test: command palette search works

**Success Criteria:**
- [ ] Users discover 2+ new features per session
- [ ] Feature usage increases by 30%

---

#### Feature 2.5.2: Error Message Enhancements
**Complexity:** Low  
**Effort:** 2 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Audit all error messages for actionability
- [ ] Add suggested fixes to errors:
  - `"API key invalid" → "Run `ragent config set anthropic.api_key <key>`"`
  - `"File not found" → "Check the file path or use `glob` to search"`
- [ ] Implement in `crates/ragent-core/src/error.rs`
- [ ] Add error codes for documentation links

**Success Criteria:**
- [ ] All errors have actionable guidance
- [ ] User complaints about confusing errors drop by 80%

---

#### Feature 2.5.3: Progress Indicators
**Complexity:** Low  
**Effort:** 2 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Add progress bars for long operations:
  - Indexing large codebase
  - Generating embeddings
  - Exporting audit logs
- [ ] Show estimated time remaining
- [ ] Implement with `indicatif` crate

**Success Criteria:**
- [ ] Users informed of progress for all long operations
- [ ] Perceived wait time reduced

---

#### Feature 2.5.4: Unified Documentation Site
**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P1 (High)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Set up documentation site (mdBook or Docusaurus)
- [ ] Consolidate all docs from `docs/` folder
- [ ] Add:
  - Getting Started guide
  - Tutorials (10+ examples)
  - API reference
  - Troubleshooting
  - FAQ
- [ ] Host at `docs.ragent.dev` (GitHub Pages)

**Success Criteria:**
- [ ] Documentation site live
- [ ] Search works
- [ ] Rated 4.5/5 by users

---

### Milestone Success Criteria (2.5)
- [ ] Feature discoverability improved
- [ ] Error messages actionable
- [ ] Progress indicators for all long operations
- [ ] Unified documentation site live

---

## Phase 2 Success Criteria

**Developer Experience Achieved:**
- [ ] LSP tool usage >50%
- [ ] Codebase indexing used in >30% of projects
- [ ] RBAC & SSO deployed to 5+ enterprise customers
- [ ] Analytics dashboard used by admins
- [ ] User satisfaction >4.5/5
- [ ] Documentation rated 4.5/5

**Business Impact:**
- [ ] Competitive parity with Aider/OpenCode on code intelligence
- [ ] 10+ enterprise customers using RBAC
- [ ] Feature usage up 30%

**Timeline:** 7-10 months (36 weeks)  
**Effort:** 10-12 person-months

---

# Phase 3: Global Expansion (Q4 2026)

**Duration:** 4-6 months  
**Effort:** 6-8 person-months  
**Priority:** P2 (Market Expansion)  
**Goal:** 3x addressable market through internationalization, enable extensibility

## Milestone 3.1: Internationalization (i18n) (Weeks 1-12)

### Objective
Support 3 languages (Chinese, Spanish, German) to expand international market.

### Features & Tasks

#### Feature 3.1.1: i18n Infrastructure
**Complexity:** High  
**Effort:** 4-5 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** None

**Technical Tasks:**
1. **Fluent integration** (Week 1-2)
   - [ ] Integrate `fluent` crate for i18n
   - [ ] Create localization files in `locales/`:
     - `locales/en-US/main.ftl`
     - `locales/zh-CN/main.ftl`
     - `locales/es-ES/main.ftl`
     - `locales/de-DE/main.ftl`
   - [ ] Implement `I18n` trait in `crates/ragent-core/src/i18n/mod.rs`
   - [ ] Auto-detect locale from `LANG` env var

2. **String extraction** (Week 3-4)
   - [ ] Audit all hardcoded strings in codebase
   - [ ] Replace with `t!("message_id")` macro
   - [ ] Extract strings to FTL files
   - [ ] ~2000 strings to translate

3. **Plural & gender support** (Week 5)
   - [ ] Implement Fluent plural rules
   - [ ] Add gender-neutral alternatives
   - [ ] Test with native speakers

**Testing Requirements:**
- [ ] Unit test: string lookup works
- [ ] Integration test: locale switching works
- [ ] Native speaker review for 3 languages

**Documentation:**
- [ ] Add `docs/userdocs/i18n.md`
- [ ] Document translation contribution workflow

**Success Criteria:**
- [ ] 3 languages fully translated
- [ ] Native speaker approval
- [ ] Locale switching seamless

---

#### Feature 3.1.2: RTL Text Support
**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Feature 3.1.1 (i18n Infrastructure)

**Technical Tasks:**
- [ ] Add RTL layout support to TUI
- [ ] Detect RTL languages: Arabic, Hebrew
- [ ] Mirror UI layout (right-to-left)
- [ ] Test with Arabic locale

**Success Criteria:**
- [ ] RTL languages display correctly
- [ ] Tested with native Arabic speakers

---

#### Feature 3.1.3: Localization CI/CD
**Complexity:** Low  
**Effort:** 1 week  
**Priority:** P2 (Medium)  
**Dependencies:** Feature 3.1.1 (i18n Infrastructure)

**Technical Tasks:**
- [ ] Set up Crowdin or Weblate for community translations
- [ ] Automate string extraction from code
- [ ] CI checks for missing translations
- [ ] Release process includes updated translations

**Success Criteria:**
- [ ] Community can contribute translations
- [ ] Automated translation updates

---

### Milestone Success Criteria (3.1)
- [ ] 3 languages supported (Chinese, Spanish, German)
- [ ] >30% users from non-English countries
- [ ] Translation workflow automated

---

## Milestone 3.2: Hooks System (Weeks 13-18)

### Objective
Enable extensibility via lifecycle hooks for custom workflows.

### Features & Tasks

#### Feature 3.2.1: Hook Infrastructure
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** None

**Technical Tasks:**
1. **Hook definition** (Week 1)
   - [ ] Define hook points:
     - `pre_tool_execute`, `post_tool_execute`
     - `pre_llm_request`, `post_llm_request`
     - `session_start`, `session_end`
     - `file_changed`, `task_claimed`, `teammate_idle`
   - [ ] Add `hooks` section to `ragent.json`:
     ```json
     {
       "hooks": {
         "post_tool_execute": [
           { "command": "notify-send 'Tool executed: {tool_name}'" }
         ],
         "file_changed": [
           { "script": ".ragent/hooks/format.sh" }
         ]
       }
     }
     ```

2. **Hook execution** (Week 2-3)
   - [ ] Implement `HookRunner` in `crates/ragent-core/src/hooks/mod.rs`
   - [ ] Execute hooks asynchronously (non-blocking)
   - [ ] Pass context as JSON: `{tool_name, args, result}`
   - [ ] Add timeout (default: 30 seconds)
   - [ ] Log hook execution in audit log

3. **Example hooks** (Week 4)
   - [ ] Create `examples/hooks/`:
     - `git_commit.sh` — Auto-commit after file changes
     - `notify.sh` — Desktop notifications
     - `test_on_change.sh` — Run tests after code changes
   - [ ] Document hook use cases

**Testing Requirements:**
- [ ] Integration test: hooks execute correctly
- [ ] Integration test: hook timeout works
- [ ] Manual test: example hooks work

**Documentation:**
- [ ] Add `docs/userdocs/hooks.md`
- [ ] Document hook points and context

**Success Criteria:**
- [ ] Hooks system implemented
- [ ] 5+ example hooks provided
- [ ] Used by 20% of advanced users

---

### Milestone Success Criteria (3.2)
- [ ] Hooks system implemented
- [ ] Example hooks tested
- [ ] Used by 20% of users

---

## Milestone 3.3: Enhanced Vision Support (Weeks 19-22)

### Objective
Enable vision capabilities across all providers (currently only Anthropic).

### Features & Tasks

#### Feature 3.3.1: Multi-Provider Vision
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** None

**Technical Tasks:**
1. **OpenAI vision** (Week 1)
   - [ ] Add vision support to OpenAI provider
   - [ ] Support `gpt-4o`, `gpt-4-vision`
   - [ ] Implement image upload in `crates/ragent-core/src/provider/openai.rs`

2. **Google Gemini vision** (Week 2)
   - [ ] Add Gemini provider (new)
   - [ ] Support `gemini-1.5-pro-vision`
   - [ ] Implement in `crates/ragent-core/src/provider/gemini.rs`

3. **Vision tool** (Week 3)
   - [ ] Create `image_analyze` tool:
     - Input: image path + query
     - Output: description or answer
   - [ ] Support formats: PNG, JPEG, WebP
   - [ ] Add to tool catalog

4. **UI integration** (Week 4)
   - [ ] Add image preview in TUI (ASCII art or terminal graphics)
   - [ ] Drag-and-drop support in TUI (if terminal supports)

**Testing Requirements:**
- [ ] Integration test: analyze image with each provider
- [ ] Manual test: upload 10+ diverse images

**Documentation:**
- [ ] Add `docs/userdocs/vision.md`
- [ ] Document image analysis workflows

**Success Criteria:**
- [ ] Vision works on OpenAI, Anthropic, Gemini
- [ ] Used in 10% of sessions

---

### Milestone Success Criteria (3.3)
- [ ] Vision support on 3+ providers
- [ ] Used in 10% of sessions

---

## Milestone 3.4: Performance Optimization Round 2 (Weeks 23-26)

### Objective
Address remaining performance issues (cloning, LSP contention).

### Features & Tasks

#### Feature 3.4.1: Reduce Cloning
**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Audit cloning with `clippy::clone_on_ref_ptr`
- [ ] Replace clones with `Arc` references
- [ ] Use `Cow<str>` for string data
- [ ] Benchmark memory usage before/after

**Success Criteria:**
- [ ] Memory usage reduced by 20%

---

#### Feature 3.4.2: LSP Connection Pooling
**Complexity:** Medium  
**Effort:** 1-2 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Implement LSP client pool (max 5 concurrent clients per language)
- [ ] Queue requests when pool exhausted
- [ ] Add metrics for pool utilization

**Success Criteria:**
- [ ] LSP contention eliminated
- [ ] p95 latency <500ms

---

### Milestone Success Criteria (3.4)
- [ ] Memory usage reduced by 20%
- [ ] LSP latency <500ms

---

## Phase 3 Success Criteria

**Global Expansion Achieved:**
- [ ] 3 languages supported
- [ ] >30% users from non-English countries
- [ ] Hooks system used by 20% of advanced users
- [ ] Vision support on 3+ providers
- [ ] Performance optimized (memory, LSP)

**Business Impact:**
- [ ] 3x addressable market (international)
- [ ] Extensibility enables custom workflows
- [ ] Competitive with multi-modal agents

**Timeline:** 4-6 months (26 weeks)  
**Effort:** 6-8 person-months

---

# Phase 4: Compliance & Certification (Q1 2027)

**Duration:** 3-4 months  
**Effort:** 4-6 person-months  
**Priority:** P2 (Regulated Industries)  
**Goal:** Achieve SOC 2 Type II, ISO 27001, GDPR compliance

## Milestone 4.1: SOC 2 Type II Audit (Weeks 1-8)

### Objective
Pass SOC 2 Type II audit to enable sales to regulated industries.

### Features & Tasks

#### Feature 4.1.1: SOC 2 Readiness
**Complexity:** High  
**Effort:** 4-5 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Phase 1 (Security, Observability, Audit Logging)

**Technical Tasks:**
1. **Gap analysis** (Week 1)
   - [ ] Hire SOC 2 consultant
   - [ ] Audit against TSC criteria (Security, Availability, Confidentiality)
   - [ ] Document gaps

2. **Remediation** (Week 2-4)
   - [ ] Implement missing controls:
     - Backup and recovery procedures
     - Incident response plan
     - Access control reviews
     - Vendor management
   - [ ] Update policies and procedures

3. **Pre-audit** (Week 5)
   - [ ] Internal audit by consultant
   - [ ] Remediate findings
   - [ ] Prepare evidence package

**Testing Requirements:**
- [ ] Mock audit by consultant

**Documentation:**
- [ ] Create SOC 2 compliance documentation

**Success Criteria:**
- [ ] Pass pre-audit
- [ ] Ready for formal audit

---

#### Feature 4.1.2: Formal SOC 2 Audit
**Complexity:** High  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Feature 4.1.1 (Readiness)

**Technical Tasks:**
- [ ] Engage audit firm (Big 4 or boutique)
- [ ] Provide evidence over 3-6 months observation period
- [ ] Respond to auditor inquiries
- [ ] Receive SOC 2 Type II report

**Success Criteria:**
- [ ] SOC 2 Type II report issued with no material weaknesses
- [ ] Published on trust page

---

### Milestone Success Criteria (4.1)
- [ ] SOC 2 Type II certified
- [ ] Report published

---

## Milestone 4.2: ISO 27001 Certification (Weeks 9-12)

### Objective
Achieve ISO 27001 certification for international markets.

### Features & Tasks

#### Feature 4.2.1: ISO 27001 Implementation
**Complexity:** High  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Milestone 4.1 (SOC 2)

**Technical Tasks:**
- [ ] Gap analysis against ISO 27001:2022
- [ ] Implement ISMS (Information Security Management System)
- [ ] Document policies, procedures, controls
- [ ] Engage certification body
- [ ] Stage 1 audit (documentation review)
- [ ] Stage 2 audit (on-site assessment)

**Success Criteria:**
- [ ] ISO 27001 certificate issued

---

### Milestone Success Criteria (4.2)
- [ ] ISO 27001 certified

---

## Milestone 4.3: GDPR Compliance (Weeks 13-16)

### Objective
Ensure GDPR compliance for EU customers.

### Features & Tasks

#### Feature 4.3.1: Data Privacy Features
**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P2 (Medium)  
**Dependencies:** Milestone 1.3 (Audit Logging)

**Technical Tasks:**
1. **Data export** (Week 1)
   - [ ] Implement `ragent privacy export` command
   - [ ] Export all user data as JSON
   - [ ] Include: sessions, messages, audit logs

2. **Data deletion** (Week 2)
   - [ ] Implement `ragent privacy delete` command
   - [ ] Permanently delete all user data
   - [ ] Log deletion in audit log

3. **Consent management** (Week 3)
   - [ ] Add telemetry opt-in/opt-out UI
   - [ ] Record consent in database
   - [ ] Respect consent for all data processing

4. **Privacy policy** (Week 4)
   - [ ] Draft privacy policy
   - [ ] Legal review
   - [ ] Publish on website

**Testing Requirements:**
- [ ] Integration test: export all user data
- [ ] Integration test: delete user data
- [ ] Legal review of implementation

**Documentation:**
- [ ] Add `docs/userdocs/privacy.md`

**Success Criteria:**
- [ ] GDPR-compliant data handling
- [ ] Privacy policy published

---

### Milestone Success Criteria (4.3)
- [ ] GDPR compliance achieved
- [ ] Privacy features implemented

---

## Phase 4 Success Criteria

**Compliance & Certification Achieved:**
- [ ] SOC 2 Type II certified
- [ ] ISO 27001 certified
- [ ] GDPR compliant
- [ ] Security whitepaper published

**Business Impact:**
- [ ] Approved vendor for regulated industries (finance, healthcare, government)
- [ ] Trust page with certifications
- [ ] 10+ enterprise contracts in regulated sectors

**Timeline:** 3-4 months (16 weeks)  
**Effort:** 4-6 person-months

---

# Phase 5: Advanced Features (2027+)

**Duration:** Ongoing  
**Effort:** Ongoing  
**Priority:** P3 (Innovation)  
**Goal:** Stay ahead of market trends, maintain leadership

## Feature 5.1: Voice Input

**Complexity:** High  
**Effort:** 4-6 weeks  
**Priority:** P3 (Nice-to-Have)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Integrate speech-to-text API (Whisper, Deepgram)
- [ ] Add voice recording UI (press-and-hold)
- [ ] Transcribe and send to LLM
- [ ] Support multiple languages

**Success Criteria:**
- [ ] Voice input works in TUI
- [ ] Used by 5% of users

---

## Feature 5.2: Git Worktree Isolation

**Complexity:** Medium  
**Effort:** 3-4 weeks  
**Priority:** P3 (Nice-to-Have)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Create temporary git worktree for risky changes
- [ ] Isolate bash commands in worktree
- [ ] Merge changes back if successful
- [ ] Add `--isolated` flag to ragent

**Success Criteria:**
- [ ] Worktree isolation prevents project corruption
- [ ] Used by 10% of users

---

## Feature 5.3: Suggested Responses

**Complexity:** Medium  
**Effort:** 2-3 weeks  
**Priority:** P3 (Nice-to-Have)  
**Dependencies:** None

**Technical Tasks:**
- [ ] Implement `suggest_responses` tool
- [ ] Generate 3-5 suggested next questions
- [ ] Display as clickable buttons in TUI
- [ ] Track usage in telemetry

**Success Criteria:**
- [ ] Suggested responses increase engagement
- [ ] Used in 20% of sessions

---

## Feature 5.4: Web UI & Mobile Client

**Complexity:** Very High  
**Effort:** 8-12 weeks  
**Priority:** P3 (Nice-to-Have)  
**Dependencies:** Phase 2 (RBAC, Analytics)

**Technical Tasks:**
- [ ] Build web UI with React/Vue
- [ ] Reuse `ragent-server` as backend
- [ ] Mobile client with React Native
- [ ] Support file uploads, voice input
- [ ] Publish to app stores

**Success Criteria:**
- [ ] Web UI feature parity with TUI
- [ ] Mobile app published
- [ ] Used by 30% of users

---

## Feature 5.5: Semantic Indexing (Phase 2)

**Complexity:** Very High  
**Effort:** 6-8 weeks  
**Priority:** P3 (Nice-to-Have)  
**Dependencies:** Feature 2.2.2 (Semantic Search Phase 1)

**Technical Tasks:**
- [ ] Integrate graph database (Neo4j or dgraph)
- [ ] Build code knowledge graph (calls, imports, inheritance)
- [ ] Implement graph traversal queries
- [ ] Enable "explain this codebase" queries

**Success Criteria:**
- [ ] Knowledge graph improves context retrieval by 50%
- [ ] Used in 40% of large projects

---

## Phase 5 Success Criteria

**Innovation Leadership Achieved:**
- [ ] Voice input adopted by 5% of users
- [ ] Worktree isolation prevents project corruption
- [ ] Web UI and mobile app published
- [ ] Knowledge graph reduces hallucinations by 50%

**Business Impact:**
- [ ] Maintain market leadership
- [ ] Differentiate from competitors
- [ ] Attract early adopters

**Timeline:** Ongoing (2027+)  
**Effort:** Ongoing

---

# 7. Dependency Map

## Critical Path (Longest Dependencies)

```
Security Fixes (1.1) → Observability (1.2) → Audit Logging (1.3) → RBAC (2.3) → SOC 2 (4.1) → ISO 27001 (4.2)
```

**Critical Path Duration:** 9-12 months

## Parallel Tracks

### Track A: Enterprise Readiness
- Security (1.1) → Observability (1.2) → Audit Logging (1.3) → RBAC (2.3) → SOC 2 (4.1)

### Track B: User Experience
- First-Run Wizard (1.5) → Feature Discoverability (2.5) → Error Messages (2.5) → Documentation (2.5)

### Track C: Code Intelligence
- LSP Docs (2.1) → Codebase Indexing (2.2) → Semantic Search (2.2.2) → Knowledge Graph (5.5)

### Track D: Global Expansion
- i18n Infrastructure (3.1) → Translations (3.1.1) → RTL Support (3.1.2)

### Track E: Performance
- Async Blocking (1.6) → Cloning (3.4.1) → LSP Pooling (3.4.2)

## Feature Dependencies

| Feature | Depends On | Blocks |
|---------|-----------|--------|
| **1.1 Security Hardening** | None | 1.2, 4.1 |
| **1.2 Observability** | 1.1 | 1.3, 2.4, 4.1 |
| **1.3 Audit Logging** | 1.2 | 2.3, 4.1 |
| **1.4 Accessibility** | None | None |
| **1.5 First-Run Wizard** | 1.1.1 (Secrets) | None |
| **1.6 Performance** | None | None |
| **2.1 LSP Docs** | None | None |
| **2.2 Codebase Indexing** | None | 2.2.2, 5.5 |
| **2.3 RBAC** | 1.3 | 2.4, 4.1 |
| **2.4 Analytics** | 1.2 | None |
| **2.5 UX Improvements** | None | None |
| **3.1 i18n** | None | 3.1.2 |
| **3.2 Hooks** | None | None |
| **3.3 Vision** | None | None |
| **3.4 Performance Round 2** | 1.6 | None |
| **4.1 SOC 2** | 1.1, 1.2, 1.3, 2.3 | 4.2 |
| **4.2 ISO 27001** | 4.1 | None |
| **4.3 GDPR** | 1.3 | None |
| **5.1 Voice** | None | None |
| **5.2 Worktree** | None | None |
| **5.3 Suggested Responses** | None | None |
| **5.4 Web UI** | 2.3, 2.4 | None |
| **5.5 Knowledge Graph** | 2.2.2 | None |

---

# 8. Resource Planning

## Team Composition

### Minimum Viable Team (3 FTEs)
- **1 Senior Engineer (Security/Infrastructure)** — Phase 1 security, observability, RBAC
- **1 Senior Engineer (Product/UX)** — Phase 1-2 UX, accessibility, LSP, indexing
- **1 Mid-Level Engineer (Backend)** — Phase 2-3 RBAC, analytics, i18n, hooks

### Optimal Team (4-5 FTEs)
- **2 Senior Engineers (Security/Infrastructure)** — Parallel work on security + observability
- **1 Senior Engineer (Product/UX)** — Accessibility, UX, documentation
- **1 Mid-Level Engineer (Backend)** — RBAC, analytics, i18n
- **1 Junior Engineer (Testing/QA)** — Testing, documentation, bug fixes

### Extended Team (External)
- **Security Auditor (Contract)** — Phase 1 security review, Phase 4 SOC 2 audit
- **SOC 2 Consultant (Contract)** — Phase 4 compliance
- **Accessibility Consultant (Contract)** — Phase 1 WCAG audit
- **Technical Writer (Contract)** — Phase 2 documentation

## Effort Distribution by Phase

| Phase | Engineering Months | External Consultants | Total |
|-------|-------------------|----------------------|-------|
| **Phase 1** | 8-10 | Security Auditor (2 weeks), Accessibility Consultant (2 weeks) | 10-12 months |
| **Phase 2** | 10-12 | Technical Writer (1 month) | 11-13 months |
| **Phase 3** | 6-8 | Translation (freelance) | 6-9 months |
| **Phase 4** | 4-6 | SOC 2 Consultant (3 months), ISO Auditor (2 months) | 9-11 months |
| **Phase 5** | Ongoing | None | Ongoing |
| **Total** | 28-36 | 7-8 months | 36-45 months |

## Budget Estimate (Fully-Loaded Costs)

| Role | Rate | Phase 1 | Phase 2 | Phase 3 | Phase 4 | Total |
|------|------|---------|---------|---------|---------|-------|
| Senior Engineer | $150K/year | $100K | $120K | $60K | $40K | $320K |
| Mid-Level Engineer | $100K/year | $60K | $80K | $50K | $30K | $220K |
| Junior Engineer | $70K/year | $40K | $50K | $30K | $20K | $140K |
| Security Auditor | $10K/engagement | $10K | - | - | $10K | $20K |
| SOC 2 Consultant | $30K | - | - | - | $30K | $30K |
| Accessibility Consultant | $5K | $5K | - | - | - | $5K |
| Technical Writer | $8K/month | - | $8K | - | - | $8K |
| **Total** | - | **$215K** | **$258K** | **$140K** | **$130K** | **$743K** |

**Total Investment:** $740K-$750K over 18-24 months

---

# 9. Risk Management

## High-Risk Items

### Risk 1: Security Audit Delays
**Probability:** High  
**Impact:** High  
**Mitigation:**
- Start security hardening early (Phase 1, Week 1)
- Engage external auditor in parallel with development
- Build buffer into timeline (2-4 weeks)

### Risk 2: SOC 2 Audit Failure
**Probability:** Medium  
**Impact:** Critical  
**Mitigation:**
- Hire experienced SOC 2 consultant
- Conduct pre-audit (internal)
- Address findings before formal audit
- Allow 3-6 months observation period

### Risk 3: Accessibility Compliance Challenges
**Probability:** Medium  
**Impact:** High  
**Mitigation:**
- Engage accessibility consultant early
- User testing with blind developers
- Iterate based on feedback
- Budget extra time for accessibility (4-6 weeks)

### Risk 4: i18n Translation Quality
**Probability:** Medium  
**Impact:** Medium  
**Mitigation:**
- Native speaker review for all translations
- Professional translation service (not machine translation)
- Community feedback loop

### Risk 5: Performance Regressions
**Probability:** Medium  
**Impact:** Medium  
**Mitigation:**
- Continuous performance benchmarking in CI
- Set SLA targets (p95 <50ms)
- Alert on regressions

### Risk 6: Scope Creep
**Probability:** High  
**Impact:** Medium  
**Mitigation:**
- Strict prioritization (P0 → P1 → P2 → P3)
- Defer P3 features to Phase 5
- Regular roadmap reviews

---

# 10. Success Criteria

## Phase 1 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| **Security Vulnerabilities** | 8 critical | 0 critical | Penetration test report |
| **Enterprise Security Reviews Passed** | 0 | 5+ | Sales pipeline |
| **Observability Coverage** | 0% | 100% | Span coverage per request |
| **Cost Tracking Accuracy** | N/A | ±5% | Compare to actual API bills |
| **WCAG 2.1 AA Compliance** | Unknown | Certified | Accessibility audit |
| **Onboarding Completion Rate** | ~50% | >80% | Telemetry |
| **TUI Responsiveness (p95)** | Unknown | <50ms | Benchmarks |

## Phase 2 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| **LSP Tool Usage** | ~20% | >50% | Telemetry |
| **Codebase Indexing Adoption** | 0% | >30% | Telemetry (large projects) |
| **RBAC Deployments** | 0 | 10+ | Customer count |
| **Feature Usage Increase** | Baseline | +30% | Telemetry |
| **User Satisfaction** | Unknown | >4.5/5 | NPS survey |
| **Documentation Rating** | Unknown | 4.5/5 | User feedback |

## Phase 3 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| **Non-English Users** | <10% | >30% | Telemetry (locale) |
| **Hooks Usage** | 0% | >20% | Telemetry (advanced users) |
| **Vision Tool Usage** | <1% | >10% | Telemetry |
| **Memory Usage Reduction** | Baseline | -20% | Benchmarks |
| **LSP Latency (p95)** | Unknown | <500ms | Benchmarks |

## Phase 4 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| **SOC 2 Certification** | No | Yes | Audit report |
| **ISO 27001 Certification** | No | Yes | Certificate |
| **GDPR Compliance** | Unknown | Compliant | Legal review |
| **Regulated Industry Customers** | 0 | 5+ | Customer count |

## Phase 5 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| **Voice Input Usage** | 0% | >5% | Telemetry |
| **Web UI Adoption** | 0% | >30% | Telemetry |
| **Mobile App Downloads** | 0 | 1000+ | App stores |
| **Knowledge Graph Hallucination Reduction** | Baseline | -50% | Evaluation dataset |

---

# 11. Sequencing Rationale

## Why This Order?

### Phase 1 First: Enterprise Readiness
**Rationale:**
- **Security is non-negotiable** — Cannot sell to enterprises with critical vulnerabilities
- **Observability unlocks optimization** — Need metrics to understand usage, costs, performance
- **Accessibility is a legal requirement** — WCAG 2.1 AA compliance required by many enterprise contracts (Section 508, ADA)
- **First-run wizard reduces friction** — Onboarding is the #1 drop-off point

**Why Security Before Features:**
- Enterprise POCs blocked by security reviews
- Reputation damage from breaches far exceeds feature value
- Security fixes are riskier later (more attack surface)

### Phase 2 Second: Developer Experience
**Rationale:**
- **Competitive parity required** — LSP and indexing are table stakes vs. Aider/OpenCode
- **RBAC enables multi-user deployments** — Required for team/enterprise use cases
- **UX improvements reduce churn** — High drop-off after onboarding

**Why After Phase 1:**
- Security must be solid before adding RBAC (authentication/authorization surface)
- Observability foundation enables analytics dashboard

### Phase 3 Third: Global Expansion
**Rationale:**
- **i18n is a market multiplier** — 3x addressable market with 3 languages
- **Hooks enable customization** — Required for advanced enterprise workflows
- **Performance unlocks scale** — Memory and LSP optimization enable larger codebases

**Why After Phase 2:**
- i18n requires stable UX (less UI churn)
- Hooks benefit from mature tool ecosystem
- Performance optimization easier with established benchmarks

### Phase 4 Fourth: Compliance
**Rationale:**
- **SOC 2/ISO 27001 required for regulated industries** — Finance, healthcare, government
- **Builds on Phase 1 security** — Compliance audit verifies security implementation

**Why After Phase 3:**
- Compliance audit requires 3-6 months observation period (Phase 1-3 overlap)
- Stability required for audit (minimize code changes during observation)

### Phase 5 Last: Innovation
**Rationale:**
- **Voice, mobile, web are nice-to-have** — Not blockers for core market
- **Knowledge graph is experimental** — High effort, unproven ROI

**Why After Phase 4:**
- Foundation (security, observability, compliance) enables experimentation
- Market feedback from Phase 1-4 guides innovation priorities

---

# 12. Implementation Guidelines

## Agile Methodology

### Sprint Structure
- **Sprint length:** 2 weeks
- **Sprint planning:** Define tasks, estimate effort, assign owners
- **Daily standups:** Async (Slack updates) + sync 2x/week
- **Sprint review:** Demo completed features to stakeholders
- **Sprint retrospective:** What went well, what to improve

### Definition of Done
- [ ] Code implemented and passes linting (`cargo clippy`)
- [ ] Unit tests pass (`cargo test`)
- [ ] Integration tests pass
- [ ] Documentation updated (SPEC.md, QUICKSTART.md, docstrings)
- [ ] Manual testing completed (if UI/UX change)
- [ ] Security review (if security-sensitive)
- [ ] Approved by code review (2+ reviewers)
- [ ] Merged to main branch

### Release Cadence
- **Phase 1:** Monthly alpha releases (0.2.0-alpha.1, 0.2.0-alpha.2, etc.)
- **Phase 2:** Bi-weekly beta releases (0.3.0-beta.1, 0.3.0-beta.2, etc.)
- **Phase 3:** Monthly beta releases
- **Phase 4:** Release candidate (1.0.0-rc.1) after SOC 2
- **Phase 5:** Stable release (1.0.0) after ISO 27001

---

# 13. Milestone Tracking

## Gantt Chart (High-Level)

```
Q2 2026 (Phase 1):
├── Week 1-6:   Security Hardening
├── Week 7-12:  Observability
├── Week 13-16: Audit Logging
├── Week 17-22: Accessibility
├── Week 23-26: First-Run Wizard
└── Week 27-30: Performance

Q3 2026 (Phase 2):
├── Week 1-4:   LSP Docs
├── Week 5-12:  Codebase Indexing
├── Week 13-20: RBAC & SSO
├── Week 21-26: Analytics Dashboard
└── Week 27-36: UX Improvements

Q4 2026 (Phase 3):
├── Week 1-12:  i18n
├── Week 13-18: Hooks
├── Week 19-22: Vision
└── Week 23-26: Performance Round 2

Q1 2027 (Phase 4):
├── Week 1-8:   SOC 2 Audit
├── Week 9-12:  ISO 27001
└── Week 13-16: GDPR

2027+ (Phase 5):
├── Voice Input
├── Git Worktree Isolation
├── Suggested Responses
├── Web UI & Mobile
└── Knowledge Graph
```

---

# 14. Communication Plan

## Stakeholder Updates

### Weekly Updates (Internal)
- **To:** Engineering team, product manager
- **Format:** Written status update (Slack)
- **Content:** Completed tasks, blockers, next week's plan

### Monthly Updates (Leadership)
- **To:** CEO, CTO, board
- **Format:** Slide deck (10 slides max)
- **Content:** Phase progress, milestones achieved, risks, financials

### Quarterly Updates (Customers)
- **To:** Enterprise customers, community
- **Format:** Blog post, changelog
- **Content:** New features, roadmap updates, testimonials

## Change Management

### Roadmap Changes
- **Minor adjustments (<2 weeks):** Engineering team decision
- **Major changes (>1 month):** Require leadership approval
- **Feature cuts:** Document rationale, communicate to stakeholders

---

# 15. Conclusion

This implementation plan provides a detailed roadmap to transform ragent from a promising alpha product into an enterprise-ready, globally-accessible, certified AI coding agent over 18-24 months.

**Key Takeaways:**
1. **Security First** — Phase 1 unlocks enterprise market
2. **Iterative Delivery** — Monthly/bi-weekly releases maintain momentum
3. **Parallel Tracks** — UX, code intelligence, i18n can proceed independently
4. **Compliance as Differentiator** — SOC 2/ISO 27001 enable regulated industries
5. **Innovation Ongoing** — Phase 5 maintains competitive edge

**Next Steps:**
1. **Leadership approval** of plan and budget
2. **Hire core team** (3-4 engineers)
3. **Kick off Phase 1** (Week 1: Security audit)
4. **Establish rituals** (sprints, standups, reviews)
5. **Track metrics** (telemetry, success criteria)

**Success Indicators:**
- Phase 1 complete → 5+ enterprise POCs
- Phase 2 complete → 10+ enterprise contracts
- Phase 3 complete → 30% international users
- Phase 4 complete → SOC 2 certified, regulated industry access
- Phase 5 complete → Market leadership maintained

---

**Plan prepared by:** swarm-s6 (tm-006)  
**Sources:**
- GAP_ANALYSIS.md (swarm-s5, tm-005)
- target/temp/s1_product_analysis.md (swarm-s1, tm-001)
- COMPETITOR_ANALYSIS.md (swarm-s4, tm-004)
- security_findings.md (swarm-s2, tm-002)
- performance_findings.md (swarm-s3, tm-003)
- UX_GAPS_REPORT.md (swarm-s2, tm-002)

**Date:** March 30, 2026  
**Version:** 1.0
