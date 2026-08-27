//! Shell command execution tool.
//!
//! Provides [`BashTool`], which runs shell commands in the agent's working
//! directory with configurable timeouts.
//!
//! On Unix systems the tool uses `bash -c` directly. On Windows it first
//! attempts to locate **Git Bash**; if that is unavailable it falls back to
//! **PowerShell** (`pwsh.exe` / `powershell.exe`). All 7 security layers
//! (safe-command whitelist, banned commands, denied patterns, directory-escape
//! prevention, syntax validation, obfuscation detection, and user allow/deny
//! lists) remain active regardless of the underlying shell.
//!
//! Shell state (current directory and exported environment variables) is
//! persisted across invocations using a per-session state file so that
//! `cd subdir` and `export FOO=bar` survive between tool calls.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Derive a filesystem-safe identifier from a session ID.
///
/// Replaces any character that is not alphanumeric or `-` with `_` so that
/// the result is safe to embed directly in a file path.
pub(crate) fn safe_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ── Platform detection ─────────────────────────────────────────────────────

/// Returns `true` when running on Windows.
#[must_use]
pub const fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

// ── Shell type ─────────────────────────────────────────────────────────────

/// The type of shell discovered on the system.
#[derive(Debug, Clone)]
enum ShellType {
    /// Standard `bash` available on Unix systems.
    Bash,
    /// Git Bash on Windows, with the resolved path to `bash.exe`.
    GitBash(PathBuf),
    /// PowerShell (`pwsh.exe` or `powershell.exe`) on Windows.
    PowerShell(PathBuf),
}

impl ShellType {
    /// Returns `true` if this is a POSIX-compatible shell (Bash or Git Bash).
    const fn is_posix(&self) -> bool {
        matches!(self, Self::Bash | Self::GitBash(_))
    }
}

// ── Shell discovery ───────────────────────────────────────────────────────

/// Well-known installation paths for Git Bash on Windows.
const GIT_BASH_KNOWN_PATHS: &[&str] = &[
    r"C:\Program Files\Git\bin\bash.exe",
    r"C:\Program Files\Git\usr\bin\bash.exe",
    r"C:\Program Files (x86)\Git\bin\bash.exe",
    r"C:\Program Files (x86)\Git\usr\bin\bash.exe",
];

/// Discover the shell to use on this system.
///
/// On Windows the search order is:
/// 1. `GIT_BASH` environment variable (if set, must point to `bash.exe`).
/// 2. Well-known Git for Windows installation directories.
/// 3. Any `bash.exe` found on the system `PATH`.
/// 4. `pwsh.exe` (PowerShell 7+) on `PATH`.
/// 5. `powershell.exe` (Windows PowerShell 5.1) on `PATH`.
///
/// On non-Windows platforms, always returns `ShellType::Bash`.
fn discover_shell() -> ShellType {
    if !is_windows() {
        return ShellType::Bash;
    }

    // 1. Check GIT_BASH env var
    if let Ok(git_bash) = std::env::var("GIT_BASH") {
        let path = PathBuf::from(git_bash);
        if path.exists() {
            tracing::info!(path = %path.display(), "Using Git Bash from GIT_BASH env var");
            return ShellType::GitBash(path);
        }
        tracing::warn!(
            path = %path.display(),
            "GIT_BASH env var set but path does not exist, continuing search"
        );
    }

    // 2. Check well-known paths
    for known in GIT_BASH_KNOWN_PATHS {
        let path = PathBuf::from(known);
        if path.exists() {
            tracing::info!(path = %path.display(), "Found Git Bash at known location");
            return ShellType::GitBash(path);
        }
    }

    // 3. Search PATH for bash.exe
    if let Ok(path) = which::which("bash") {
        tracing::info!(path = %path.display(), "Found bash on PATH");
        return ShellType::GitBash(path);
    }

    // 4. Try pwsh.exe (PowerShell 7+)
    if let Ok(path) = which::which("pwsh") {
        tracing::info!(path = %path.display(), "Git Bash not found; falling back to PowerShell 7+");
        return ShellType::PowerShell(path);
    }

    // 5. Try powershell.exe (Windows PowerShell 5.1)
    if let Ok(path) = which::which("powershell") {
        tracing::info!(
            path = %path.display(),
            "Git Bash not found; falling back to Windows PowerShell 5.1"
        );
        return ShellType::PowerShell(path);
    }

    // No shell found — return Bash anyway so that execute() can produce a
    // clear error message.
    tracing::error!("No suitable shell found on Windows");
    ShellType::Bash
}

/// Global cache for the discovered shell type. The shell is discovered once
/// per process and reused for all subsequent invocations.
static SHELL_CACHE: OnceLock<ShellType> = OnceLock::new();

/// Return the cached shell type, discovering it on the first call.
fn get_shell() -> &'static ShellType {
    SHELL_CACHE.get_or_init(discover_shell)
}

// ── Windows state/temp directory helpers ───────────────────────────────────

/// Return the base directory for ragent shell state/temp files on Windows.
///
/// Uses `%LOCALAPPDATA%\ragent\shell\` on Windows and `/tmp` on Unix.
/// Creates the directory (and parents) if it does not exist.
fn windows_state_dir() -> Result<PathBuf> {
    let base = std::env::var("LOCALAPPDATA").map_or_else(
        |_| {
            // Fallback: use HOME/.local/share on Unix, or USERPROFILE on Windows
            dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
        },
        PathBuf::from,
    );
    let dir = base.join("ragent").join("shell");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create state directory {}", dir.display()))?;
    Ok(dir)
}

/// Return the path of the persistent state file for the given session.
///
/// On Unix: `/tmp/ragent_shell_<session_id>.state`
/// On Windows: `%LOCALAPPDATA%\ragent\shell\ragent_shell_<session_id>.state`
#[must_use]
pub fn state_file_path(session_id: &str) -> String {
    if is_windows() {
        // On Windows we compute the path dynamically; errors are handled in execute()
        match windows_state_dir() {
            Ok(dir) => dir
                .join(format!(
                    "ragent_shell_{}.state",
                    safe_session_id(session_id)
                ))
                .to_string_lossy()
                .into_owned(),
            Err(_) => format!("/tmp/ragent_shell_{}.state", safe_session_id(session_id)),
        }
    } else {
        format!("/tmp/ragent_shell_{}.state", safe_session_id(session_id))
    }
}

/// Return a temporary script file path appropriate for the current shell type.
///
/// On Unix: `/tmp/ragent_cmd_<session_id>_<timestamp>.sh`
/// On Windows Git Bash: `%LOCALAPPDATA%\ragent\shell\ragent_cmd_<session_id>_<timestamp>.sh`
/// On Windows PowerShell: `%LOCALAPPDATA%\ragent\shell\ragent_cmd_<session_id>_<timestamp>.ps1`
fn script_file_path(session_id: &str, shell: &ShellType) -> Result<String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();

    let ext = match shell {
        ShellType::Bash | ShellType::GitBash(_) => "sh",
        ShellType::PowerShell(_) => "ps1",
    };

    let name = format!(
        "ragent_cmd_{}_{}.{}",
        safe_session_id(session_id),
        timestamp,
        ext
    );

    if is_windows() {
        let dir = windows_state_dir()?;
        Ok(dir.join(name).to_string_lossy().into_owned())
    } else {
        Ok(format!("/tmp/{name}"))
    }
}

/// Convert a Windows backslash path to forward slashes for Git Bash.
///
/// Git Bash can handle `C:/Users/...` but backslashes in wrapper scripts
/// are interpreted as escape characters by bash.
fn to_posix_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Escape a string for safe embedding in a POSIX single-quoted string literal.
///
/// The result can be placed between single quotes in a shell script without
/// allowing metacharacters or quote injection to alter the surrounding syntax.
pub(crate) fn sh_quote_single(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escape a string for safe embedding in a PowerShell single-quoted string literal.
///
/// In PowerShell single-quoted strings, the only special character is the
/// single quote itself, which must be doubled to escape it.
pub(crate) fn ps_quote_single(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ── State parsing ──────────────────────────────────────────────────────────

/// Parse the current working directory from a state file's contents.
///
/// The state file may contain lines written by `export -p` (e.g.
/// `declare -x PWD="/some/dir"`) **and** an explicit trailing line in the
/// form `RAGENT_PWD=/some/dir` which we prefer because it is unambiguous.
fn parse_cwd_from_state(state: &str) -> Option<String> {
    // Prefer the explicit marker we append after every command.
    for line in state.lines().rev() {
        if let Some(val) = line.strip_prefix("RAGENT_PWD=") {
            let v = val.trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

// ── BashTool struct ────────────────────────────────────────────────────────

/// Executes shell commands and returns combined stdout/stderr output.
///
/// On Unix, uses `bash -c`. On Windows, uses Git Bash (preferred) or
/// PowerShell as a fallback. Output is truncated to ~30 KB to avoid
/// overwhelming the agent context. Commands that exceed the configured
/// timeout (default 120 s) are terminated.
pub struct BashTool;

const DEFAULT_TIMEOUT_SECS: u64 = 120;

// Safe commands: only these exact prefixes are auto-approved without user prompting.
// The check is prefix-based: a command is safe if it equals the entry exactly OR starts
// with the entry followed by a space (so "ls" matches "ls -la", "git" matches "git status", etc.).
const SAFE_COMMANDS: &[&str] = &[
    // --- File management ---
    "ls",
    "cd",
    "pwd",
    "mkdir",
    "touch",
    "cp",
    "mv",
    // NOTE: "rm" is intentionally excluded — prefix matching cannot distinguish
    // safe "rm file.txt" from destructive "rm -rf /". DENIED_PATTERNS blocks the
    // destructive variants; individual rm calls go through normal permission flow.
    // --- File reading & search ---
    "cat",
    "head",
    "tail",
    "grep",
    "egrep",
    "fgrep",
    "find",
    "rg", // ripgrep
    "wc",
    // --- Version control ---
    "git", // covers all git subcommands (clone, add, commit, push, pull, status, diff, log …)
    "gh",  // GitHub CLI
    // --- Build / package management ---
    "cargo",
    "rustc",
    "rustfmt",
    "clippy-driver",
    "npm",
    "yarn",
    "pnpm",
    "node",
    "npx",
    "python3",
    "python",
    "pip",
    "pip3",
    "make",
    "docker-compose",
    // --- Text / data utilities ---
    "echo",
    "printf",
    "chmod",
    "jq", // JSON query/processing
    "yq", // YAML query/processing
    "sed",
    "awk",
    "sort",
    "uniq",
    "cut",
    "tr",
    "xargs",
    "date",
    "which",
    "tree",
    "diff",
    "patch",
];

// Banned commands: these are never allowed (unless YOLO mode enabled).
// High-risk tools that could exfiltrate data or connect to external systems.
const BANNED_COMMANDS: &[&str] = &[
    "curl",
    "wget",
    "nc",
    "netcat",
    "telnet",
    "axel",
    "aria2c",
    "lynx",
    "w3m",
    // Attack and exploitation tools
    "nmap",
    "masscan",
    "nikto",
    "sqlmap",
    "hydra",
    "john",
    "hashcat",
    "aircrack",
    "metasploit",
    "msfconsole",
    "msfvenom",
    "burpsuite",
    "ettercap",
    "arpspoof",
    // tcpdump and wireshark are blocked by default but can be enabled via YOLO mode
    "tcpdump",
    "wireshark",
];

// Denied patterns that require word-boundary matching (bare command names).
// These are checked with the same boundary logic as BANNED_COMMANDS to avoid
// false positives (e.g., "mkfs" should not match "wmkfs").
const DENIED_COMMANDS: &[&str] = &[
    // Disk / partition destruction
    "mkfs",
    "wipefs",
    // Kernel modifications
    "insmod",
    // User/group manipulation
    "useradd",
    "usermod",
    "groupadd",
    // System configuration
    "visudo",
    // Boot/firmware
    "grub-install",
    "efibootmgr",
];

// Denied patterns that represent command invocations with specific arguments.
// These should be checked against extracted command tokens, not as arbitrary substrings,
// to avoid false positives (e.g., "sudo " should not match "visudo ").
const DENIED_COMMAND_PATTERNS: &[&str] = &[
    // Privilege escalation commands
    "sudo ",
    "sudo\t",
    "su -",
    "su root",
    "doas ",
    // User/group manipulation
    "passwd ",
    // System configuration
    "crontab -",
];

// Denied patterns that use substring matching (composite patterns, arguments, paths).
// These remain as simple substring checks because they match specific dangerous
// command structures, not just command names.
const DENIED_PATTERNS: &[&str] = &[
    // Destructive filesystem operations
    "rm -rf /",
    "rm -r -f /",
    "rm -fr /",
    "rm -Rf /",
    "rmdir /",
    // Disk operations
    "dd if=",
    "shred /dev",
    // Device writes
    "> /dev/sd",
    "> /dev/nvme",
    "> /dev/vd",
    // Fork bomb
    ":(){ :|:&};:",
    // Privilege escalation
    "chmod -R 777 /",
    "chown -R",
    // Network exfiltration of sensitive files
    "curl.*etc/shadow",
    "wget.*etc/shadow",
    // History / credential file theft
    ".bash_history",
    ".ssh/id_",
    // Kernel modifications
    "modprobe -r",
    "sysctl -w",
    // Destructive git operations
    "git push --force",
    "git push -f ",
    "git push origin --delete",
    // More destructive patterns
    "rm -rf ~",
    "rm -rf $HOME",
    "rm -rf .",
    "chmod 000 /",
    "chmod -R 000",
    // Data exfiltration via pipes
    "> /dev/tcp",
    "bash -i >&",
    "/dev/tcp/",
    "/dev/udp/",
    "systemctl disable",
    "systemctl mask",
    "chattr +i",
];

/// Check if command is in the safe whitelist (exact match or with allowed args).
#[must_use]
pub fn is_safe_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    SAFE_COMMANDS.iter().any(|safe| {
        trimmed == *safe
            || trimmed
                .strip_prefix(safe)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// Returns the built-in safe commands allowlist.
///
/// These commands are auto-approved without user prompting (Layer 1).
#[must_use]
pub fn get_safe_commands() -> Vec<&'static str> {
    SAFE_COMMANDS.to_vec()
}

/// Returns the built-in banned commands, denied commands, denied command patterns, and denied patterns.
///
/// Used by the TUI to display the complete security policy in `/bash show`.
/// Returns: (`banned_commands`, `denied_commands`, `denied_command_patterns`, `denied_patterns`)
#[must_use]
pub fn get_builtin_lists() -> (
    Vec<&'static str>,
    Vec<&'static str>,
    Vec<&'static str>,
    Vec<&'static str>,
) {
    (
        BANNED_COMMANDS.to_vec(),
        DENIED_COMMANDS.to_vec(),
        DENIED_COMMAND_PATTERNS.to_vec(),
        DENIED_PATTERNS.to_vec(),
    )
}

/// Extract the bare heredoc delimiter from a line that contains `<<`.
///
/// Handles `<<EOF`, `<< EOF`, `<<'EOF'`, `<<"EOF"`, and `<<-EOF` variants.
/// Returns `None` if no heredoc marker is found.
fn extract_heredoc_delimiter(line: &str) -> Option<String> {
    let pos = line.find("<<")?;
    // <<- is allowed (strip leading tabs from body); skip the optional '-'
    let rest = line[pos + 2..].trim_start_matches('-').trim_start();
    let delimiter = if let Some(inner) = rest.strip_prefix('\'') {
        inner.split('\'').next()?.to_string()
    } else if let Some(inner) = rest.strip_prefix('"') {
        inner.split('"').next()?.to_string()
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | ')'))
            .unwrap_or(rest.len());
        rest[..end].to_string()
    };
    if delimiter.is_empty() {
        None
    } else {
        Some(delimiter)
    }
}

/// Return a copy of `cmd` with heredoc bodies removed.
///
/// The line containing the `<<` marker and the closing delimiter line are
/// kept so that the structural shell command is still present for subsequent
/// checks; only the body lines (the literal data) are dropped.  This
/// prevents heredoc content (e.g. Rust string literals containing `\nc\n`)
/// from producing false positives in the banned-command scan.
fn strip_heredoc_bodies(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    let mut iter = cmd.split('\n');
    'outer: while let Some(line) = iter.next() {
        if let Some(delimiter) = extract_heredoc_delimiter(line) {
            result.push_str(line);
            result.push('\n');
            // Skip body lines until the closing delimiter.
            for body_line in iter.by_ref() {
                if body_line.trim_end() == delimiter {
                    result.push_str(body_line);
                    result.push('\n');
                    continue 'outer;
                }
                // body content intentionally omitted
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Extract command names from a shell command string.
///
/// Splits on shell operators (|, ;, &&, ||, &, newline) and extracts the first
/// token after each operator (or at the start). Returns a list of command names.
///
/// Examples:
/// - "mkfs /dev/sda" → ["mkfs"]
/// - "ls | grep foo" → ["ls", "grep"]
/// - "cd tmp && mkfs" → ["cd", "mkfs"]
fn extract_command_names(cmd: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut in_command = true;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '|' | ';' | '&' | '\n' => {
                // Shell operator - extract command before it
                if in_command && !current.is_empty() {
                    commands.push(current.trim().to_string());
                }
                current.clear();
                in_command = true;
            }
            ' ' | '\t' => {
                // Whitespace - end of command name
                if in_command && !current.is_empty() {
                    commands.push(current.trim().to_string());
                    current.clear();
                    in_command = false;
                }
            }
            '\'' => {
                // Single-quoted string - skip until closing quote
                for qc in chars.by_ref() {
                    if qc == '\'' {
                        break;
                    }
                }
                in_command = false;
            }
            '"' => {
                // Double-quoted string - skip until closing quote (handle escapes)
                while let Some(qc) = chars.next() {
                    if qc == '\\' {
                        chars.next(); // skip escaped char
                    } else if qc == '"' {
                        break;
                    }
                }
                in_command = false;
            }
            _ => {
                if in_command {
                    current.push(c);
                }
            }
        }
    }

    // Add final command if present
    if in_command && !current.is_empty() {
        commands.push(current.trim().to_string());
    }

    commands
}

/// Check if command contains a denied command name with word-boundary matching.
///
/// Unlike `contains_banned_command()` which scans the entire command text,
/// this extracts actual command names (first token after shell operators)
/// and checks only those. This prevents false positives from command names
/// appearing in arguments or strings.
fn contains_denied_command(cmd: &str) -> bool {
    let cmd_stripped = strip_heredoc_bodies(cmd);
    let command_names = extract_command_names(&cmd_stripped);

    for cmd_name in command_names {
        let cmd_lower = cmd_name.to_lowercase();

        // Check denied command names (bare commands like "mkfs", "insmod")
        for denied in DENIED_COMMANDS {
            // Check if the command name exactly matches the denied command
            // or if it starts with the denied command followed by a non-alphanumeric
            // character (e.g., "mkfs.ext4" matches "mkfs")
            if cmd_lower == *denied {
                return true;
            }
            if let Some(rest) = cmd_lower.strip_prefix(denied)
                && let Some(first_char) = rest.chars().next()
                && !first_char.is_alphanumeric()
                && first_char != '_'
                && first_char != '-'
            {
                return true;
            }
        }

        // Check denied command patterns (commands with specific args like "sudo ", "su -")
        for pattern in DENIED_COMMAND_PATTERNS {
            // For patterns with trailing space, match command name with space
            // For patterns with arg (e.g., "su -"), match the full pattern
            if pattern.ends_with(' ') {
                let pattern_cmd = pattern.trim_end();
                if cmd_lower == pattern_cmd {
                    return true;
                }
            } else {
                // Pattern includes argument (e.g., "su -", "crontab -")
                // Reconstruct the command with potential arguments from original command
                if cmd_stripped.to_lowercase().contains(pattern) {
                    // Additional check: ensure it's a command invocation, not in a string
                    // This is a simplified check - full parsing would be complex
                    return true;
                }
            }
        }
    }

    false
}

/// Check if command uses a banned tool (e.g., curl, wget).
fn contains_banned_command(cmd: &str) -> bool {
    // Strip heredoc bodies first so that literal data inside a heredoc
    // (e.g. Rust string escapes like `\nc\n`) cannot trigger false positives.
    let cmd_stripped = strip_heredoc_bodies(cmd);
    let cmd_lower = cmd_stripped.trim().to_lowercase();
    let bytes = cmd_lower.as_bytes();
    let clen = bytes.len();

    BANNED_COMMANDS.iter().any(|banned| {
        let banned_bytes = banned.as_bytes();
        let blen = banned_bytes.len();
        if clen < blen {
            return false;
        }
        // Require word boundaries: banned name must not be part of a longer identifier.
        // Characters that delimit command tokens: whitespace, |, ;, &, (, ), `, ', "
        let is_boundary = |b: u8| !b.is_ascii_alphanumeric() && b != b'_' && b != b'-';
        for i in 0..=(clen - blen) {
            if &bytes[i..i + blen] == banned_bytes {
                let before_ok = i == 0 || is_boundary(bytes[i - 1]);
                let after_ok = i + blen == clen || is_boundary(bytes[i + blen]);
                if before_ok && after_ok {
                    return true;
                }
            }
        }
        false
    })
}

/// Check if command tries to escape the working directory.
///
/// Rejects `cd`/`pushd` with `..`, `/`, `~`, `$HOME`, `${HOME}`, and on
/// Windows also rejects absolute paths like `C:\` or `\`.
fn is_directory_escape_attempt(cmd: &str, working_dir: &std::path::Path) -> bool {
    is_directory_escape_attempt_inner(cmd, working_dir, is_windows())
}

/// Inner implementation that accepts an explicit `on_windows` flag for testing.
fn is_directory_escape_attempt_inner(
    cmd: &str,
    working_dir: &std::path::Path,
    on_windows: bool,
) -> bool {
    let canonical_wd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    for token in &["cd ", "pushd "] {
        // Find each occurrence of the token in the command
        let mut search_start = 0;
        while let Some(pos) = cmd[search_start..].find(token) {
            let abs_pos = search_start + pos;
            // Only treat it as a cd if it's at the start or after a shell separator
            let before = if abs_pos == 0 {
                b';'
            } else {
                cmd.as_bytes()[abs_pos - 1]
            };
            let is_after_separator = matches!(before, b';' | b'&' | b'|' | b'(' | b'\n' | b' ');
            if abs_pos == 0 || is_after_separator {
                let arg_start = abs_pos + token.len();
                // Extract the argument (up to next whitespace or ; & | )
                let arg = cmd[arg_start..]
                    .split([';', '&', '|', ')', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim();

                if arg.starts_with("..") {
                    return true;
                }
                if arg.starts_with('~') || arg.starts_with("$HOME") || arg.starts_with("${HOME}") {
                    return true;
                }
                if arg.starts_with('/') {
                    // Single-segment slash-prefixed tokens (e.g., /help, /start)
                    // are likely commands, not file paths - exclude from escape check.
                    if arg.len() > 1 && !arg.strip_prefix('/').unwrap_or(arg).contains('/') {
                        // Single segment after / - treat as command, not a file path
                        continue;
                    }
                    // Allow if the absolute path resolves to the working directory or a subdirectory
                    let target = std::path::Path::new(arg);
                    let canonical_target = target
                        .canonicalize()
                        .unwrap_or_else(|_| target.to_path_buf());
                    if !canonical_target.starts_with(&canonical_wd) {
                        return true;
                    }
                }

                // Windows-specific: reject absolute Windows paths and bare backslash
                if on_windows {
                    // Match drive-letter paths: C:\, D:\, etc.
                    let arg_bytes = arg.as_bytes();
                    if arg_bytes.len() >= 2
                        && arg_bytes[0].is_ascii_alphabetic()
                        && arg_bytes[1] == b':'
                    {
                        // This is a Windows absolute path like C:\Users — reject it
                        return true;
                    }
                    // Match bare backslash: \ (root of current drive)
                    if arg.starts_with('\\') {
                        return true;
                    }
                }
            }
            search_start = abs_pos + 1;
        }
    }
    false
}

/// Pre-check command syntax without executing.
///
/// On Unix, uses `bash -n -c`. On Windows with Git Bash, uses the discovered
/// Git Bash executable with `-n -c`. On Windows with PowerShell, this check
/// is skipped entirely because PowerShell has its own runtime parser.
///
/// Returns error if syntax is invalid or the shell program cannot be found.
async fn validate_bash_syntax(cmd: &str) -> Result<()> {
    let shell = get_shell();

    // Skip syntax validation when using PowerShell (PowerShell has its own
    // parser and `-n` is a POSIX-shell-only concept).
    if !shell.is_posix() {
        return Ok(());
    }

    // Use the actual discovered shell program for syntax checking rather than
    // a hardcoded "sh" which may not exist on Windows (or may be a different
    // shell like dash on some Linux systems).
    let (program, args): (&OsStr, Vec<&OsStr>) = match shell {
        ShellType::Bash => (
            OsStr::new("bash"),
            vec![OsStr::new("-n"), OsStr::new("-c"), OsStr::new(cmd)],
        ),
        ShellType::GitBash(path) => (
            path.as_os_str(),
            vec![OsStr::new("-n"), OsStr::new("-c"), OsStr::new(cmd)],
        ),
        ShellType::PowerShell(_) => unreachable!("PowerShell handled above"),
    };

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        Command::new(program)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                bail!("Bash syntax error: {stderr}");
            }
            Ok(())
        }
        Ok(Err(e)) => bail!("Failed to check bash syntax: {e}"),
        Err(_) => bail!("Bash syntax check timed out"),
    }
}

// ── Wrapper script generation ──────────────────────────────────────────────

/// Build the wrapper script for a POSIX-compatible shell (Bash or Git Bash).
///
/// The wrapper:
/// 1. Sources the state file (restoring env vars and `RAGENT_PWD`).
/// 2. Runs the user command from the temporary script file.
/// 3. Saves exported variables via `export -p`.
/// 4. Appends `RAGENT_PWD=<cwd>` as an unambiguous marker.
/// 5. Cleans up the temporary script file.
pub(crate) fn build_posix_wrapper(state_file: &str, script_file: &str) -> String {
    // Use forward slashes even on Windows (Git Bash understands them)
    let state_file_posix = if is_windows() {
        to_posix_path(state_file)
    } else {
        state_file.to_string()
    };
    let script_file_posix = if is_windows() {
        to_posix_path(script_file)
    } else {
        script_file.to_string()
    };

    // Escape the file paths so malicious characters in session IDs or temp
    // directories cannot break out of the generated wrapper script.
    let state_quoted = sh_quote_single(&state_file_posix);
    let script_quoted = sh_quote_single(&script_file_posix);

    format!(
        "STATE_FILE={state_quoted}\n\
         SCRIPT_FILE={script_quoted}\n\
         if [ -f \"$STATE_FILE\" ]; then\n\
           . \"$STATE_FILE\" 2>/dev/null\n\
           cd \"${{RAGENT_PWD:-}}\" 2>/dev/null || true\n\
         fi\n\
         bash \"$SCRIPT_FILE\"\n\
         EXIT_CODE=$?\n\
         export -p 2>/dev/null > \"$STATE_FILE\" || true\n\
         printf 'RAGENT_PWD=%s\\n' \"$(pwd)\" >> \"$STATE_FILE\"\n\
         rm -f \"$SCRIPT_FILE\"\n\
         exit $EXIT_CODE\n"
    )
}

/// Build the wrapper script for PowerShell.
///
/// The wrapper:
/// 1. Dot-sources the state script if it exists (restoring `$env:` variables and `RAGENT_PWD`).
/// 2. Changes to the saved directory.
/// 3. Executes the user command via `Invoke-Expression`.
/// 4. Persists user-set environment variables to the state script.
/// 5. Appends the `RAGENT_PWD` marker.
/// 6. Cleans up the temporary script file.
pub(crate) fn build_powershell_wrapper(state_file: &str, script_file: &str) -> String {
    // Escape the file paths so malicious characters in session IDs or temp
    // directories cannot break out of the generated wrapper script.
    let state_quoted = ps_quote_single(state_file);
    let script_quoted = ps_quote_single(script_file);

    format!(
        "$ErrorActionPreference = 'Continue'\n\
         $StateFile = {state_quoted}\n\
         if (Test-Path $StateFile) {{ . $StateFile }}\n\
         if ($env:RAGENT_PWD) {{ Set-Location $env:RAGENT_PWD }}\n\
         $UserCmd = Get-Content -Raw {script_quoted}\n\
         try {{\n\
           Invoke-Expression $UserCmd\n\
         }} finally {{\n\
           $exitCode = $LASTEXITCODE\n\
           # Persist environment variables set during the session\n\
           $envLines = @()\n\
           Get-ChildItem Env: | ForEach-Object {{\n\
             $envLines += \"Set-Item -Path 'Env:\\`\" + $_.Key + \"\\`' -Value '\\`\" + $_.Value.Replace(\"'\", \"''\") + \"\\`'\"\n\
           }}\n\
           $envLines += \"Set-Location -Path '\\`\" + (Get-Location).Path + \"\\`'\"\n\
           $envLines += 'RAGENT_PWD=' + (Get-Location).Path\n\
           Set-Content -Path $StateFile -Value $envLines\n\
           Remove-Item -Force {script_quoted} -ErrorAction SilentlyContinue\n\
           exit $exitCode\n\
         }}\n"
    )
}

/// Validate a shell command against all background-task security layers.
///
/// This is a stripped-down version of [`BashTool::execute`] that performs
/// the same banned/denied/directory-escape/syntax/obfuscation checks without
/// executing the command. It is used by the `bg` background task manager
/// before spawning a long-running process.
pub async fn validate_shell_command(command: &str, working_dir: &std::path::Path) -> Result<()> {
    let shell = get_shell();

    if is_windows() && matches!(shell, ShellType::Bash) {
        bail!(
            "No suitable shell found on Windows.              Please install Git for Windows or PowerShell 7+."
        );
    }

    if is_safe_command(command) {
        tracing::info!("Safe bash command auto-approved");
    }

    if contains_banned_command(command) {
        if ragent_config::yolo::is_enabled() {
            tracing::warn!("YOLO mode: allowing banned command tool");
        } else if ragent_config::bash_lists::is_allowlisted(command) {
            tracing::info!("Banned command allowed by user allowlist");
        } else {
            bail!(
                "Command rejected: uses banned external tool (curl, wget, nc, telnet, axel, aria2c, lynx, w3m).                  These tools could exfiltrate data or connect to external systems."
            );
        }
    }

    if is_directory_escape_attempt(command, working_dir) {
        bail!(
            "Command rejected: attempts to escape working directory {}.              Use only relative paths (cd ./subdir, cd subdir).",
            working_dir.display()
        );
    }

    validate_bash_syntax(command).await?;

    if contains_denied_command(command) {
        if ragent_config::yolo::is_enabled() {
            tracing::warn!("YOLO mode: allowing denied command name");
        } else {
            bail!(
                "Command rejected: uses dangerous command (mkfs, insmod, useradd, etc.).                  These commands could cause irreversible damage to the system."
            );
        }
    }

    for pattern in DENIED_PATTERNS {
        if command.contains(pattern) {
            if ragent_config::yolo::is_enabled() {
                tracing::warn!(pattern, "YOLO mode: allowing denied pattern");
            } else {
                bail!(
                    "Command rejected: contains dangerous pattern '{pattern}'. This pattern could cause irreversible damage to the system."
                );
            }
        }
    }

    if !ragent_config::yolo::is_enabled()
        && let Some(pattern) = ragent_config::bash_lists::matches_denylist(command)
    {
        bail!(
            "Command rejected: matches user-defined deny pattern '{pattern}'.                 Use `/bash remove deny \"{pattern}\"` to remove this restriction."
        );
    }

    if !ragent_config::yolo::is_enabled() {
        validate_no_obfuscation(command)?;
    }

    Ok(())
}

/// Build the argv prefix that runs a shell command at low CPU/IO priority so
/// heavy agent workloads do not make the host system unresponsive.
///
/// Returns `["nice", "-n", "<level>", "ionice", "-c", "3"]` on Linux (or
/// `["nice", "-n", "<level>"]` elsewhere) when a `nice` level is configured via
/// `bash.nice` in `ragent.json` (default `10`). Returns an empty vector when no
/// level is configured or on Windows (the wrappers are POSIX utilities).
///
/// Callers prepend the returned argv to their program invocation so the actual
/// program runs as a child of `nice`, lowering its CPU scheduling priority, and
/// (on Linux) of `ionice -c 3`, marking its I/O as idle-class.
fn low_priority_prefix(shell: &ShellType) -> Vec<std::ffi::OsString> {
    // POSIX wrappers only; Git Bash and PowerShell on Windows also understand
    // them but the shell executable may not be a standard `nice`/`ionice`, so
    // restrict the wrappers to native POSIX shells.
    if matches!(shell, ShellType::PowerShell(_) | ShellType::GitBash(_)) || is_windows() {
        return Vec::new();
    }

    let Some(level) = ragent_config::bash_lists::nice_level() else {
        return Vec::new();
    };

    let mut prefix: Vec<std::ffi::OsString> =
        vec!["nice".into(), "-n".into(), level.to_string().into()];
    if cfg!(target_os = "linux") {
        prefix.push("ionice".into());
        prefix.push("-c".into());
        prefix.push("3".into());
    }
    prefix
}

/// Prepend the low-priority argv prefix (see [`low_priority_prefix`]) to a
/// command so the real program runs as a child of `nice` / `ionice`.
///
/// This rewrites `cmd` to run `nice ... <original program> <original args>`.
/// `kill_on_drop` is carried over from the original command. Callers should
/// invoke this immediately after `Command::new(..)` + program/args and BEFORE
/// configuring `current_dir`/stdio/env: the rebuild replaces the command
/// object wholesale, so anything configured before this call is lost.
/// Configuration applied after this call survives untouched.
fn prepend_low_priority(cmd: &mut Command, shell: &ShellType) {
    let prefix = low_priority_prefix(shell);
    if prefix.is_empty() {
        return;
    }

    let original_program = cmd.as_std().get_program().to_os_string();
    let original_args: Vec<std::ffi::OsString> = cmd.as_std().get_args().map(Into::into).collect();
    let kill_on_drop = cmd.get_kill_on_drop();

    let Some((prog, rest)) = prefix.split_first() else {
        return;
    };
    let mut rebuilt = Command::new(prog);
    rebuilt.args(rest);
    rebuilt.args([original_program]);
    rebuilt.args(original_args);
    rebuilt.kill_on_drop(kill_on_drop);

    *cmd = rebuilt;
}

/// Build the argv + wrapper-script invocation for a shell, configured with the
/// low-priority prefix (when enabled), the working directory, null stdin, and
/// piped stdout/stderr. Callers then add the env vars / timeout / output as
/// appropriate.
///
/// `wrapper` is the shell script that carries the user command plus any
/// persistent-shell bookkeeping. For Git Bash and PowerShell this is a
/// `.sh`/`.ps1` file written earlier; for native Bash it is also a script.
fn build_shell_command(shell: &ShellType, wrapper: &str, working_dir: &std::path::Path) -> Command {
    let mut cmd = match shell {
        ShellType::Bash => {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(wrapper);
            cmd
        }
        ShellType::GitBash(path) => {
            let mut cmd = Command::new(path);
            cmd.arg("-c").arg(wrapper);
            cmd
        }
        ShellType::PowerShell(path) => {
            let mut cmd = Command::new(path);
            cmd.arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(wrapper);
            cmd
        }
    };
    // Apply the low-priority wrapper BEFORE configuring cwd/stdio: the wrapper
    // rebuilds the command object wholesale, so anything configured before this
    // call would be discarded.
    prepend_low_priority(&mut cmd, shell);
    cmd.current_dir(working_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    cmd
}

/// Truncate very long command output, keeping the first 15k + last 15k chars
/// (CC1-T7). Returns the input unchanged when it fits within the budget.
fn truncate_output(content: String) -> String {
    const FIRST_CHARS: usize = 15_000;
    const LAST_CHARS: usize = 15_000;
    const MAX_OUTPUT: usize = FIRST_CHARS + LAST_CHARS + 1000; // allow for separator

    if content.len() <= MAX_OUTPUT {
        return content;
    }

    // Find valid UTF-8 char boundaries near the target split points.
    let first_end = {
        let mut i = FIRST_CHARS.min(content.len());
        while i > 0 && !content.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let first_part = &content[..first_end];
    let remainder_len = content.len() - first_end;
    let last_part = if remainder_len > LAST_CHARS {
        let mut j = content.len() - LAST_CHARS;
        while j < content.len() && !content.is_char_boundary(j) {
            j += 1;
        }
        &content[j..]
    } else {
        &content[first_end..]
    };

    let omitted = remainder_len.saturating_sub(LAST_CHARS);
    format!(
        "{}\n\n... ({} lines omitted) ...\n\n{}",
        first_part,
        omitted / content.lines().count().max(1), // rough line count
        last_part
    )
}

/// Spawn a long-running shell command for background execution.
///
/// The returned [`tokio::process::Child`] has stdin closed, stdout/stderr
/// piped, and `kill_on_drop(true)` so dropping the handle terminates the
/// process. The command is validated through [`validate_shell_command`] before
/// spawning.
pub async fn spawn_background_shell(
    command: &str,
    working_dir: &std::path::Path,
) -> Result<tokio::process::Child> {
    validate_shell_command(command, working_dir).await?;
    let _permit = crate::resource::acquire_process_permit().await?;
    let shell = get_shell();
    let mut cmd = build_shell_command(shell, command, working_dir);
    Ok(cmd.spawn()?)
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Execute a shell command and return its stdout and stderr. Commands are \
         run in the agent's working directory. Required parameter: `command` \
         (string). Optional: `timeout` in seconds (integer, default 120). The \
         command is validated by a 7-layer security model: safe-command \
         whitelist, banned-command checks, denied patterns, directory-escape \
         prevention, syntax validation, obfuscation detection, and user \
         allowlist/denylist. Long-running or interactive commands may need a \
         higher timeout."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "REQUIRED. Shell command to execute."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)"
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    /// Executes a shell command.
    ///
    /// On Unix, uses `bash -c`. On Windows, uses Git Bash (preferred) or
    /// PowerShell as a fallback. All 7 security layers are enforced
    /// regardless of the underlying shell.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `command` parameter is missing or invalid
    /// - The command contains a dangerous pattern (e.g., `rm -rf /`, `mkfs`, `dd if=`)
    /// - No suitable shell is found on Windows
    /// - The command fails to execute (command not found, permission denied, etc.)
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let command = input["command"]
            .as_str()
            .context("Missing required 'command' parameter")?;
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        // ── Determine shell ─────────────────────────────────────────────
        let shell = get_shell();

        tracing::info!(
            command = %crate::sanitize::redact_secrets(command),
            working_dir = %ctx.working_dir.display(),
            shell = ?shell,
            "Executing bash command"
        );

        if is_windows() && matches!(shell, ShellType::Bash) {
            // ShellType::Bash on Windows means no shell was found.
            bail!(
                "No suitable shell found on Windows. \
                 Please install Git for Windows or PowerShell 7+."
            );
        }

        // ── Security checks (all 7 layers, shell-agnostic) ───────────────
        if is_safe_command(command) {
            tracing::info!("Safe bash command auto-approved");
        }

        // CC1-T4: Check for banned commands (curl, wget, nc, etc.)
        // A user-defined allowlist entry (via /bash add allow <cmd>) exempts the command.
        if contains_banned_command(command) {
            if ragent_config::yolo::is_enabled() {
                tracing::warn!("YOLO mode: allowing banned command tool");
            } else if ragent_config::bash_lists::is_allowlisted(command) {
                tracing::info!("Banned command allowed by user allowlist");
            } else {
                bail!(
                    "Command rejected: uses banned external tool (curl, wget, nc, telnet, axel, aria2c, lynx, w3m). \
                     These tools could exfiltrate data or connect to external systems."
                );
            }
        }

        // CC1-T5: Check for directory escape attempts (cd to parent or absolute paths)
        if is_directory_escape_attempt(command, &ctx.working_dir) {
            bail!(
                "Command rejected: attempts to escape working directory {}. \
                 Use only relative paths (cd ./subdir, cd subdir).",
                ctx.working_dir.display()
            );
        }

        // CC1-T6: Pre-check bash syntax (skipped for PowerShell)
        validate_bash_syntax(command).await?;

        // Check for denied command names (word-boundary matched, e.g. mkfs, insmod, useradd)
        if contains_denied_command(command) {
            if ragent_config::yolo::is_enabled() {
                tracing::warn!("YOLO mode: allowing denied command name");
            } else {
                bail!(
                    "Command rejected: uses dangerous command (mkfs, insmod, useradd, etc.). \
                     These commands could cause irreversible damage to the system."
                );
            }
        }

        // Check for denied patterns (substring-matched, e.g. "rm -rf /", "sudo ", "/dev/tcp/")
        for pattern in DENIED_PATTERNS {
            if command.contains(pattern) {
                if ragent_config::yolo::is_enabled() {
                    tracing::warn!(pattern, "YOLO mode: allowing denied pattern");
                } else {
                    bail!(
                        "Command rejected: contains dangerous pattern '{pattern}'. This pattern could cause irreversible damage to the system."
                    );
                }
            }
        }

        // Check user-defined denylist (from ragent.json `bash.denylist`)
        if !ragent_config::yolo::is_enabled()
            && let Some(pattern) = ragent_config::bash_lists::matches_denylist(command)
        {
            bail!(
                "Command rejected: matches user-defined deny pattern '{pattern}'. \
                    Use `/bash remove deny \"{pattern}\"` to remove this restriction."
            );
        }

        // Reject commands that use encoding/eval tricks to bypass the denylist.
        if !ragent_config::yolo::is_enabled() {
            validate_no_obfuscation(command)?;
        }

        // Acquire a process-spawn permit to bound concurrency.
        let _permit = crate::resource::acquire_process_permit().await?;

        // ── Persistent shell state ────────────────────��───────────────────
        let state_file = state_file_path(&ctx.session_id);
        let script_file = script_file_path(&ctx.session_id, shell)?;

        // Write the user command to the temporary script file.
        std::fs::write(&script_file, command)
            .context("Failed to write command to temporary script file")?;

        // Build the appropriate wrapper script for the detected shell.
        let wrapper = match shell {
            ShellType::Bash | ShellType::GitBash(_) => {
                build_posix_wrapper(&state_file, &script_file)
            }
            ShellType::PowerShell(_) => build_powershell_wrapper(&state_file, &script_file),
        };

        // ── sudo askpass broker ──────────────────────────────────────────
        // On POSIX systems, install an askpass helper so that any `sudo`
        // invocation inside the command (including in child scripts) routes
        // its password prompt through ragent's question dialog instead of
        // hanging on the controlling tty. See `askpass` module docs.
        let mut broker = crate::askpass::AskPassBroker::start(&ctx.session_id);
        let start = Instant::now();

        let result = match shell {
            ShellType::Bash | ShellType::GitBash(_) | ShellType::PowerShell(_) => {
                // Build the wrapper invocation. For the foreground tool the
                // wrapper script carries the user command + persistent-shell
                // bookkeeping; askpass env vars are added below.
                let mut cmd = build_shell_command(shell, &wrapper, &ctx.working_dir);
                // `build_shell_command` detaches stdin from the tty so nothing
                // can block on it; sudo is handled via SUDO_ASKPASS instead.
                // Kill the child process when the future is dropped (e.g.
                // when `tokio::time::timeout` elapses).  Without this, a
                // timed-out command becomes an orphan that keeps consuming
                // CPU — the root cause of the "100% CPU stuck mid-run"
                // bug.
                if let Some(ref mut b) = broker {
                    for (k, v) in b.env_vars() {
                        cmd.env(k, v);
                    }
                    b.spawn_watcher(ctx.session_id.clone(), Arc::clone(&ctx.event_bus));
                }
                tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output())
                    .await
            }
        };

        // Tear down the askpass broker (cancels the watcher, removes temp
        // files) once the command has finished.
        if let Some(b) = broker {
            b.stop();
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // After execution, read the saved cwd and publish ShellCwdChanged.
        if let Ok(state_content) = std::fs::read_to_string(&state_file)
            && let Some(cwd) = parse_cwd_from_state(&state_content)
        {
            ctx.event_bus.publish(Event::ShellCwdChanged {
                session_id: ctx.session_id.clone(),
                cwd,
            });
        }

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut content = String::new();
                if !stdout.is_empty() {
                    content.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str("[stderr]\n");
                    content.push_str(&stderr);
                }
                if content.is_empty() {
                    content = "(no output)".to_string();
                }

                let content = truncate_output(content);
                let line_count = content.lines().count();
                Ok(ToolOutput {
                    content: format!(
                        "Exit code: {exit_code}\nDuration: {elapsed_ms}ms\n\n{content}"
                    ),
                    metadata: Some(json!({
                        "exit_code": exit_code,
                        "duration_ms": elapsed_ms,
                        "line_count": line_count,
                    })),
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to execute command: {e}. Check that the command exists and is accessible."
            )),
            Err(_) => Ok(ToolOutput {
                content: format!("Command timed out after {timeout_secs} seconds"),
                metadata: Some(json!({
                    "timed_out": true,
                    "duration_ms": timeout_secs * 1000,
                })),
            }),
        }
    }
}

/// Rejects commands that attempt to bypass the denylist via encoding,
/// eval, or dynamic variable expansion tricks.
fn validate_no_obfuscation(command: &str) -> Result<()> {
    // base64 decode piped into shell
    if command.contains("base64") && (command.contains("| bash") || command.contains("| sh")) {
        bail!("Command rejected: base64-decode-to-shell pattern detected.");
    }

    // Python/perl one-liners executing encoded payloads
    if (command.contains("python") || command.contains("perl"))
        && (command.contains("exec(") || command.contains("eval("))
    {
        bail!("Command rejected: dynamic eval/exec in scripting language.");
    }

    // $'\xNN' hex escape sequences used to build commands
    if command.contains("$'\\x") {
        bail!("Command rejected: hex escape sequence obfuscation detected.");
    }

    // Prevent `eval` with variable expansion that could hide intent
    if command.contains("eval ") && command.contains("$(") {
        bail!("Command rejected: eval with command substitution detected.");
    }

    Ok(())
}

// ── Tests ─────────────────────────���────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/inline/bash.rs"]
mod bash_tests;
