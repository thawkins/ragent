# Windows Shell Support

ragent's `BashTool` supports running shell commands on Windows, in addition to
the existing Unix (Linux/macOS) support. On Windows the tool automatically
discovers and uses the best available shell.

## Shell Discovery Order

When running on Windows, `BashTool` searches for a shell in the following order:

1. **`GIT_BASH` environment variable** — If set, the path it points to is used
   directly (must be a valid `bash.exe`).
2. **Well-known Git for Windows paths:**
   - `C:\Program Files\Git\bin\bash.exe`
   - `C:\Program Files\Git\usr\bin\bash.exe`
   - `C:\Program Files (x86)\Git\bin\bash.exe`
   - `C:\Program Files (x86)\Git\usr\bin\bash.exe`
3. **`bash.exe` on PATH** — Any `bash.exe` found via the system `PATH`.
4. **`pwsh` (PowerShell 7+)** — Modern cross-platform PowerShell.
5. **`powershell` (Windows PowerShell 5.1)** — Built-in Windows PowerShell.

If no shell is found, the tool returns an error:
```
No suitable shell found on Windows. Please install Git for Windows or PowerShell 7+.
```

## Preferred Shell: Git Bash

Git Bash is preferred because it provides POSIX compatibility, which matches
ragent's existing Unix behavior. The safe-command whitelist, banned-command
checks, denied patterns, syntax validation (`sh -n -c`), and all other
security layers work identically to Unix.

## Fallback: PowerShell

When Git Bash is unavailable, PowerShell is used as a fallback. Key
differences:

- **Syntax validation is skipped.** The `sh -n -c` pre-check is a POSIX
  concept and does not apply to PowerShell. PowerShell reports parse errors
  at execution time instead.
- **Wrapper script format.** PowerShell uses `Invoke-Expression` with
  `-NoLogo -NoProfile -NonInteractive -Command` arguments. State is
  persisted via `Set-Content` / dot-source instead of `export -p` / `source`.
- **Exit codes.** `$LASTEXITCODE` is captured and propagated to the Rust
  caller.

## PowerShell Execution Policy

PowerShell's execution policy may block script execution. If you encounter
errors, set the policy for the current user:

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## File Paths and State

On Windows, temporary script files and persistent state files are stored in
`%LOCALAPPDATA%\ragent\shell\` instead of `/tmp/`. The directory is created
automatically on first use.

| Platform | State file path | Script file path |
|----------|---------------|-----------------|
| Unix     | `/tmp/ragent_shell_<id>.state` | `/tmp/ragent_cmd_<id>_<ts>.sh` |
| Windows (Git Bash) | `%LOCALAPPDATA%\ragent\shell\ragent_shell_<id>.state` | `%LOCALAPPDATA%\ragent\shell\ragent_cmd_<id>_<ts>.sh` |
| Windows (PowerShell) | `%LOCALAPPDATA%\ragent\shell\ragent_shell_<id>.state` | `%LOCALAPPDATA%\ragent\shell\ragent_cmd_<id>_<ts>.ps1` |

## Security Layers on Windows

All 7 security layers are active on Windows regardless of the shell:

| Layer | Description | Windows Behavior |
|-------|-------------|-----------------|
| 1. Safe-command whitelist | Auto-approve known-safe commands | Unchanged — same 51 commands |
| 2. Banned commands | Block dangerous tools (curl, wget, nmap, etc.) | Unchanged |
| 3. Denied commands & patterns | Block destructive commands (mkfs, sudo, rm -rf /) | Unchanged |
| 4. Directory-escape prevention | Block `cd ..`, `cd /`, `cd ~` | Extended — also blocks `cd C:\`, `cd \`, `cd D:\` |
| 5. Syntax validation | Pre-check command syntax | **Git Bash**: unchanged · **PowerShell**: skipped |
| 6. Obfuscation detection | Block base64-pipe, eval, hex escapes | Unchanged |
| 7. User allow/deny lists | Custom per-project allow/deny rules | Unchanged |

### Windows Directory-Escape Detection

In addition to the standard Unix checks, `cd` and `pushd` on Windows also
reject:

- **Drive-letter paths**: `cd C:\Users`, `cd D:\project`
- **Bare backslash**: `cd \` (root of current drive)

## Custom Shell Override

Set the `GIT_BASH` environment variable to force a specific Git Bash location:

```bash
# In .bashrc or ragent.json environment
export GIT_BASH="C:/Users/me/Git/bin/bash.exe"
```

## Limitations

- `cmd.exe` is **not** supported as a shell fallback. Only Git Bash and
  PowerShell are supported.
- WSL/WSL2 integration is not yet supported (separate spec).
- PowerShell aliases for POSIX commands (`dir` for `ls`, `type` for `cat`)
  are **not** added to the safe-command whitelist. Use the POSIX names.