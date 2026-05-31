# Implementation Plan: Windows Bash Tool Support

**Spec ID:** `WinBash`

**Created:** 2026-04-23
**Author:** ragent-agent
**Version:** 1.0.0-draft

---

## Overview

This plan implements Windows support for ragent's `BashTool` as specified in `SPEC.md`. The approach is incremental: first detect the platform and discover available shells, then adapt file paths and wrapper scripts, and finally validate that all 7 security layers remain effective on Windows. The implementation is confined to the `ragent-tools-core` crate (specifically `src/bash.rs`) and its existing test suite.

---

## Milestones

### Milestone 1: Shell Discovery & Selection
**Deliverable:** `BashTool` can detect Windows and choose Git Bash or PowerShell.

- Implement `is_windows()` helper.
- Implement `find_git_bash()` with env-var override + known paths + PATH search.
- Implement `find_powershell()` with pwsh → powershell fallback.
- Add shell-selection caching (store chosen shell path in the session).
- Unit tests for discovery logic.

### Milestone 2: Windows Path & Wrapper Adaptation
**Deliverable:** Temporary scripts and state files live in `%LOCALAPPDATA%\ragent\shell\` and wrapper scripts work for both Git Bash and PowerShell.

- Create `windows_state_dir()` and `windows_state_file_path()` helpers.
- Update `state_file_path()` to return a Windows-friendly path on Windows.
- Implement Git Bash wrapper (POSIX-style, forward-slash paths).
- Implement PowerShell wrapper (dot-source state, `$env:` persistence, `RAGENT_PWD` marker).
- Update `execute()` to dispatch to the correct wrapper based on selected shell.

### Milestone 3: Security Layer Validation
**Deliverable:** All 7 security layers are verified on Windows.

- Ensure `is_safe_command`, `contains_banned_command`, `contains_denied_command`, `DENIED_PATTERNS`, `is_directory_escape_attempt`, `validate_no_obfuscation`, and user allow/deny lists are active.
- Adapt `is_directory_escape_attempt` to catch Windows absolute paths (`C:\`, `D:\`, `\`).
- Adapt syntax validation: run for Git Bash, skip for PowerShell.
- Add Windows-specific tests for directory escape.
- Add integration tests for PowerShell fallback behaviour.

### Milestone 4: Cross-Platform Regression Testing
**Deliverable:** Linux/macOS behaviour is unchanged; Windows tests pass in CI.

- Run full test suite on Linux to confirm zero regressions.
- Verify that `#[cfg(windows)]` blocks do not leak into public API.
- Update documentation (`docs/`) with Windows shell requirements.

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `is_windows()` platform helper | FR-001 | S | Critical | completed | — |
| T-002 | Implement Git Bash discovery (`find_git_bash`) | FR-002 | M | Critical | completed | T-001 |
| T-003 | Implement PowerShell discovery (`find_powershell`) | FR-003 | M | Critical | completed | T-001 |
| T-004 | Cache selected shell path per session | NFR-002 | S | High | completed | T-002, T-003 |
| T-005 | Create Windows state/temp directory helpers | FR-004 | S | Critical | completed | T-001 |
| T-006 | Implement Git Bash wrapper script for Windows | FR-005 | M | High | completed | T-002, T-005 |
| T-007 | Implement PowerShell wrapper script | FR-006 | M | High | completed | T-003, T-005 |
| T-008 | Adapt `execute()` dispatch for Windows shells | FR-001, FR-005, FR-006 | M | Critical | completed | T-004, T-006, T-007 |
| T-009 | Adapt syntax validation: run for Git Bash, skip for PowerShell | FR-009 | S | High | completed | T-008 |
| T-010 | Extend directory-escape detection for Windows paths | FR-014 | S | High | completed | T-008 |
| T-011 | Ensure safe-command whitelist applies unchanged | FR-007 | S | Medium | completed | T-008 |
| T-012 | Ensure banned/denied command checks apply unchanged | FR-008 | S | Medium | completed | T-008 |
| T-013 | Ensure obfuscation detection runs on Windows | FR-013 | S | Medium | completed | T-008 |
| T-014 | Ensure YOLO mode and user allow/deny lists work on Windows | FR-010 | S | Medium | completed | T-008 |
| T-015 | Improve error messages for missing shells | FR-012 | S | Medium | completed | T-003 |
| T-016 | Unit tests for `find_git_bash` and `find_powershell` | FR-002, FR-003 | M | High | completed | T-002, T-003 |
| T-017 | Unit tests for Windows wrapper script generation | FR-005, FR-006 | M | High | completed | T-006, T-007 |
| T-018 | Unit tests for Windows directory-escape prevention | FR-014 | S | High | completed | T-010 |
| T-019 | Cross-platform regression tests (Linux/macOS unchanged) | NFR-001 | M | Critical | completed | T-008 |
| T-020 | Update documentation with Windows shell prerequisites | NFR-001 | S | Low | completed | T-019 |
## Effort Legend

| Value | Meaning | Approx. Duration |
|-------|---------|------------------|
| S | Small | ≤ 2 hours |
| M | Medium | ½ – 1 day |
| L | Large | 2 – 3 days |

## Priority Legend

| Value | Meaning |
|-------|---------|
| Critical | Blocks milestone delivery; must be completed first |
| High | Required for feature completeness |
| Medium | Important for robustness or UX |
| Low | Nice-to-have or documentation |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Git Bash path varies by install type (Scoop, Chocolatey, manual) | High | Medium | T-002 searches multiple known paths and the `PATH` env var; users can override via `GIT_BASH` env var. |
| PowerShell execution policy blocks script execution | Medium | High | Document requirement to set `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser` in setup docs. |
| Windows temp-directory permissions differ from `/tmp` | Low | Medium | Use `%LOCALAPPDATA%` which is user-writable by default; create parent dirs automatically. |
| Security layer false positives on PowerShell-native syntax | Medium | Medium | T-012 adds tests with realistic PowerShell commands; adjust `DENIED_PATTERNS` only if needed. |
| CI does not run Windows tests | High | Medium | Add a Windows runner to GitHub Actions (or document manual testing steps). |

---

## Open Questions

1. Should we support `cmd.exe` as a tertiary fallback? **Decision:** No — PowerShell is the modern Windows shell and `cmd.exe` lacks the features needed for state persistence.
2. Should PowerShell commands use `Invoke-Expression` or `&` (call operator)? **Decision:** `Invoke-Expression` for inline command strings to match the existing `bash -c` pattern.
3. Do we need to escape PowerShell special characters in the wrapper script? **Decision:** Yes — use single-quoted strings where possible and escape `$` with `` ` `` in generated wrappers.