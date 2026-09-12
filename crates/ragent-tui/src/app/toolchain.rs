//! Toolchain presence report for the `/toolchain` slash command.
//!
//! This module holds the static data tables that map the code-index
//! supported-language list (`ragent_codeindex::scanner::SUPPORTED_LANGUAGES`)
//! onto the runtime toolchains required to build and run code in each
//! language, plus the classification of every entry as either an application
//! language (runtime probed) or a data/format entry (runtime skipped), and
//! the report engine: a PATH presence probe (`command -v` semantics, no
//! spawn), a bounded version probe (`<tool> <flag>` with a 2 s timeout), and
//! the markdown/JSON report renderers.
//!
//! Dependencies: `ragent_codeindex::scanner` (language list),
//! `serde_json` (report serialisation), `tempfile` (probe test fixtures).

use ragent_codeindex::scanner::SUPPORTED_LANGUAGES;

/// Classification of a `SUPPORTED_LANGUAGES` entry.
///
/// Application languages have a source runtime (compiler or interpreter) that
/// can be probed on `PATH`; data/format entries are pure data, markup, or
/// build-DSL formats with no source runtime and are reported as
/// `(data format — runtime n/a)` without probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageClass {
    /// Language with a probed runtime (see [`LanguageRuntime`]).
    Application,
    /// Pure data / markup / build-DSL format — no runtime to probe.
    DataFormat,
}

/// The runtime toolchain(s) an application language requires.
///
/// Each entry lists one primary runtime command plus an ordered fallback
/// chain; presence is satisfied when the primary (or a fallback, when
/// declared) resolves on `PATH` (FR-006). Version capture uses each
/// command's per-tool version flag (FR-008 notes in SPEC FR-006 table).
#[derive(Debug, Clone, Copy)]
pub struct LanguageRuntime {
    /// Ordered runtime commands to probe (primary first, then fallbacks).
    pub commands: &'static [&'static str],
}

impl LanguageRuntime {
    /// Convenience constructor for the multi-command case.
    pub const fn many(commands: &'static [&'static str]) -> Self {
        Self { commands }
    }
}

/// Per-tool version flag, replacing the `--version` default.
///
/// Most tools report a version on stdout with `--version`; the exceptions
/// below need their own flag (see SPEC FR-008 notes).
#[derive(Debug, Clone, Copy)]
pub struct VersionFlag {
    /// Tool command the flag applies to (compile-time constant).
    pub command: &'static str,
    /// The flag argument that makes the tool print its version.
    pub flag: &'static str,
}

/// Version-flag exceptions (command, flag). Everything else defaults to
/// `--version`.
///
/// Go and Zig have no `--version` flag; their versions are subcommands
/// (`go version`, `zig version`).
pub const VERSION_FLAG_OVERRIDES: &[VersionFlag] = &[
    VersionFlag {
        command: "java",
        flag: "-version",
    },
    VersionFlag {
        command: "erl",
        flag: "-version",
    },
    VersionFlag {
        command: "lua",
        flag: "-v",
    },
    VersionFlag {
        command: "luajit",
        flag: "-v",
    },
    VersionFlag {
        command: "go",
        flag: "version",
    },
    VersionFlag {
        command: "zig",
        flag: "version",
    },
];

/// Multi-argument version probes (command, argv). Everything else uses the
/// single flag from [`version_flag_for`].
///
/// `dotnet --version` prints only the active SDK version (and fails when no
/// SDK is installed), so the dotnet probe combines `--list-sdks` and
/// `--list-runtimes` into one line.
pub const VERSION_PROBE_ARGS_OVERRIDES: &[(&str, &[&str])] =
    &[("dotnet", &["--list-sdks", "--list-runtimes"])];

/// Per-language classification and runtime mapping.
///
/// The table must cover every entry of [`SUPPORTED_LANGUAGES`] exactly once
/// (guaranteed by T-002's exhaustiveness check); ids are the canonical
/// scanner ids, not user aliases.
pub const LANGUAGE_RUNTIMES: &[(&str, LanguageClass, Option<LanguageRuntime>)] = &[
    // ── Application languages ────────────────────────────────────────────
    (
        "rust",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cargo", "rustc"])),
    ),
    (
        "python",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["python3"])),
    ),
    (
        "typescript",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node", "tsc"])),
    ),
    (
        "tsx",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node", "tsc"])),
    ),
    (
        "javascript",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node"])),
    ),
    (
        "jsx",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node"])),
    ),
    (
        "go",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["go"])),
    ),
    // cc is the portable compiler-driver name; gcc/clang are fallbacks.
    (
        "c",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cc", "gcc", "clang"])),
    ),
    (
        "c_header",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cc", "gcc", "clang"])),
    ),
    (
        "cpp",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["c++", "g++", "clang++"])),
    ),
    (
        "cpp_header",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["c++", "g++", "clang++"])),
    ),
    (
        "java",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["java", "javac"])),
    ),
    (
        "kotlin",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["kotlinc"])),
    ),
    (
        "ruby",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ruby"])),
    ),
    (
        "swift",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["swift"])),
    ),
    (
        "csharp",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["dotnet"])),
    ),
    (
        "lua",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["lua", "luajit"])),
    ),
    (
        "shell",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["bash"])),
    ),
    (
        "zsh",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["zsh"])),
    ),
    (
        "fish",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["fish"])),
    ),
    (
        "zig",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["zig"])),
    ),
    (
        "nim",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["nim"])),
    ),
    (
        "elixir",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["elixir"])),
    ),
    (
        "erlang",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["erl"])),
    ),
    (
        "haskell",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ghc", "runghc"])),
    ),
    (
        "ocaml",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ocaml"])),
    ),
    (
        "r",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["R"])),
    ),
    (
        "dart",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["dart"])),
    ),
    (
        "php",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["php"])),
    ),
    (
        "perl",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["perl"])),
    ),
    // ── Data / markup / build-DSL formats (no runtime probed) ────────────
    ("toml", LanguageClass::DataFormat, None),
    ("yaml", LanguageClass::DataFormat, None),
    ("json", LanguageClass::DataFormat, None),
    ("xml", LanguageClass::DataFormat, None),
    ("html", LanguageClass::DataFormat, None),
    ("css", LanguageClass::DataFormat, None),
    ("scss", LanguageClass::DataFormat, None),
    ("sql", LanguageClass::DataFormat, None),
    ("markdown", LanguageClass::DataFormat, None),
    ("protobuf", LanguageClass::DataFormat, None),
    ("verilog", LanguageClass::DataFormat, None),
    ("vhdl", LanguageClass::DataFormat, None),
    ("terraform", LanguageClass::DataFormat, None),
    ("openscad", LanguageClass::DataFormat, None),
    ("cmake", LanguageClass::DataFormat, None),
    ("gradle", LanguageClass::DataFormat, None),
    ("gradle_kts", LanguageClass::DataFormat, None),
    ("maven", LanguageClass::DataFormat, None),
    ("nix", LanguageClass::DataFormat, None),
    ("hcl", LanguageClass::DataFormat, None),
];

/// Verify the FR-005 exhaustiveness guarantee of [`LANGUAGE_RUNTIMES`] against
/// [`SUPPORTED_LANGUAGES`], returning one human-readable error per violation.
///
/// A mapping is exhaustive when it holds all of the following:
///
/// - every `SUPPORTED_LANGUAGES` id has exactly one mapping row (no gaps, no
///   duplicates), and
/// - every mapping row references an id from `SUPPORTED_LANGUAGES` (no rows
///   outside the scanner list), and
/// - the partition matches FR-005: exactly 30 application rows and exactly 20
///   data-format rows.
///
/// Returns an empty `Vec` when the guarantee holds. Called by the tests as the
/// CI-detectable guard, and reusable at runtime by any caller that wants to
/// assert the invariant before walking the table.
pub fn exhaustiveness_errors() -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    // Every scanner id must map exactly once.
    for lang in SUPPORTED_LANGUAGES {
        let match_count = LANGUAGE_RUNTIMES
            .iter()
            .filter(|(id, _, _)| *id == *lang)
            .count();
        if match_count == 0 {
            errors.push(format!(
                "language {lang:?} in SUPPORTED_LANGUAGES has no mapping row"
            ));
        } else if match_count > 1 {
            errors.push(format!(
                "language {lang:?} has {match_count} mapping rows, expected exactly 1"
            ));
        }
    }

    // Every mapping row must reference a scanner id.
    for (id, _, _) in LANGUAGE_RUNTIMES {
        if !SUPPORTED_LANGUAGES.contains(id) {
            errors.push(format!("mapping id {id:?} is not in SUPPORTED_LANGUAGES"));
        }
    }

    // The partition must match FR-005 (30 application / 20 data-format).
    let application = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::Application)
        .count();
    let data_format = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::DataFormat)
        .count();
    if application != 30 {
        errors.push(format!(
            "application-language row count is {application}, expected 30 (FR-005)"
        ));
    }
    if data_format != 20 {
        errors.push(format!(
            "data-format row count is {data_format}, expected 20 (FR-005)"
        ));
    }

    errors
}

/// Walk [`SUPPORTED_LANGUAGES`] in list order, resolving each id through
/// [`LANGUAGE_RUNTIMES`].
///
/// This is the FR-004/FR-005 derivation API: the report iterates the scanner
/// const and classifies via the single mapping table rather than any second
/// hard-coded language list. A row resolves to `None` only when the mapping is
/// non-exhaustive (guarded by [`exhaustiveness_errors`]).
#[must_use]
pub fn supported_language_rows() -> impl Iterator<
    Item = (
        &'static str,
        Option<(LanguageClass, Option<LanguageRuntime>)>,
    ),
> {
    SUPPORTED_LANGUAGES.iter().map(|&id| (id, lookup(id)))
}

/// Look up the classification and runtime for a canonical language id.
///
/// Returns `None` when the id has no mapping row — a signal that a new
/// `SUPPORTED_LANGUAGES` entry was added without extending this table
/// (guarded by the T-002 exhaustiveness check).
#[must_use]
pub fn lookup(id: &str) -> Option<(LanguageClass, Option<LanguageRuntime>)> {
    LANGUAGE_RUNTIMES
        .iter()
        .find(|(lang_id, _, _)| *lang_id == id)
        .map(|(_, class, runtime)| (*class, runtime.as_ref().copied()))
}

/// Whether a language id is an application language (runtime probed).
#[must_use]
pub fn is_application(id: &str) -> bool {
    matches!(lookup(id), Some((LanguageClass::Application, _)))
}

/// The version flag for a runtime command (default `--version`).
#[must_use]
pub fn version_flag_for(command: &str) -> &'static str {
    VERSION_FLAG_OVERRIDES
        .iter()
        .find(|o| o.command == command)
        .map_or("--version", |o| o.flag)
}

/// Resolve a command name against a list of `PATH` directories
/// (FR-007, `command -v` semantics) without spawning any process.
///
/// Returns the full path of the first matching executable file, or `None`
/// when the command does not resolve. On Unix the file must carry an
/// execute bit; on Windows the file must exist and match a `PATHEXT`
/// extension (an explicit extension in the command is honoured as-is).
fn resolve_in_dirs(command: &str, dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    use std::path::Path;

    if command.is_empty() || command.contains('/') {
        // Absolute/relative paths are not probed; only bare command names.
        return None;
    }

    let candidate_names: Vec<String> = if cfg!(windows) {
        let ext = Path::new(command)
            .extension()
            .map_or_else(|| true, |e| e.is_empty());
        if ext {
            let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT".into());
            pathext
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|e| format!("{command}{e}"))
                .collect()
        } else {
            vec![command.to_string()]
        }
    } else {
        vec![command.to_string()]
    };

    for dir in dirs {
        for name in &candidate_names {
            let candidate = dir.join(name);
            let Ok(meta) = std::fs::metadata(&candidate) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 != 0 {
                    return Some(candidate);
                }
            }
            #[cfg(not(unix))]
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// Probe whether a runtime command is installed, by `PATH` resolution.
///
/// Presence is determined purely in-process (FR-007): the command name is
/// resolved against the `PATH` environment variable with `command -v`
/// semantics — no tool (or `which`) is spawned and no network access is
/// used. `Some(path)` means `installed`, `None` means `not installed`.
#[must_use]
pub fn probe_installed(command: &str) -> Option<std::path::PathBuf> {
    let dirs: Vec<std::path::PathBuf> = std::env::split_paths(&std::env::var_os("PATH")?).collect();
    resolve_in_dirs(command, &dirs)
}

// ---------------------------------------------------------------------------
// Version probe (T-004, FR-008 / FR-012)
// ---------------------------------------------------------------------------

/// Per-probe timeout for version capture (FR-012).
pub const VERSION_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Outcome of a version probe (FR-008 / FR-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionResult {
    /// First non-empty line captured from stdout or stderr.
    Text(String),
    /// Probe could not start, or produced no usable version text.
    Unknown,
    /// Probe exceeded [`VERSION_PROBE_TIMEOUT`] and was killed.
    Timeout,
}

impl VersionResult {
    /// Map to the version string the report carries (FR-012): captured text
    /// for [`VersionResult::Text`], the `unknown` / `timeout` placeholders
    /// otherwise, so an installed-but-unversioned runtime still reports
    /// `installed` rather than `-`.
    #[must_use]
    pub fn placeholder_or_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Unknown => Some("unknown".to_string()),
            Self::Timeout => Some("timeout".to_string()),
        }
    }
}

/// Run one bounded version probe and return the raw spawn/timeout outcome.
///
/// Shared engine behind [`probe_version_args`] and
/// [`probe_version_args_all_lines`]: spawns `program args`, polls with
/// `try_wait` on a worker thread so a hung child is killed at the deadline
/// instead of blocking the caller forever (FR-012), and hands the result to
/// the caller over a channel. The outcome is one of:
///
/// - `Ok(Some(Ok(output)))` — a completed probe carrying the pipes' content;
/// - `Ok(Some(Err(io)))` — spawn failure reported by the worker;
/// - `Ok(None)` — the worker's timed-out send (output discarded);
/// - `Err(RecvTimeoutError)` — the worker never reported within the bounded
///   receive window.
///
/// The receive is bounded by [`VERSION_PROBE_TIMEOUT`] plus a slack margin;
/// on a receive timeout the worker thread is **detached, not joined** — the
/// non-timeout path reads the pipes to EOF via `wait_with_output()`, and a
/// grandchild inheriting them (shell wrappers fork) can block that read
/// indefinitely. Joining would hang `/toolchain list` past its own bound;
/// a detached leak of one short-lived thread per timed-out receive is the
/// safe containment.
#[must_use]
fn run_probe_raw(
    program: &str,
    args: &[&str],
) -> Result<Option<Result<std::process::Output, std::io::Error>>, std::sync::mpsc::RecvTimeoutError>
{
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // A spawn failure never reaches the worker: report it via the same
    // channel shape so callers keep distinguishing Unknown from Timeout.
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => return Ok(Some(Err(err))),
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + VERSION_PROBE_TIMEOUT;
        let mut timed_out = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        timed_out = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        if timed_out {
            // A killed direct child may leave grandchildren holding the
            // stdout/stderr pipes (shell wrappers fork); reading to EOF could
            // block until those exit. Timeout discards the output, so send
            // without draining the pipes (FR-012 containment).
            let _ = tx.send(None);
            return;
        }
        let output = child.wait_with_output();
        let _ = tx.send(Some(output));
    });

    rx.recv_timeout(VERSION_PROBE_TIMEOUT + std::time::Duration::from_secs(2))
}

/// Capture the version of an installed runtime command with explicit
/// arguments.
///
/// `program` may be a bare command name or the path returned by
/// [`probe_installed`]; the walk layer (T-011) passes the resolved path so no
/// second `PATH` lookup is needed. The version flag must be supplied
/// explicitly here (the name-keyed per-tool overrides of
/// [`version_flag_for`] do not match full paths, so callers apply
/// `version_flag_for(command_name)` themselves).
#[must_use]
pub fn probe_version_args(program: &str, args: &[&str]) -> VersionResult {
    match run_probe_raw(program, args) {
        // `None` is the worker's explicit "timed out, output discarded" send.
        Ok(None) => VersionResult::Timeout,
        Ok(Some(Ok(output))) => {
            let text = all_non_empty_lines(&output.stdout)
                .into_iter()
                .next()
                .or_else(|| all_non_empty_lines(&output.stderr).into_iter().next());
            match text {
                Some(line) => VersionResult::Text(line),
                None => VersionResult::Unknown,
            }
        }
        Ok(Some(Err(_))) => VersionResult::Unknown,
        // Receive timeout: the worker never reported; scheduler pathology.
        Err(_) => VersionResult::Timeout,
    }
}

/// Capture the version of an installed runtime command (FR-008).
///
/// Executes `<command> <version-flag>` — the per-tool flag from
/// [`version_flag_for`], default `--version` — and returns the first
/// non-empty line from stdout or stderr, trimmed. The probe is bounded by
/// [`VERSION_PROBE_TIMEOUT`]; an overrunning child is killed and reported as
/// [`VersionResult::Timeout`] (FR-012). Spawn failures and textless exits
/// yield [`VersionResult::Unknown`]; no failure propagates a panic or blocks
/// the caller.
#[must_use]
pub fn probe_version(command: &str) -> VersionResult {
    probe_version_args(command, &[version_flag_for(command)])
}

// ---------------------------------------------------------------------------
// Full walk (T-011, FR-004 / FR-006 / FR-009 / FR-010)
// ---------------------------------------------------------------------------

/// Probe every supported language and assemble the full report rows.
///
/// The walk iterates [`SUPPORTED_LANGUAGES`] in list order through
/// [`supported_language_rows`] (FR-004 — no second hard-coded language
/// list). Application languages probe each runtime command in FR-006 order
/// via [`probe_installed`] (FR-007 PATH resolution, no spawn) and — for
/// commands found — [`probe_command_version`] with the resolved binary path
/// and the per-tool flag from [`version_flag_for`] (FR-008, bounded by
/// [`VERSION_PROBE_TIMEOUT`] FR-012). Data-format rows carry empty probes
/// and render the FR-005 marker. A missing runtime reports `not installed`
/// with a `-` version and never aborts or truncates the walk (FR-010).
///
/// Runs entirely in-process; the caller executes it on a blocking thread
/// (FR-016). Read-only: PATH resolution and version probes modify no state
/// (FR-013).
#[must_use]
pub fn build_report_rows() -> Vec<ReportRow> {
    supported_language_rows()
        .map(|(id, resolved)| {
            let Some((class, runtime)) = resolved else {
                // Non-exhaustive mapping (guarded by `exhaustiveness_errors`
                // in CI): emit a data-format-shaped row so the walk still
                // produces one row per scanner entry.
                return ReportRow {
                    id,
                    class: LanguageClass::DataFormat,
                    probes: Vec::new(),
                };
            };
            let probes = runtime
                .map(|rt| {
                    rt.commands
                        .iter()
                        .copied()
                        .map(probe_command)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            ReportRow { id, class, probes }
        })
        .collect()
}

/// Probe one runtime command: PATH presence first, version only when found.
fn probe_command(command: &'static str) -> CommandProbe {
    let Some(path) = probe_installed(command) else {
        // FR-010: absent runtimes cost one PATH walk, no spawn, and never
        // abort the walk.
        return CommandProbe {
            command,
            installed: false,
            version: None,
        };
    };
    let version =
        probe_command_version(command, path.to_string_lossy().as_ref()).placeholder_or_text();
    CommandProbe {
        command,
        installed: true,
        version,
    }
}

/// All non-empty, trimmed lines of a byte stream, if any.
///
/// Used by the multi-argument probes ([`VERSION_PROBE_ARGS_OVERRIDES`]) whose
/// commands print one version per line (e.g. `dotnet --list-sdks` reports
/// every installed SDK, not just the active one).
fn all_non_empty_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Capture the version of an installed runtime command with explicit
/// arguments, keeping every non-empty output line.
///
/// Behaves like [`probe_version_args`] except that multi-line output is
/// joined into one `, ` separated string instead of keeping only the first
/// line. Needed for probes such as `dotnet --list-sdks` where every line is
/// a distinct installed version.
fn probe_version_args_all_lines(program: &str, args: &[&str]) -> VersionResult {
    match run_probe_raw(program, args) {
        // `None` is the worker's explicit "timed out, output discarded" send.
        Ok(None) => VersionResult::Timeout,
        Ok(Some(Ok(output))) => {
            let mut lines = all_non_empty_lines(&output.stdout);
            if lines.is_empty() {
                lines = all_non_empty_lines(&output.stderr);
            }
            if lines.is_empty() {
                VersionResult::Unknown
            } else {
                VersionResult::Text(lines.join(", "))
            }
        }
        Ok(Some(Err(_))) => VersionResult::Unknown,
        // Receive timeout: the worker never reported; scheduler pathology.
        Err(_) => VersionResult::Timeout,
    }
}

/// Version probe for one resolved runtime command.
///
/// Most commands run `<path> <flag>` with the per-tool flag from
/// [`version_flag_for`]. Commands listed in [`VERSION_PROBE_ARGS_OVERRIDES`]
/// need multiple probes (e.g. `dotnet --list-sdks` + `--list-runtimes`);
/// every non-empty line of each probe's output is kept and the combined
/// output is joined into one `, ` separated version string, keeping the
/// first [`VersionResult::Timeout`] when any probe overruns. Keeping every
/// line matters for `dotnet`: `--list-sdks`/`--list-runtimes` print one
/// installed version per line and only the first line would drop the newer
/// SDKs.
fn probe_command_version(command: &'static str, program: &str) -> VersionResult {
    let Some((_, arg_sets)) = VERSION_PROBE_ARGS_OVERRIDES
        .iter()
        .find(|(probe_command, _)| *probe_command == command)
    else {
        return probe_version_args(program, &[version_flag_for(command)]);
    };
    let mut parts: Vec<String> = Vec::new();
    let mut timed_out = false;
    for args in arg_sets.iter() {
        match probe_version_args_all_lines(program, &[args]) {
            VersionResult::Text(text) => parts.push(text),
            VersionResult::Timeout => timed_out = true,
            VersionResult::Unknown => {}
        }
    }
    if !parts.is_empty() {
        VersionResult::Text(parts.join(", "))
    } else if timed_out {
        VersionResult::Timeout
    } else {
        VersionResult::Unknown
    }
}

// ---------------------------------------------------------------------------
// Report model + markdown renderer (T-005, FR-009 / FR-005)
// ---------------------------------------------------------------------------

/// Probe outcome for a single runtime command.
///
/// Produced by the T-011 walk (presence via [`probe_installed`], version via
/// the T-004 probe helper); the renderer consumes these as-is.
#[derive(Debug, Clone)]
pub struct CommandProbe {
    /// Runtime command that was probed (FR-006 order, primary first).
    pub command: &'static str,
    /// Whether the command resolved on `PATH` (FR-007).
    pub installed: bool,
    /// Captured version text (FR-008); `None` renders the `-` placeholder.
    pub version: Option<String>,
}

/// One row of the `/toolchain list` report: a language id plus its probe
/// outcomes.
#[derive(Debug, Clone)]
pub struct ReportRow {
    /// Canonical language id (a `SUPPORTED_LANGUAGES` entry).
    pub id: &'static str,
    /// Classification from the mapping table (FR-005).
    pub class: LanguageClass,
    /// Per-command probes in FR-006 order; empty for data-format rows.
    pub probes: Vec<CommandProbe>,
}

/// Marker rendered in the Status column of data-format rows (FR-005).
pub const DATA_FORMAT_MARKER: &str = "(data format — runtime n/a)";

/// Fixed column widths (characters) of the `/toolchain list` ASCII table
/// (FR-017): Language, Runtime, Status, and Version. Language and Runtime
/// cells wider than their column are clipped; the Status and Version columns
/// word-wrap onto continuation grid lines instead.
pub const TABLE_COLUMN_WIDTHS: [usize; 4] = [10, 10, 10, 50];

/// Word-wrap one cell to `width` characters, returning at least one line.
///
/// Greedy wrap on word boundaries; a single word longer than the column is
/// hard-split across lines so every returned line fits the column width.
fn wrap_cell(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    // Hard-split over-long words first, then greedily pack whole words.
    let mut words: Vec<String> = Vec::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        if word.chars().count() <= width {
            words.push(word.to_string());
        } else {
            for chunk in word.chars().collect::<Vec<_>>().chunks(width) {
                words.push(chunk.iter().collect());
            }
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in words {
        if current.is_empty() {
            current = word;
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(&word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word;
        }
    }
    lines.push(current);
    lines
}

/// Sanitize a table cell: collapse line breaks to spaces and escape pipes so
/// captured version text (FR-008) cannot break the markdown table (FR-009).
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// Compute the (runtime, status, version) cell triple for one report row.
///
/// Shared by the markdown and JSON renderers so both surfaces render probe
/// outcomes identically (FR-005 data-format marker, FR-006 multi-runtime
/// per-command status, FR-010 not-installed placeholders).
fn report_fields(row: &ReportRow) -> (String, String, String) {
    match row.class {
        LanguageClass::DataFormat => (
            "-".to_string(),
            DATA_FORMAT_MARKER.to_string(),
            "-".to_string(),
        ),
        LanguageClass::Application => {
            if row.probes.len() <= 1 {
                let probe = row.probes.first();
                let runtime = probe.map_or("-", |p| p.command).to_string();
                let status = match probe {
                    Some(p) if p.installed => "installed",
                    _ => "not installed",
                }
                .to_string();
                let version = probe
                    .and_then(|p| p.version.clone())
                    .unwrap_or_else(|| "-".to_string());
                (runtime, status, version)
            } else {
                let runtime = row
                    .probes
                    .iter()
                    .map(|p| p.command)
                    .collect::<Vec<_>>()
                    .join(", ");
                let status = row
                    .probes
                    .iter()
                    .map(|p| {
                        format!(
                            "{}: {}",
                            p.command,
                            if p.installed {
                                "installed"
                            } else {
                                "not installed"
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let version = row
                    .probes
                    .iter()
                    .map(|p| format!("{}: {}", p.command, p.version.as_deref().unwrap_or("-")))
                    .collect::<Vec<_>>()
                    .join("; ");
                (runtime, status, version)
            }
        }
    }
}

/// Count application rows and those with at least one installed runtime.
///
/// Returns `(installed, application)` — the markdown summary line and the
/// JSON `installed`/`total` fields share this counter (FR-009 / FR-015).
fn installed_summary(rows: &[ReportRow]) -> (usize, usize) {
    let mut installed = 0usize;
    let mut application = 0usize;
    for row in rows {
        if matches!(row.class, LanguageClass::Application) {
            application += 1;
            if row.probes.iter().any(|p| p.installed) {
                installed += 1;
            }
        }
    }
    (installed, application)
}

/// Render the `/toolchain list` report as a pre-formatted ASCII table (FR-009).
///
/// The message is prefixed `From: /toolchain list`, followed by the
/// Language / Runtime / Status / Version table (rows in the given order) and
/// the `<n>/<m> application runtimes installed.` summary line, where `m` is
/// the number of application rows and `n` the number of those with at least
/// one installed runtime. Data-format rows show [`DATA_FORMAT_MARKER`] in
/// place of presence/version (FR-005); absent runtimes render `not installed`
/// with a `-` version (FR-010); rows with multiple runtime commands list the
/// per-command status (FR-006).
///
/// The grid is emitted directly (markdown-table-free) inside a fenced block so
/// the TUI markdown pipeline's `From: /` code-block bypass deposits it
/// verbatim; column widths are then exact: the Language, Runtime, Status, and
/// Version columns are fixed at the [`TABLE_COLUMN_WIDTHS`] widths (FR-017).
/// Language and Runtime cells wider than their column are clipped; the Status
/// and Version columns word-wrap onto continuation grid lines (Language and
/// Runtime cells render blank on every continuation line, and the Status cell
/// is blank on Version-only continuation lines), so no cell text is lost.
#[must_use]
pub fn render_markdown_report(rows: &[ReportRow]) -> String {
    const HEADERS: [&str; 4] = ["Language", "Runtime", "Status", "Version"];
    let fields: Vec<[String; 4]> = rows
        .iter()
        .map(|row| {
            let (runtime, status, version) = report_fields(row);
            [cell(row.id), cell(&runtime), cell(&status), cell(&version)]
        })
        .collect();
    // Fixed columns (FR-017): 10/10/10/50; Language/Runtime cells are
    // clipped, the Status and Version columns word-wrap onto continuation
    // lines.
    let widths = TABLE_COLUMN_WIDTHS;
    let border = {
        let mut out = String::from("+");
        for width in &widths {
            out.push_str(&"-".repeat(*width + 2));
            out.push('+');
        }
        out
    };
    // Renders one grid row as 1..n grid lines: the Status and Version cells
    // word-wrap at word boundaries and the taller of the two sets the row
    // height; Language/Runtime appear only on the first line, and every line
    // is padded to the exact fixed column widths.
    let render_row = |cells: &[String; 4]| -> Vec<String> {
        let status_lines = wrap_cell(&cells[2], widths[2]);
        let version_lines = wrap_cell(&cells[3], widths[3]);
        (0..status_lines.len().max(version_lines.len()))
            .map(|line_idx| {
                let mut line = String::from("|");
                for (idx, width) in widths.iter().enumerate() {
                    let segment: String = match idx {
                        0 | 1 if line_idx == 0 => cells[idx].chars().take(*width).collect(),
                        2 => status_lines.get(line_idx).cloned().unwrap_or_default(),
                        3 => version_lines.get(line_idx).cloned().unwrap_or_default(),
                        _ => String::new(),
                    };
                    let pad = width.saturating_sub(segment.chars().count());
                    line.push(' ');
                    line.push_str(&segment);
                    line.push_str(&" ".repeat(pad));
                    line.push(' ');
                    line.push('|');
                }
                line
            })
            .collect()
    };
    let mut grid = String::new();
    grid.push_str(&border);
    grid.push('\n');
    for line in render_row(&HEADERS.map(str::to_string)) {
        grid.push_str(&line);
        grid.push('\n');
    }
    grid.push_str(&border);
    grid.push('\n');
    for row_fields in &fields {
        // One border after the LAST line of each (possibly multi-line) row
        // keeps the grid well-formed and borders = data rows + 2.
        for line in render_row(row_fields) {
            grid.push_str(&line);
            grid.push('\n');
        }
        grid.push_str(&border);
        grid.push('\n');
    }
    let (installed, application) = installed_summary(rows);
    // Fenced-block form: the `From: /` + bare-fence shape triggers the TUI
    // markdown pipeline's code-block bypass, which deposits the block verbatim
    // instead of re-flowing it through `html2text` (which would squeeze the
    // columns back to content-proportional widths). The summary line lives
    // inside the block because the bypass drops post-fence text.
    format!(
        "From: /toolchain list\n\n```\n{grid}{installed}/{application} application runtimes installed.\n```\n"
    )
}

/// Render the `/toolchain list --json` report as a JSON document (FR-015).
///
/// Emits the bare JSON document with no `From:` prefix so the output can be
/// piped or parsed directly (TC-009). The `languages` array carries one
/// object per report row with `id`, `runtime`, `status`, and `version`
/// fields rendered identically to the markdown table cells (data-format rows
/// included, FR-005); `installed` and `total` summarise the application-class
/// rows exactly like the markdown summary line.
#[must_use]
pub fn render_json_report(rows: &[ReportRow]) -> String {
    let (installed, total) = installed_summary(rows);
    let languages = rows
        .iter()
        .map(|row| {
            let (runtime, status, version) = report_fields(row);
            serde_json::json!({
                "id": row.id,
                "runtime": runtime,
                "status": status,
                "version": version,
            })
        })
        .collect::<Vec<_>>();
    let doc = serde_json::json!({
        "languages": languages,
        "installed": installed,
        "total": total,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one expected grid line from `(width, text)` column pairs: each
    /// segment is clipped to its column width and right-padded with spaces,
    /// mirroring [`TABLE_COLUMN_WIDTHS`] rendering (`| seg | seg | ... |`).
    fn grid_line(columns: [(usize, &str); 4]) -> String {
        let mut line = String::from("|");
        for (width, text) in columns {
            let text: String = text.chars().take(width).collect();
            let pad = width.saturating_sub(text.chars().count());
            line.push(' ');
            line.push_str(&text);
            line.push_str(&" ".repeat(pad));
            line.push(' ');
            line.push('|');
        }
        line
    }

    /// T-002 primary guarantee: the exported exhaustiveness check reports no
    /// violations for the current table.
    #[test]
    fn exhaustiveness_check_reports_no_violations() {
        assert_eq!(exhaustiveness_errors(), Vec::<String>::new());
    }

    /// Every scanner language id must have exactly one mapping row.
    ///
    /// This is the T-002 exhaustiveness guarantee expressed as a test; it
    /// lives here so the const table cannot drift from the scanner list
    /// without a CI-visible failure.
    #[test]
    fn every_supported_language_has_a_mapping() {
        for lang in SUPPORTED_LANGUAGES {
            let match_count = LANGUAGE_RUNTIMES
                .iter()
                .filter(|(id, _, _)| *id == *lang)
                .count();
            assert_eq!(match_count, 1, "language {lang:?} must map exactly once");
        }
    }

    /// No mapping row may reference a language id outside the scanner list.
    #[test]
    fn no_mapping_rows_outside_scanner_list() {
        for (id, _, _) in LANGUAGE_RUNTIMES {
            assert!(
                SUPPORTED_LANGUAGES.contains(id),
                "mapping id {id:?} is not in SUPPORTED_LANGUAGES"
            );
        }
    }

    /// The FR-005 partition is exactly 30 application languages and 20
    /// data-format entries over 50 canonical ids.
    #[test]
    fn partition_counts_match_fr005() {
        let application = LANGUAGE_RUNTIMES
            .iter()
            .filter(|(_, class, _)| *class == LanguageClass::Application)
            .count();
        let data_format = LANGUAGE_RUNTIMES
            .iter()
            .filter(|(_, class, _)| *class == LanguageClass::DataFormat)
            .count();
        assert_eq!(application, 30, "FR-005 application partition");
        assert_eq!(data_format, 20, "FR-005 data-format partition");
        assert_eq!(
            application + data_format,
            SUPPORTED_LANGUAGES.len(),
            "rows must total the scanner list length"
        );
    }

    /// `supported_language_rows` walks the scanner list in list order and
    /// resolves every id through the mapping table (FR-004/FR-005 derivation).
    #[test]
    fn supported_language_rows_walk_in_scanner_order() {
        let rows: Vec<(&str, Option<(LanguageClass, Option<LanguageRuntime>)>)> =
            supported_language_rows().collect();
        assert_eq!(
            rows.len(),
            SUPPORTED_LANGUAGES.len(),
            "one row per scanner id"
        );
        for (index, (id, resolved)) in rows.iter().enumerate() {
            assert_eq!(*id, SUPPORTED_LANGUAGES[index], "list order preserved");
            assert!(
                resolved.is_some(),
                "every scanner id must resolve via lookup (exhaustive mapping)"
            );
        }
        // The first scanner id is `rust`; spot-check the classification path.
        assert_eq!(rows[0].0, "rust");
        let (class, runtime) = rows[0].1.expect("rust resolves");
        assert_eq!(class, LanguageClass::Application);
        let (want_class, want_runtime) = lookup("rust").unwrap();
        assert_eq!(class, want_class);
        assert_eq!(
            runtime.map(|rt| rt.commands),
            want_runtime.map(|rt| rt.commands)
        );
    }

    /// The exhaustiveness checker's counting logic detects a gap: a table
    /// with one mapping row removed leaves some scanner id with zero rows.
    /// (The real table's clean state is asserted by
    /// `exhaustiveness_check_reports_no_violations`.)
    #[test]
    fn exhaustiveness_detects_gap_shape() {
        let mut table: Vec<(&str, LanguageClass, Option<LanguageRuntime>)> =
            LANGUAGE_RUNTIMES.to_vec();
        table.remove(0);
        let gap_detected = SUPPORTED_LANGUAGES
            .iter()
            .any(|lang| !table.iter().any(|(id, _, _)| *id == *lang));
        assert!(
            gap_detected,
            "removing a mapping row must leave a detectable gap"
        );
    }

    /// Application languages carry at least one runtime command; data-format
    /// entries carry none.
    #[test]
    fn application_languages_have_runtime_data_formats_do_not() {
        for (id, class, runtime) in LANGUAGE_RUNTIMES {
            match class {
                LanguageClass::Application => {
                    let rt = runtime.expect("application language must have a runtime");
                    assert!(!rt.commands.is_empty(), "{id:?} has empty runtime list");
                    assert!(
                        rt.commands.iter().all(|c| !c.is_empty()),
                        "{id:?} has an empty runtime command name"
                    );
                }
                LanguageClass::DataFormat => {
                    assert!(runtime.is_none(), "{id:?} is data-format but has a runtime");
                }
            }
        }
    }

    /// Version-flag overrides must target known runtime commands.
    #[test]
    fn version_flag_overrides_target_known_commands() {
        let known: std::collections::HashSet<&str> = LANGUAGE_RUNTIMES
            .iter()
            .filter_map(|(_, _, rt)| rt.as_ref())
            .flat_map(|rt| rt.commands.iter().copied())
            .collect();
        for o in VERSION_FLAG_OVERRIDES {
            assert!(
                known.contains(o.command),
                "override for {:?} targets an unknown runtime command",
                o.command
            );
            assert!(
                !o.flag.is_empty(),
                "override for {:?} has an empty flag",
                o.command
            );
        }
    }

    /// `lookup` resolves application and data-format ids, and unknown ids
    /// return `None`.
    #[test]
    fn lookup_classifies_known_and_unknown_ids() {
        let (class, runtime) = lookup("rust").expect("rust mapped");
        assert_eq!(class, LanguageClass::Application);
        let rt = runtime.expect("rust runtime present");
        assert!(rt.commands.contains(&"cargo"));
        assert!(rt.commands.contains(&"rustc"));
        assert!(is_application("rust"));
        assert!(is_application("python"));
        assert!(!is_application("toml"));
        assert!(!is_application("hcl"));
        assert!(lookup("notalanguage").is_none());
    }

    /// `version_flag_for` returns the override when present and the
    /// `--version` default otherwise.
    #[test]
    fn version_flag_defaults_and_overrides() {
        assert_eq!(version_flag_for("java"), "-version");
        assert_eq!(version_flag_for("erl"), "-version");
        assert_eq!(version_flag_for("lua"), "-v");
        assert_eq!(version_flag_for("luajit"), "-v");
        assert_eq!(version_flag_for("go"), "version");
        assert_eq!(version_flag_for("zig"), "version");
        assert_eq!(version_flag_for("cargo"), "--version");
        assert_eq!(version_flag_for("python3"), "--version");
    }

    /// The dotnet multi-arg probe override targets `dotnet` with
    /// `--list-sdks` + `--list-runtimes` (FR-008 notes).
    #[test]
    fn version_probe_args_override_covers_dotnet() {
        assert_eq!(
            VERSION_PROBE_ARGS_OVERRIDES,
            &[("dotnet", &["--list-sdks", "--list-runtimes"][..])]
        );
    }

    /// Multi-line probe output keeps every non-empty line (joined with
    /// `, `), so all installed SDK/runtime versions are reported — not just
    /// the first (e.g. `dotnet --list-sdks` lists 8.x and 9.x).
    #[test]
    fn probe_version_args_all_lines_keeps_every_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_script(
            &dir,
            "list-multi",
            "echo '8.0.130 [/opt/sdk]'\necho ''\necho '9.0.120 [/opt/sdk]'\necho 'noise' >&2",
        );
        assert_eq!(
            probe_version_args_all_lines(script.to_string_lossy().as_ref(), &["--list-sdks"]),
            VersionResult::Text("8.0.130 [/opt/sdk], 9.0.120 [/opt/sdk]".to_string()),
            "every non-empty stdout line must be kept"
        );
    }

    /// Write an executable shell script into a temp directory and return its
    /// path (test helper for probe-version fixtures).
    fn write_script(dir: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod script");
        }
        path
    }

    /// A version probe captures the first non-empty stdout line (FR-008).
    #[test]
    fn probe_version_captures_first_stdout_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_script(
            &dir,
            "versioned",
            "echo ''\necho '   '\necho 'tool 9.9 output'\necho 'noise' >&2",
        );
        assert_eq!(
            probe_version_args(script.to_string_lossy().as_ref(), &["--version"]),
            VersionResult::Text("tool 9.9 output".to_string()),
            "first non-empty line wins, blank lines skipped"
        );
    }

    /// A probe falls back to stderr when stdout carries no usable text
    /// (e.g. `erl -version` prints to stderr).
    #[test]
    fn probe_version_falls_back_to_stderr() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_script(&dir, "versioned-err", "echo 'Erlang style version' >&2");
        assert_eq!(
            probe_version_args(script.to_string_lossy().as_ref(), &["-version"]),
            VersionResult::Text("Erlang style version".to_string()),
            "stderr must be consulted when stdout is empty"
        );
    }

    /// Spawn failure and a non-zero exit without usable text yield
    /// `Unknown`, never a panic (FR-012).
    #[test]
    fn probe_version_reports_unknown_on_failure() {
        assert_eq!(
            probe_version_args("/nonexistent-dir-xyz/no-such-tool", &["--version"]),
            VersionResult::Unknown,
            "spawn failure must map to Unknown"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_script(&dir, "silent-failure", "exit 1");
        assert_eq!(
            probe_version_args(script.to_string_lossy().as_ref(), &["--version"]),
            VersionResult::Unknown,
            "non-zero exit without text must map to Unknown"
        );
    }

    /// A probe exceeding the per-probe timeout is killed and reported as
    /// `Timeout` (FR-012), within a bounded wall-clock window.
    #[test]
    fn probe_version_times_out_and_kills_child() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = write_script(&dir, "sleeper", "sleep 30");
        let started = std::time::Instant::now();
        let result = probe_version_args(script.to_string_lossy().as_ref(), &["--version"]);
        let elapsed = started.elapsed();
        assert_eq!(result, VersionResult::Timeout, "sleeper must time out");
        assert!(
            elapsed >= VERSION_PROBE_TIMEOUT,
            "timeout must not fire early (elapsed {elapsed:?})"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "probe must be killed promptly, not run to child completion (elapsed {elapsed:?})"
        );
    }

    /// `placeholder_or_text` maps outcomes to FR-012 report placeholders.
    #[test]
    fn version_result_placeholder_mapping() {
        assert_eq!(
            VersionResult::Text("1.2.3".to_string()).placeholder_or_text(),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            VersionResult::Unknown.placeholder_or_text(),
            Some("unknown".to_string())
        );
        assert_eq!(
            VersionResult::Timeout.placeholder_or_text(),
            Some("timeout".to_string())
        );
    }

    /// Renderer produces the FR-009 fenced ASCII-grid shape with the
    /// FR-017 fixed-width columns, summary line, and `From:` prefix for a
    /// hand-built probe set.
    #[test]
    fn render_markdown_report_table_summary_and_prefix() {
        let rows = vec![
            ReportRow {
                id: "rust",
                class: LanguageClass::Application,
                probes: vec![
                    CommandProbe {
                        command: "cargo",
                        installed: true,
                        version: Some("cargo 1.85.0".to_string()),
                    },
                    CommandProbe {
                        command: "rustc",
                        installed: true,
                        version: Some("rustc 1.85.0".to_string()),
                    },
                ],
            },
            ReportRow {
                id: "go",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "go",
                    installed: true,
                    version: Some("go version go1.22".to_string()),
                }],
            },
            ReportRow {
                id: "nim",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "nim",
                    installed: false,
                    version: None,
                }],
            },
            ReportRow {
                id: "toml",
                class: LanguageClass::DataFormat,
                probes: Vec::new(),
            },
        ];
        let report = render_markdown_report(&rows);
        assert!(report.starts_with("From: /toolchain list\n\n```\n"));
        // FR-017: fixed columns are 10/10/10/50; Language/Runtime cells are
        // clipped (the rust row's 12-char runtime clips to 10) and the Status
        // and Version columns word-wrap onto continuation grid lines (the
        // rust row's 34-char status wraps to 4 lines; the nim row's 13-char
        // status wraps to 2; the toml row's 27-char data-format marker wraps
        // to 4; every version cell here fits its 50-char column).
        let expected_row = |lang: &str, runtime: &str, status: &str, version: &str| {
            grid_line([(10, lang), (10, runtime), (10, status), (50, version)])
        };
        // First line of a multi-line row: clipped Language/Runtime plus the
        // first wrapped Status segment; the Version cell fits its column.
        assert!(
            report.contains(&expected_row("Language", "Runtime", "Status", "Version")),
            "fixed-width header row, got: {report}"
        );
        assert!(
            report.contains(&expected_row(
                "rust",
                "cargo, rus",
                "cargo:",
                "cargo: cargo 1.85.0; rustc: rustc 1.85.0"
            )),
            "rust first line, got: {report}"
        );
        assert!(report.contains(&expected_row("go", "go", "installed", "go version go1.22")));
        assert!(report.contains(&expected_row("nim", "nim", "not", "-")));
        assert!(report.contains(&expected_row("toml", "-", "(data", "-")));
        // Status continuation lines: blank Language/Runtime, wrapped segments.
        let cont = |status: &str| expected_row("", "", status, "");
        assert!(
            report.contains(&cont("installed;")),
            "rust cont 1: {report}"
        );
        assert!(report.contains(&cont("rustc:")), "rust cont 2: {report}");
        assert!(
            report.contains(&cont("installed")),
            "rust/nim last seg: {report}"
        );
        assert!(report.contains(&cont("format —")), "toml cont 1: {report}");
        assert!(report.contains(&cont("runtime")), "toml cont 2: {report}");
        assert!(report.contains(&cont("n/a)")), "toml cont 3: {report}");
        assert!(
            report.ends_with("\n2/3 application runtimes installed.\n```\n"),
            "summary line wrong: {report}"
        );
        // Pipe lines: header(1) + rust(4) + go(1) + nim(2) + toml(4).
        let row_lines = report.lines().filter(|l| l.starts_with('|')).count();
        assert_eq!(row_lines, 12, "wrapped row grid lines: {report}");
    }

    /// FR-017 word-wrap: a version cell wider than the 50-char column wraps
    /// onto continuation grid lines (blank Language/Runtime/Status cells) and
    /// the Language/Runtime columns keep clipping, so no version text is lost.
    #[test]
    fn render_markdown_report_version_cell_word_wraps() {
        let rows = vec![ReportRow {
            id: "python",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "python3",
                installed: true,
                version: Some(
                    "Python 3.12.4 (main, Jun  6 2024, long tail text that exceeds fifty)"
                        .to_string(),
                ),
            }],
        }];
        let report = render_markdown_report(&rows);
        let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
        // Header + 1 data row wrapping to 2 lines = 3 pipe lines total
        // (borders are `+-` lines, not pipe lines).
        assert_eq!(
            grid_lines.len(),
            3,
            "wrapped row spans 2 grid lines: {report}"
        );
        // First line carries the row id and the first wrapped segment.
        assert!(
            grid_lines[1].contains("python     ") && grid_lines[1].contains("Python 3.12.4 (main,"),
            "first wrapped line: {}",
            grid_lines[1]
        );
        // Continuation line: blank Language/Runtime/Status, remainder text.
        assert!(
            grid_lines[2].contains("that exceeds fifty)"),
            "continuation line carries the wrap remainder: {}",
            grid_lines[2]
        );
        let blank_prefix = "|            |            |            |";
        assert!(
            grid_lines[2].starts_with(blank_prefix),
            "continuation line blanks the first three columns: {}",
            grid_lines[2]
        );
        // Borders = data rows + 2 (2 borders above/below header + one row
        // border after each row group).
        let borders = report
            .lines()
            .filter(|l| l.trim_start().starts_with("+-"))
            .count();
        assert_eq!(borders, 3, "one border per row group plus 2: {report}");
    }

    /// Long unbroken version tokens hard-split across wrap lines so every
    /// line still fits the 50-char column.
    #[test]
    fn render_markdown_report_version_cell_hard_splits_long_tokens() {
        let long_token = "v".repeat(80);
        let rows = vec![ReportRow {
            id: "zig",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "zig",
                installed: true,
                version: Some(long_token.clone()),
            }],
        }];
        let report = render_markdown_report(&rows);
        let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
        assert_eq!(
            grid_lines.len(),
            3,
            "80-char token wraps to 2 lines: {report}"
        );
        assert!(
            grid_lines[1].contains(&long_token[..50]) && grid_lines[2].contains(&long_token[50..]),
            "token hard-splits at the column boundary: {report}"
        );
        // Every grid line stays 93 columns wide.
        for line in grid_lines {
            assert_eq!(line.chars().count(), 93, "grid line width: {line}");
        }
    }

    /// A report with zero application rows renders the `0/0` summary, and a
    /// data-format-only table never shows presence/version columns.
    #[test]
    fn render_markdown_report_empty_and_data_only() {
        // Build expected grid lines from the fixed column widths.
        let row = |widths: [usize; 4], cells: [&str; 4]| {
            grid_line([
                (widths[0], cells[0]),
                (widths[1], cells[1]),
                (widths[2], cells[2]),
                (widths[3], cells[3]),
            ])
        };

        let report = render_markdown_report(&[]);
        assert!(report.starts_with("From: /toolchain list\n\n```\n"));
        // Empty rows: headers pad inside the fixed 10/10/10/50 columns.
        assert!(
            report.contains(&row(
                [10, 10, 10, 50],
                ["Language", "Runtime", "Status", "Version"]
            )),
            "fixed-width header row, got: {report}"
        );
        assert!(report.ends_with("\n0/0 application runtimes installed.\n```\n"));

        let data_only = render_markdown_report(&[ReportRow {
            id: "yaml",
            class: LanguageClass::DataFormat,
            probes: Vec::new(),
        }]);
        // The 27-char data-format marker word-wraps in the Status column.
        assert!(
            data_only.contains(&row([10, 10, 10, 50], ["yaml", "-", "(data", "-"])),
            "data-format row first line, got: {data_only}"
        );
        assert!(
            data_only.contains(&row([10, 10, 10, 50], ["", "", "format —", ""])),
            "data-format marker wrap line 1, got: {data_only}"
        );
        assert!(
            data_only.contains(&row([10, 10, 10, 50], ["", "", "runtime", ""])),
            "data-format marker wrap line 2, got: {data_only}"
        );
        assert!(
            data_only.contains(&row([10, 10, 10, 50], ["", "", "n/a)", ""])),
            "data-format marker wrap line 3, got: {data_only}"
        );
        assert!(data_only.ends_with("\n0/0 application runtimes installed.\n```\n"));
    }

    /// Version text containing pipes or line breaks must not break the
    /// table grid (FR-009 single-line rendering via `cell`).
    #[test]
    fn render_markdown_report_sanitizes_version_text() {
        let rows = vec![ReportRow {
            id: "r",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "R",
                installed: true,
                version: Some("R version | 4.3\nwith newline\ttab".to_string()),
            }],
        }];
        let report = render_markdown_report(&rows);
        assert!(report.contains(r"R version \| 4.3 with newline tab"));
        assert!(report.ends_with("\n1/1 application runtimes installed.\n```\n"));
    }

    /// Row order is preserved in the rendered table (NFR-002 determinism).
    #[test]
    fn render_markdown_report_preserves_row_order() {
        let rows = vec![
            ReportRow {
                id: "python",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "python3",
                    installed: true,
                    version: Some("Python 3.12".to_string()),
                }],
            },
            ReportRow {
                id: "json",
                class: LanguageClass::DataFormat,
                probes: Vec::new(),
            },
            ReportRow {
                id: "zig",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "zig",
                    installed: false,
                    version: None,
                }],
            },
        ];
        let report = render_markdown_report(&rows);
        let python_pos = report.find("| python ").expect("python row");
        let json_pos = report.find("| json ").expect("json row");
        let zig_pos = report.find("| zig ").expect("zig row");
        assert!(python_pos < json_pos && json_pos < zig_pos, "order kept");
    }

    /// FR-017: the Language, Runtime, Status, and Version columns are fixed
    /// at 10/10/10/50 characters; Language/Runtime over-wide content clips to
    /// the column width, Status and Version word-wrap (FR-017).
    #[test]
    fn render_markdown_report_uses_fixed_column_widths() {
        let rows = vec![ReportRow {
            id: "rust",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "cargo",
                installed: true,
                version: Some("cargo 1.85.0".to_string()),
            }],
        }];
        let report = render_markdown_report(&rows);
        let widths: Vec<usize> = report
            .lines()
            .find(|l| l.starts_with("+--"))
            .expect("border row")
            .split('+')
            .filter(|s| !s.is_empty())
            .map(|seg| seg.len() - 2)
            .collect();
        assert_eq!(widths, vec![10, 10, 10, 50], "FR-017 widths: {report}");
        // A deliberately over-wide id clips to the 10-char Language column.
        let over = render_markdown_report(&[ReportRow {
            id: "verylonglanguageid",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "cargo",
                installed: true,
                version: Some("cargo 1.85.0".to_string()),
            }],
        }]);
        assert!(over.contains("| verylongla |"), "clip to 10: {over}");
    }

    /// FR-017: a status cell wider than the 10-char column word-wraps onto
    /// continuation grid lines (blank Language/Runtime cells) so no status
    /// text is clipped away.
    #[test]
    fn render_markdown_report_status_cell_word_wraps() {
        let rows = vec![ReportRow {
            id: "nim",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "nim",
                installed: false,
                version: None,
            }],
        }];
        let report = render_markdown_report(&rows);
        let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
        // Header + 1 data row wrapping to 2 lines = 3 pipe lines total
        // (borders are `+-` lines, not pipe lines).
        assert_eq!(
            grid_lines.len(),
            3,
            "wrapped row spans 2 grid lines: {report}"
        );
        // First line carries the row id/runtime and the first wrap segment.
        assert!(
            grid_lines[1].contains("| nim        | nim        | not        |"),
            "first wrapped line: {}",
            grid_lines[1]
        );
        // Continuation line: blank Language/Runtime, remainder in Status.
        assert!(
            grid_lines[2].contains("|            |            | installed  |"),
            "continuation line carries the wrap remainder: {}",
            grid_lines[2]
        );
        // Borders = data rows + 2 (2 borders above/below header + one row
        // border after each row group).
        let borders = report
            .lines()
            .filter(|l| l.trim_start().starts_with("+-"))
            .count();
        assert_eq!(borders, 3, "one border per row group plus 2: {report}");
    }

    /// JSON report contains the FR-015 schema: `languages` array with
    /// `id`/`runtime`/`status`/`version` objects plus `installed`/`total`
    /// summary fields, and parses as valid JSON (TC-009).
    #[test]
    fn render_json_report_schema_and_validity() {
        let rows = vec![
            ReportRow {
                id: "rust",
                class: LanguageClass::Application,
                probes: vec![
                    CommandProbe {
                        command: "cargo",
                        installed: true,
                        version: Some("cargo 1.85.0".to_string()),
                    },
                    CommandProbe {
                        command: "rustc",
                        installed: true,
                        version: Some("rustc 1.85.0".to_string()),
                    },
                ],
            },
            ReportRow {
                id: "yaml",
                class: LanguageClass::DataFormat,
                probes: Vec::new(),
            },
            ReportRow {
                id: "zig",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "zig",
                    installed: false,
                    version: None,
                }],
            },
        ];
        let report = render_json_report(&rows);
        let parsed: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
        let langs = parsed["languages"].as_array().expect("languages array");
        assert_eq!(langs.len(), 3, "one object per report row");
        let rust = &langs[0];
        assert_eq!(rust["id"], "rust");
        assert_eq!(rust["runtime"], "cargo, rustc");
        assert_eq!(rust["status"], "cargo: installed; rustc: installed");
        assert_eq!(rust["version"], "cargo: cargo 1.85.0; rustc: rustc 1.85.0");
        assert_eq!(langs[1]["status"], DATA_FORMAT_MARKER);
        assert_eq!(langs[2]["status"], "not installed");
        assert_eq!(langs[2]["version"], "-");
        assert_eq!(parsed["installed"], 1);
        assert_eq!(parsed["total"], 2, "application rows only");
        assert!(report.starts_with('{'), "bare document, no From: prefix");
    }

    /// JSON summary matches the markdown summary on identical rows, and the
    /// empty report renders a bare `{ languages: [], installed: 0, total: 0
    /// }` document (FR-009 / FR-015 parity).
    #[test]
    fn render_json_report_summary_parity_with_markdown() {
        let rows = vec![
            ReportRow {
                id: "python",
                class: LanguageClass::Application,
                probes: vec![CommandProbe {
                    command: "python3",
                    installed: true,
                    version: Some("Python 3.12".to_string()),
                }],
            },
            ReportRow {
                id: "json",
                class: LanguageClass::DataFormat,
                probes: Vec::new(),
            },
        ];
        let json = render_json_report(&rows);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["installed"], 1);
        assert_eq!(parsed["total"], 1);
        let markdown = render_markdown_report(&rows);
        assert!(
            markdown.contains("1/1 application runtimes installed."),
            "summary parity with JSON totals"
        );

        let empty: Vec<ReportRow> = Vec::new();
        let empty_json = render_json_report(&empty);
        let parsed: serde_json::Value = serde_json::from_str(&empty_json).expect("valid JSON");
        assert_eq!(parsed["languages"].as_array().map(Vec::len), Some(0));
        assert_eq!(parsed["installed"], 0);
        assert_eq!(parsed["total"], 0);
    }

    /// `resolve_in_dirs` finds an executable file in a supplied directory
    /// and skips non-executable files (FR-007, command -v semantics).
    #[test]
    fn resolve_in_dirs_finds_executable_skips_plain_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe = dir.path().join("mytool");
        std::fs::write(&exe, b"#!/bin/sh\n").expect("write exe");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))
                .expect("chmod exe");
        }
        let plain = dir.path().join("other");
        std::fs::write(&plain, b"data").expect("write plain");

        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(
            resolve_in_dirs("mytool", &dirs),
            Some(exe),
            "executable file must resolve"
        );
        assert_eq!(
            resolve_in_dirs("other", &dirs),
            None,
            "non-executable must not resolve"
        );
        assert_eq!(resolve_in_dirs("missing", &dirs), None);
    }

    /// `resolve_in_dirs` searches directories in order and stops at the
    /// first match.
    #[test]
    fn resolve_in_dirs_searches_dirs_in_order() {
        let first = tempfile::tempdir().expect("tempdir first");
        let second = tempfile::tempdir().expect("tempdir second");
        let exe = first.path().join("firsttool");
        std::fs::write(&exe, b"#!/bin/sh\n").expect("write");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        let dirs = vec![first.path().to_path_buf(), second.path().to_path_buf()];
        assert_eq!(resolve_in_dirs("firsttool", &dirs), Some(exe.clone()));

        // Empty/missing directory entries are skipped without error.
        let with_bad = vec![
            std::path::PathBuf::from("/nonexistent-dir-xyz"),
            first.path().to_path_buf(),
        ];
        assert_eq!(resolve_in_dirs("firsttool", &with_bad), Some(exe));
    }

    /// `resolve_in_dirs` rejects empty names and path-carrying commands —
    /// only bare command names are probed (FR-007).
    #[test]
    fn resolve_in_dirs_rejects_pathed_or_empty_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(resolve_in_dirs("", &dirs), None);
        assert_eq!(resolve_in_dirs("/bin/sh", &dirs), None);
        assert_eq!(resolve_in_dirs("./sh", &dirs), None);
        assert_eq!(resolve_in_dirs("../sh", &dirs), None);
    }

    /// `probe_installed` resolves a guaranteed-present system binary and
    /// reports `None` for a name that does not exist anywhere on `PATH`.
    #[test]
    fn probe_installed_resolves_real_path() {
        // /bin/ls exists on every supported Linux/macOS dev environment.
        let found = probe_installed("ls").or_else(|| probe_installed("sh"));
        assert!(found.is_some(), "a system binary must resolve on PATH");

        assert_eq!(
            probe_installed("definitely-not-a-real-tool-xyz"),
            None,
            "absent command must not resolve"
        );
    }

    /// Every application-language runtime command must be a bare command
    /// name (no path separators) so FR-007 probing applies uniformly.
    #[test]
    fn runtime_commands_are_bare_names() {
        for (id, _, rt) in LANGUAGE_RUNTIMES {
            let Some(rt) = rt else { continue };
            for cmd in rt.commands {
                assert!(!cmd.is_empty(), "{id:?} has an empty command");
                assert!(!cmd.contains('/'), "{id:?} command {cmd:?} carries a path");
            }
        }
    }

    /// The T-011 walk emits exactly one row per `SUPPORTED_LANGUAGES` entry,
    /// in list order, matching the mapping-table classification (FR-004).
    #[test]
    fn build_report_rows_covers_scanner_list_in_order() {
        let rows = build_report_rows();
        assert_eq!(
            rows.len(),
            SUPPORTED_LANGUAGES.len(),
            "one row per scanner id"
        );
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.id, SUPPORTED_LANGUAGES[index], "list order preserved");
            let (class, _) = lookup(row.id).expect("exhaustive mapping");
            assert_eq!(row.class, class, "classification from the mapping table");
        }
    }

    /// Application rows probe every mapped runtime command in FR-006 order;
    /// data-format rows carry no probes (FR-005 marker path).
    #[test]
    fn build_report_rows_probes_match_runtime_mapping() {
        let rows = build_report_rows();
        for row in &rows {
            let (_, runtime) = lookup(row.id).expect("exhaustive mapping");
            match runtime {
                None => {
                    assert!(
                        row.probes.is_empty(),
                        "data-format row {id:?} must carry no probes",
                        id = row.id
                    );
                }
                Some(rt) => {
                    let got: Vec<&str> = row.probes.iter().map(|p| p.command).collect();
                    assert_eq!(got, rt.commands, "probe order for {id:?}", id = row.id);
                    for probe in &row.probes {
                        if probe.installed {
                            // Found on PATH: version carries text or a
                            // FR-012 placeholder, never empty.
                            assert!(
                                probe.version.is_some(),
                                "{cmd:?} installed but no version text",
                                cmd = probe.command
                            );
                        } else {
                            assert!(
                                probe.version.is_none(),
                                "{cmd:?} absent but carries version text",
                                cmd = probe.command
                            );
                        }
                    }
                }
            }
        }
    }

    /// The rust row is a two-command multi-runtime row (FR-006): `cargo`
    /// and `rustc` in order, each reporting presence honestly — `cargo` is
    /// installed in every environment that builds this workspace, so its
    /// version probe must capture real text (spot-check against TC-001).
    #[test]
    fn build_report_rows_rust_row_probes_cargo_and_rustc() {
        let rows = build_report_rows();
        let rust = rows
            .iter()
            .find(|row| row.id == "rust")
            .expect("rust is in SUPPORTED_LANGUAGES");
        assert_eq!(rust.class, LanguageClass::Application);
        let commands: Vec<&str> = rust.probes.iter().map(|p| p.command).collect();
        assert_eq!(commands, ["cargo", "rustc"], "FR-006 order");
        for probe in &rust.probes {
            assert!(
                probe.installed,
                "{cmd:?} must resolve in this workspace",
                cmd = probe.command
            );
            let version = probe
                .version
                .as_deref()
                .expect("installed probe has version");
            assert!(
                !version.trim().is_empty(),
                "version text must be non-empty, got {version:?}"
            );
        }
    }

    /// Absent runtimes report `not installed` with no version and the walk
    /// continues past them (FR-010): a command name that cannot exist keeps
    /// the row shape intact.
    #[test]
    fn probe_command_absent_reports_not_installed_shape() {
        // Not reachable through `build_report_rows` (mapping commands are
        // real), so drive `probe_command`'s building blocks directly.
        assert_eq!(probe_installed("definitely-not-a-real-tool-xyz"), None);
        let rows: Vec<ReportRow> = supported_language_rows()
            .map(|(id, resolved)| {
                let (class, runtime) = resolved.expect("exhaustive mapping");
                let probes = runtime
                    .map(|rt| {
                        rt.commands
                            .iter()
                            .copied()
                            .map(|command| CommandProbe {
                                command,
                                installed: false,
                                version: None,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                ReportRow { id, class, probes }
            })
            .collect();
        assert_eq!(rows.len(), SUPPORTED_LANGUAGES.len());
        let rendered = render_markdown_report(&rows);
        assert!(rendered.contains("0/30 application runtimes installed."));
    }
}
