//! "Open in" targets for a session's directory (spec SESSIONMENU FR-06).
//!
//! Resolves the program to start for an editor, the system file manager or
//! a terminal, and starts it detached so the TUI never waits on it.
//! Resolution is a pure function of a [`LaunchEnv`] (OS, environment
//! variables, `PATH`), so it can be tested without touching the machine.
//!
//! - Editor: `$VISUAL`, else `$EDITOR`, opened on `.`. A terminal editor
//!   (vim, nano, helix, `emacs -nw`, ...) would fight the TUI for the
//!   terminal, so it is run inside a new terminal window instead.
//! - File manager: `xdg-open` (Linux), `open` (macOS), `explorer` (Windows).
//! - Terminal: `$TERMINAL`, else the first common terminal found on `PATH`
//!   (Linux); Terminal.app (macOS); `cmd` (Windows).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where to open a session's directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenTarget {
    /// The user's editor (`$VISUAL` / `$EDITOR`).
    Editor,
    /// The system file manager.
    FileManager,
    /// A new terminal window.
    Terminal,
}

impl OpenTarget {
    /// Menu label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::FileManager => "File manager",
            Self::Terminal => "Terminal",
        }
    }
}

/// A resolved program to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// Program name or path.
    pub program: String,
    /// Arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: PathBuf,
}

/// What resolution may look at.
#[derive(Debug, Clone, Default)]
pub struct LaunchEnv {
    /// `std::env::consts::OS` value: `linux`, `macos`, `windows`, ...
    pub os: String,
    /// Environment variables.
    pub vars: HashMap<String, String>,
    /// `PATH` directories, searched in order.
    pub path: Vec<PathBuf>,
}

impl LaunchEnv {
    /// The running process's OS, environment and `PATH`.
    #[must_use]
    pub fn current() -> Self {
        let path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        Self {
            os: std::env::consts::OS.to_string(),
            vars: std::env::vars().collect(),
            path,
        }
    }

    /// A non-empty variable.
    fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .filter(|v| !v.trim().is_empty())
    }

    /// Whether `program` is an executable file in a `PATH` directory.
    fn on_path(&self, program: &str) -> bool {
        self.path.iter().any(|dir| dir.join(program).is_file())
    }
}

/// Editors that draw in the terminal they are started from.
const TERMINAL_EDITORS: &[&str] = &[
    "vi", "vim", "nvim", "nano", "pico", "hx", "helix", "micro", "kak", "joe", "ne", "mg",
];

/// Linux terminals tried in order when `$TERMINAL` is not set.
const LINUX_TERMINALS: &[&str] = &[
    "x-terminal-emulator",
    "gnome-terminal",
    "konsole",
    "xfce4-terminal",
    "kitty",
    "alacritty",
    "foot",
    "wezterm",
    "xterm",
];

/// The editor command line (`$VISUAL`, else `$EDITOR`), split on whitespace.
fn editor_words(env: &LaunchEnv) -> Option<Vec<String>> {
    let line = env.var("VISUAL").or_else(|| env.var("EDITOR"))?;
    let words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    (!words.is_empty()).then_some(words)
}

/// Whether the editor command line runs inside a terminal.
fn is_terminal_editor(words: &[String]) -> bool {
    let name = Path::new(&words[0])
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let emacs_tty = name == "emacs"
        && words
            .iter()
            .any(|w| w == "-nw" || w == "--no-window-system");
    TERMINAL_EDITORS.contains(&name.as_str()) || emacs_tty
}

/// The terminal program and the arguments that make it run a command.
///
/// # Returns
///
/// `(program, exec_args)`: the command to run is appended after
/// `exec_args`. `None` when no terminal is known.
fn terminal(env: &LaunchEnv) -> Option<(String, Vec<String>)> {
    let program = match env.var("TERMINAL") {
        Some(t) => t.to_string(),
        None if env.os == "linux" => LINUX_TERMINALS
            .iter()
            .find(|t| env.on_path(t))
            .map(|t| (*t).to_string())?,
        None => return None,
    };
    let name = Path::new(&program)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let exec: &[&str] = match name.as_str() {
        "gnome-terminal" => &["--"],
        "kitty" | "foot" => &[],
        "wezterm" => &["start", "--"],
        _ => &["-e"],
    };
    Some((program, exec.iter().map(|s| (*s).to_string()).collect()))
}

/// Resolves what to start to open `dir` in `target`.
///
/// # Arguments
///
/// * `target` - editor, file manager or terminal.
/// * `dir` - the session's directory; also the working directory.
/// * `env` - OS, environment and `PATH` to resolve against.
///
/// # Returns
///
/// The program to start, or `None` when this machine has no such target
/// (no editor variable, no terminal found, an unsupported OS).
#[must_use]
pub fn launch_spec(target: OpenTarget, dir: &Path, env: &LaunchEnv) -> Option<LaunchSpec> {
    let spec = |program: String, args: Vec<String>| LaunchSpec {
        program,
        args,
        cwd: dir.to_path_buf(),
    };
    let dir_text = dir.display().to_string();
    match (target, env.os.as_str()) {
        (OpenTarget::FileManager, "macos") => Some(spec("open".into(), vec![dir_text])),
        (OpenTarget::FileManager, "windows") => Some(spec("explorer".into(), vec![dir_text])),
        (OpenTarget::FileManager, _) => Some(spec("xdg-open".into(), vec![dir_text])),
        (OpenTarget::Terminal, "macos") => Some(spec(
            "open".into(),
            vec!["-a".into(), "Terminal".into(), dir_text],
        )),
        (OpenTarget::Terminal, "windows") => Some(spec(
            "cmd".into(),
            vec!["/c".into(), "start".into(), "cmd".into()],
        )),
        (OpenTarget::Terminal, _) => terminal(env).map(|(program, _)| spec(program, Vec::new())),
        (OpenTarget::Editor, os) => {
            let mut words = editor_words(env)?;
            words.push(".".into());
            if !is_terminal_editor(&words) {
                let program = words.remove(0);
                return Some(spec(program, words));
            }
            match os {
                "windows" => {
                    let mut args = vec!["/c".into(), "start".into(), "cmd".into(), "/k".into()];
                    args.extend(words);
                    Some(spec("cmd".into(), args))
                }
                "macos" => None,
                _ => {
                    let (program, mut args) = terminal(env)?;
                    args.extend(words);
                    Some(spec(program, args))
                }
            }
        }
    }
}

/// The program that opens `url` in the default browser on `env.os`
/// (SESSIONMENU T4.4, `Open PR` / `Open MR`).
///
/// # Returns
///
/// `None` unless `url` is an `http://` or `https://` URL, so a stored value
/// can never start anything but a browser.
#[must_use]
pub fn url_spec(url: &str, env: &LaunchEnv) -> Option<LaunchSpec> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return None;
    }
    let program = match env.os.as_str() {
        "macos" => "open",
        "windows" => "explorer",
        _ => "xdg-open",
    };
    Some(LaunchSpec {
        program: program.to_string(),
        args: vec![url.to_string()],
        cwd: std::env::temp_dir(),
    })
}

/// Starts `spec` detached: no stdin/stdout/stderr, never waited on by the
/// caller (a background thread reaps it).
///
/// # Errors
///
/// Returns a message when the program cannot be started.
pub fn spawn(spec: &LaunchSpec) -> Result<(), String> {
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", spec.program))?;
    std::thread::spawn(move || {
        if let Err(e) = child.wait() {
            tracing::debug!(error = %e, "open-in child wait failed");
        }
    });
    Ok(())
}
