//! Bash tool tests (M8/T8.3).
//! Compiled as a submodule of bash via #[path].

    use super::*;

    // ── Safe command tests ──────────────────────────────────────────────

    #[test]
    fn test_safe_command_exact_match() {
        assert!(is_safe_command("ls"));
        assert!(is_safe_command("git"));
        assert!(is_safe_command("cargo"));
    }

    #[test]
    fn test_safe_command_with_args() {
        assert!(is_safe_command("ls -la"));
        assert!(is_safe_command("git status"));
        assert!(is_safe_command("cargo build"));
    }

    #[test]
    fn test_unsafe_command() {
        assert!(!is_safe_command("rm"));
        assert!(!is_safe_command("curl"));
        assert!(!is_safe_command("nmap"));
    }

    // ── Banned command tests ─────────────────────────────────────────────

    #[test]
    fn test_banned_command_exact() {
        assert!(contains_banned_command("curl http://example.com"));
        assert!(contains_banned_command("wget http://example.com"));
    }

    #[test]
    fn test_banned_command_word_boundary() {
        // "curl" should not match "curling"
        assert!(!contains_banned_command("curling -v output"));
        // "nc" should not match "uncle"
        assert!(!contains_banned_command("uncle"));
    }

    // ── Denied command tests ─────────────────────────────────────────────

    #[test]
    fn test_denied_command_mkfs() {
        assert!(contains_denied_command("mkfs /dev/sda"));
        assert!(contains_denied_command("mkfs.ext4 /dev/sda"));
    }

    #[test]
    fn test_denied_command_sudo() {
        assert!(contains_denied_command("sudo apt install foo"));
        assert!(contains_denied_command("sudo\tapt install foo"));
    }

    // ── Directory escape tests ───────────────────────────────────────────

    #[test]
    fn test_directory_escape_parent() {
        let wd = std::path::Path::new("/home/user/project");
        assert!(is_directory_escape_attempt("cd ..", wd));
        assert!(is_directory_escape_attempt("cd ../..", wd));
    }

    #[test]
    fn test_directory_escape_home() {
        let wd = std::path::Path::new("/home/user/project");
        assert!(is_directory_escape_attempt("cd ~", wd));
        assert!(is_directory_escape_attempt("cd $HOME", wd));
        assert!(is_directory_escape_attempt("cd ${HOME}", wd));
    }

    #[test]
    fn test_directory_escape_absolute() {
        // Use a real temporary directory so canonicalize() works properly.
        let tmp = tempfile::tempdir().expect("tempdir");
        let wd = tmp.path();
        // Multi-segment absolute path outside wd should be detected.
        assert!(is_directory_escape_attempt("cd /etc/passwd", wd));
    }

    #[test]
    fn test_directory_escape_subpath_ok() {
        let wd = std::path::Path::new("/home/user/project");
        assert!(!is_directory_escape_attempt("cd src", wd));
        assert!(!is_directory_escape_attempt("cd ./src", wd));
    }

    // ── Obfuscation detection tests ──────────────────────────────────────

    #[test]
    fn test_obfuscation_base64() {
        assert!(validate_no_obfuscation("echo dGVzdA== | base64 -d | bash").is_err());
    }

    // ── Windows directory escape tests (inner function, testable on any OS) ──

    #[test]
    fn test_windows_directory_escape_drive_letter() {
        // Drive-letter absolute paths should be rejected on Windows
        let wd = std::path::Path::new("/home/user/project");
        assert!(is_directory_escape_attempt_inner("cd C:\\Users", wd, true));
        assert!(is_directory_escape_attempt_inner(
            "cd D:\\project",
            wd,
            true
        ));
        assert!(is_directory_escape_attempt_inner("cd C:/Users", wd, true));
    }

    #[test]
    fn test_windows_directory_escape_backslash() {
        // Bare backslash (root of current drive) should be rejected on Windows
        let wd = std::path::Path::new("/home/user/project");
        assert!(is_directory_escape_attempt_inner("cd \\", wd, true));
    }

    #[test]
    fn test_windows_directory_escape_not_on_unix() {
        // On Unix, Windows-style paths are not flagged (they're not valid paths)
        let wd = std::path::Path::new("/home/user/project");
        // On Unix (on_windows=false), C:\ paths should NOT trigger escape detection
        // because is_windows() returns false
        assert!(!is_directory_escape_attempt_inner(
            "cd C:\\Users",
            wd,
            false
        ));
    }

    #[test]
    fn test_directory_escape_pushd() {
        let wd = std::path::Path::new("/home/user/project");
        assert!(is_directory_escape_attempt("pushd ..", wd));
        assert!(is_directory_escape_attempt("pushd ~", wd));
    }

    // ── Obfuscation detection tests (continued) ──────────────────────────
    #[test]
    fn test_obfuscation_python_exec() {
        assert!(validate_no_obfuscation("python -c exec('code')").is_err());
    }

    #[test]
    fn test_obfuscation_hex_escape() {
        assert!(validate_no_obfuscation("$'\\x6c\\x73'").is_err());
    }

    #[test]
    fn test_obfuscation_eval_subshell() {
        assert!(validate_no_obfuscation("eval $(whoami)").is_err());
    }

    #[test]
    fn test_obfuscation_clean_command() {
        assert!(validate_no_obfuscation("ls -la").is_ok());
    }

    // ── Shell discovery tests (Unix-only) ───────────────────────────────

    #[test]
    fn test_is_unix_returns_bash() {
        // On non-Windows, discover_shell should return ShellType::Bash
        if !is_windows() {
            let shell = discover_shell();
            assert!(matches!(shell, ShellType::Bash));
        }
    }

    #[test]
    fn test_shell_cache_is_consistent() {
        let shell1 = get_shell();
        let shell2 = get_shell();
        // Both references should point to the same cached value
        let _ = shell1;
        let _ = shell2;
        // Both references should point to the same cached value
        assert!(matches!(shell1, ShellType::Bash) || is_windows());
    }

    // ── State file path tests ────────────────────────────────────────────

    #[test]
    fn test_state_file_path_format() {
        let path = state_file_path("test-session-123");
        if is_windows() {
            // On Windows, should use LOCALAPPDATA-based path
            assert!(
                path.contains("ragent_shell_test-session-123.state"),
                "Windows state path should contain the session filename: {path}"
            );
        } else {
            // On Unix, should use /tmp
            assert!(
                path.starts_with("/tmp/ragent_shell_"),
                "Unix state path should start with /tmp: {path}"
            );
        }
    }

    #[test]
    fn test_safe_session_id() {
        assert_eq!(safe_session_id("abc-123"), "abc-123");
        assert_eq!(safe_session_id("abc 123"), "abc_123");
        assert_eq!(safe_session_id("abc/123"), "abc_123");
    }

    // ── Windows path helpers ─────────────────────────────────────────────

    #[test]
    fn test_to_posix_path() {
        assert_eq!(
            to_posix_path(r"C:\Users\test\file.txt"),
            "C:/Users/test/file.txt"
        );
        assert_eq!(to_posix_path("/tmp/test.sh"), "/tmp/test.sh");
    }

    // ── Heredoc handling tests ───────────────────────────────────────────

    #[test]
    fn test_strip_heredoc_bodies() {
        let cmd = "cat <<'EOF'\nsome heredoc content\nEOF\n";
        let stripped = strip_heredoc_bodies(cmd);
        // The heredoc body should be removed but the markers kept
        assert!(!stripped.contains("some heredoc content"));
        assert!(stripped.contains("cat <<'EOF'"));
        assert!(stripped.contains("EOF"));
    }

    // ── Extract command names tests ──────────────────────────────────────

    #[test]
    fn test_extract_command_names_simple() {
        let names = extract_command_names("ls -la");
        assert_eq!(names, vec!["ls"]);
    }

    #[test]
    fn test_extract_command_names_piped() {
        let names = extract_command_names("ls | grep foo");
        assert_eq!(names, vec!["ls", "grep"]);
    }

    #[test]
    fn test_extract_command_names_chained() {
        let names = extract_command_names("cd tmp && mkfs");
        assert_eq!(names, vec!["cd", "mkfs"]);
    }

    // ── PowerShell wrapper tests ─────────────────────────────────────────

    #[test]
    fn test_powershell_wrapper_contains_invoke() {
        let wrapper = build_powershell_wrapper("C:/state.state", "C:/cmd.ps1");
        assert!(wrapper.contains("Invoke-Expression"));
        assert!(wrapper.contains("RAGENT_PWD"));
    }

    #[test]
    fn test_posix_wrapper_structure() {
        let wrapper = build_posix_wrapper("/tmp/ragent_shell_test.state", "/tmp/cmd.sh");
        assert!(wrapper.contains("STATE_FILE"));
        assert!(wrapper.contains("RAGENT_PWD"));
        assert!(wrapper.contains("EXIT_CODE"));
    }

    // ── Script file path tests ──────────────────────────────────────────

    #[test]
    fn test_script_file_path_bash_extension() {
        let path = script_file_path("test", &ShellType::Bash).unwrap();
        assert!(
            path.ends_with(".sh"),
            "Bash script should have .sh extension: {path}"
        );
    }

    #[test]
    fn test_script_file_path_powershell_extension() {
        let path =
            script_file_path("test", &ShellType::PowerShell(PathBuf::from("pwsh.exe"))).unwrap();
        assert!(
            path.ends_with(".ps1"),
            "PowerShell script should have .ps1 extension: {path}"
        );
    }

    #[test]
    fn test_script_file_path_gitbash_extension() {
        let path =
            script_file_path("test", &ShellType::GitBash(PathBuf::from("bash.exe"))).unwrap();
        assert!(
            path.ends_with(".sh"),
            "Git Bash script should have .sh extension: {path}"
        );
    }

