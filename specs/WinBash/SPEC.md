---
status: draft
audit:
  - { time: 1780271571, from: "none", to: "draft", actor: "system" }
---
# Windows Bash Tool Support – Specification

**Spec ID:** `WinBash`

**Created:** 2026-04-23
**Author:** ragent-agent
**Version:** 1.0.0-draft

---

## Executive Summary

This specification extends ragent's `BashTool` to support the Windows operating system. On Windows, the tool shall first attempt to locate and use **Git Bash** (the POSIX-compatible shell bundled with Git for Windows). If Git Bash is unavailable, the tool shall transparently fall back to **PowerShell** as the command executor. All existing security layers (safe-command whitelist, banned commands, denied patterns, directory-escape prevention, syntax validation, obfuscation detection, and user allow/deny lists) shall remain fully functional regardless of the underlying shell.

## Scope & Objectives

### Scope

- Windows OS detection at runtime.
- Discovery of the Git Bash executable (`bash.exe`) via common installation paths and the `PATH` environment variable.
- Transparent fallback to PowerShell (`powershell.exe` / `pwsh.exe`) when Git Bash is not found.
- Adaptation of temporary-file paths, state-file paths, and shell wrapper scripts for Windows conventions.
- Preservation of per-session shell state (`cd`, `export`) across invocations on Windows.
- Adaptation of the 7-layer security model to work correctly under both Git Bash and PowerShell.

### Out of Scope

- `cmd.exe` (Command Prompt) support — PowerShell is the preferred native fallback.
- Windows-specific permission UI changes (the existing permission system is shell-agnostic).
- WSL/WSL2 integration (that is a separate, future spec).
- Automatic installation of Git for Windows or PowerShell.

### Objectives

1. Allow ragent to execute shell commands on Windows with the same UX as on Linux/macOS.
2. Prefer Git Bash for maximum POSIX compatibility of existing safe commands and scripts.
3. Ensure graceful degradation to PowerShell when Git Bash is absent.
4. Maintain the existing security posture without weakening any layer on Windows.

---

## Requirements

### FR-001 — Operating-System Detection (Ubiquitous)

`The <BashTool> shall <detect whether the current operating system is Windows at runtime using cfg!(target_os = "windows") or an equivalent platform check>.`

**Acceptance criteria:**
- On Windows, `BashTool` enters Windows execution mode.
- On non-Windows platforms, the existing Unix-style `bash -c` execution path is used unchanged.
- The detection is performed once per tool invocation (no caching required).

### FR-002 — Git Bash Discovery (Event-Driven)

`When <BashTool is in Windows execution mode>, the <BashTool> shall <search for the Git Bash executable in the following order>`:
1. The directory referenced by the `GIT_BASH` environment variable, if set.
2. Common Git for Windows installation directories:
   - `C:\Program Files\Git\bin\bash.exe`
   - `C:\Program Files\Git\usr\bin\bash.exe`
   - `C:\Program Files (x86)\Git\bin\bash.exe`
   - `C:\Program Files (x86)\Git\usr\bin\bash.exe`
3. Any `bash.exe` found in the system `PATH`.

**Acceptance criteria:**
- If an executable file exists at any checked path, that path is used as the shell binary.
- The first valid path found stops the search.
- All path existence checks are case-insensitive (Windows filesystem semantics).

### FR-003 — PowerShell Fallback (Event-Driven)

`When <Git Bash is not found by FR-002>, the <BashTool> shall <search for PowerShell in the following order>`:
1. `pwsh.exe` (PowerShell 7+) via `which` / `where` resolution.
2. `powershell.exe` (Windows PowerShell 5.1) via `which` / `where` resolution.

**Acceptance criteria:**
- If either executable is found, it becomes the shell binary for the session.
- If neither is found, the tool returns a clear error: `"No suitable shell found on Windows. Please install Git for Windows or PowerShell 7+."`.
- PowerShell execution uses the `-Command` argument (not `-File`) so that the command string is passed inline.

### FR-004 — Temporary & State File Paths (State-Driven)

`While <running on Windows>, the <BashTool> shall <store temporary script files and persistent state files in the local application data directory instead of /tmp>.`

**Acceptance criteria:**
- The base directory is `%LOCALAPPDATA%\ragent\shell\` (resolved via `std::env::var("LOCALAPPDATA")`).
- State files are named `ragent_shell_<safe-session-id>.state`.
- Temporary script files are named `ragent_cmd_<safe-session-id>_<timestamp>.ps1` when using PowerShell, or `.sh` when using Git Bash.
- All parent directories are created automatically if they do not exist.

### FR-005 — Git Bash Wrapper Script (Ubiquitous)

`The <BashTool> shall <generate a POSIX-compatible wrapper script for Git Bash on Windows that replicates the existing Unix wrapper behaviour>:`
- Source the state file if it exists.
- Restore `RAGENT_PWD` via `cd`.
- Execute the user command.
- Capture the exit code.
- Persist exported variables with `export -p`.
- Append `RAGENT_PWD=<cwd>`.
- Clean up the temporary script file.

**Acceptance criteria:**
- The wrapper uses forward slashes or properly escaped paths so that Git Bash understands them.
- The state file path is passed as an absolute Windows path (e.g., `C:/Users/…`) because Git Bash transparently translates drive-letter paths.
- Exit codes are propagated correctly to the Rust caller.

### FR-006 — PowerShell Wrapper Script (Ubiquitous)

`The <BashTool> shall <generate a PowerShell wrapper script that emulates the Unix shell-state behaviour as closely as possible>:`
- Dot-source a state script if it exists (to restore variables).
- Restore the previous working directory.
- Execute the user command via `Invoke-Expression`.
- Capture the exit code in `$LASTEXITCODE`.
- Persist environment variables by writing `Set-Variable` or `$env:` assignments to the state file.
- Append a `RAGENT_PWD=<cwd>` marker.
- Clean up the temporary script file.

**Acceptance criteria:**
- Environment variables exported during a command (e.g., `$env:FOO = "bar"`) survive into the next invocation.
- Directory changes (`cd`, `Push-Location`) survive into the next invocation.
- The state file format is stable across invocations.
- Exit codes are propagated correctly (non-zero exits are detected).

### FR-007 — Safe-Command Whitelist on Windows (Ubiquitous)

`The <BashTool> shall <apply the identical safe-command whitelist (SAFE_COMMANDS) regardless of whether Git Bash or PowerShell is the underlying executor>.`

**Acceptance criteria:**
- Commands such as `ls`, `cat`, `git`, `cargo`, etc. are auto-approved on Windows exactly as on Unix.
- PowerShell-native aliases for common POSIX commands (e.g., `dir` for `ls`, `type` for `cat`) are **not** added to the whitelist.
- The whitelist check is performed on the raw command string before any shell-specific transformation.

### FR-008 — Banned & Denied Command Enforcement (Ubiquitous)

`The <BashTool> shall <enforce banned-command and denied-pattern checks on Windows using the same lists and logic as on Unix>.`

**Acceptance criteria:**
- `curl`, `wget`, `nc`, `nmap`, etc. are rejected on Windows.
- Denied patterns such as `rm -rf /`, `:(){ :|:&};:`, and `/dev/tcp/` are still checked.
- Directory-escape prevention (`cd ..`, `cd /`, `cd ~`) is active on Windows.
- The checks run against the raw command string before it is passed to the shell.

### FR-009 — Syntax Validation on Windows (Event-Driven)

`When <Git Bash is the chosen shell>, the <BashTool> shall <run the existing `sh -n -c` syntax validation unchanged>.`

`When <PowerShell is the chosen shell>, the <BashTool> shall <skip the POSIX syntax validation and instead rely on PowerShell's own runtime error reporting>.`

**Acceptance criteria:**
- Git Bash mode: `validate_bash_syntax` is called exactly as on Unix.
- PowerShell mode: no pre-flight syntax check is performed (PowerShell parses at execution time).
- If PowerShell reports a parse error, the error message is captured in stderr and returned to the agent.

### FR-010 — YOLO Mode & User Allow/Deny Lists (State-Driven)

`While <YOLO mode is enabled or a user-defined allow/deny list entry applies>, the <BashTool> shall <honour those overrides on Windows exactly as on Unix>.`

**Acceptance criteria:**
- `ragent_config::yolo::is_enabled()` bypasses layers 2, 3, and 6 on Windows.
- `ragent_config::bash_lists::is_allowlisted(command)` exempts a command from the banned list.
- `ragent_config::bash_lists::matches_denylist(command)` blocks user-defined patterns.

### FR-011 — Process Spawn & Concurrency (Ubiquitous)

`The <BashTool> shall <acquire a process-spawn permit via `crate::resource::acquire_process_permit()` before spawning the shell process on Windows>.`

**Acceptance criteria:**
- The same concurrency-limiting mechanism is used on Windows.
- The timeout mechanism (`tokio::time::timeout`) applies to Windows shell processes.
- Process output (stdout, stderr) is captured and truncated to 100 KB as on Unix.

### FR-012 — Error Reporting & Diagnostics (Event-Driven)

`When <a shell is not found or a Windows-specific error occurs>, the <BashTool> shall <return a descriptive error message that names the missing shell and suggests remediation>.`

**Acceptance criteria:**
- Missing Git Bash → log at `info` level: `"Git Bash not found; falling back to PowerShell."`
- Missing PowerShell → error: `"No suitable shell found on Windows. Please install Git for Windows or PowerShell 7+."`
- Shell execution failures (e.g., command not found) return the shell's stderr to the agent.

### FR-013 — Obfuscation Detection (Ubiquitous)

`The <BashTool> shall <run the existing `validate_no_obfuscation` check on Windows regardless of the underlying shell>.`

**Acceptance criteria:**
- Base64-encoded pipes, `python -c exec(...)`, hex escapes, and other obfuscation techniques are rejected on Windows.
- The check is performed on the raw command string before shell dispatch.

### FR-014 — Directory-Escape Prevention on Windows (Ubiquitous)

`The <BashTool> shall <treat Windows-style absolute paths (e.g., `C:\`, `D:\`) and parent-directory references (`..`) as directory-escape attempts>.`

**Acceptance criteria:**
- `cd ..` is rejected on Windows (same as Unix).
- `cd C:\Users` is rejected on Windows (absolute path escape).
- `cd D:\project` is rejected if it is outside the working directory.
- `cd \` is rejected.
- `cd ~` is rejected (Windows user-profile directory).

---

## Non-Functional Requirements

### NFR-001 — Cross-Platform Compatibility

`The <BashTool> shall <not introduce any Unix-only dependencies or break existing Linux/macOS behaviour>.`

**Acceptance criteria:**
- All existing tests pass on Linux and macOS after the change.
- No `#[cfg(windows)]` attribute leaks into public API signatures.

### NFR-002 — Performance

`The <BashTool> shall <incur no more than 10 ms additional latency per invocation on Windows compared to Unix>.`

**Acceptance criteria:**
- Shell discovery is performed once per session (cached in `ToolContext` or `BashTool` state).
- Path existence checks use `tokio::fs::metadata` (async) to avoid blocking the executor.

### NFR-003 — Security Parity

`The <BashTool> shall <maintain the same 7-layer security model on Windows as on Unix>.`

**Acceptance criteria:**
- Every layer that is active on Unix is also active on Windows.
- No new bypass vectors are introduced by the PowerShell fallback.
- PowerShell-specific dangerous cmdlets (e.g., `Invoke-Expression` with user input, `Remove-Item -Recurse -Force C:\`) are caught by the existing denied-pattern checks.

---

## Glossary

| Term | Definition |
|---|---|
| Git Bash | The POSIX-compatible Bash shell bundled with Git for Windows, typically installed at `C:\Program Files\Git\bin\bash.exe`. |
| PowerShell | Microsoft's modern command-line shell and scripting language (`pwsh.exe` for v7+, `powershell.exe` for v5.1). |
| Windows execution mode | The code path taken by `BashTool` when `cfg!(target_os = "windows")` is true. |
| State file | The persistent file (per session) that stores exported environment variables and the current working directory between tool calls. |
