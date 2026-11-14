//! "Open in" program resolution (spec SESSIONMENU FR-06, milestone M3).
//!
//! Resolution is a pure function of a `LaunchEnv`, so every case builds its
//! own environment; nothing is started. The fake `PATH` directory lives under
//! the git-ignored `target/temp/` tree in a directory unique to each test.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ragent_tui::open_in::{LaunchEnv, LaunchSpec, OpenTarget, launch_spec};

const DIR: &str = "/work/project";

/// An environment for `os` with `vars` and an empty `PATH`.
fn env(os: &str, vars: &[(&str, &str)]) -> LaunchEnv {
    LaunchEnv {
        os: os.to_string(),
        vars: vars
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<HashMap<_, _>>(),
        path: Vec::new(),
    }
}

/// A `PATH` directory unique to `name` holding empty files named `programs`.
fn fake_path(name: &str, programs: &[&str]) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/open_in")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fake path dir");
    for program in programs {
        std::fs::write(dir.join(program), "").expect("fake program");
    }
    dir
}

fn resolve(target: OpenTarget, env: &LaunchEnv) -> Option<LaunchSpec> {
    launch_spec(target, Path::new(DIR), env)
}

fn args(spec: &LaunchSpec) -> Vec<&str> {
    spec.args.iter().map(String::as_str).collect()
}

#[test]
fn test_open_in_file_manager_per_os() {
    for (os, program) in [
        ("linux", "xdg-open"),
        ("macos", "open"),
        ("windows", "explorer"),
    ] {
        let spec = resolve(OpenTarget::FileManager, &env(os, &[])).expect(os);
        assert_eq!(spec.program, program, "{os}");
        assert_eq!(args(&spec), [DIR], "{os}");
        assert_eq!(spec.cwd, Path::new(DIR));
    }
}

#[test]
fn test_open_in_gui_editor_prefers_visual() {
    let e = env("linux", &[("VISUAL", "code -w"), ("EDITOR", "vim")]);
    let spec = resolve(OpenTarget::Editor, &e).expect("editor");
    assert_eq!(spec.program, "code");
    assert_eq!(args(&spec), ["-w", "."]);

    let blank_visual = env("linux", &[("VISUAL", "  "), ("EDITOR", "gedit")]);
    let spec = resolve(OpenTarget::Editor, &blank_visual).expect("editor");
    assert_eq!(spec.program, "gedit", "a blank VISUAL falls back to EDITOR");
}

#[test]
fn test_open_in_no_editor_variable_hides_editor() {
    assert!(resolve(OpenTarget::Editor, &env("linux", &[])).is_none());
}

#[test]
fn test_open_in_terminal_editor_runs_inside_terminal() {
    let e = env("linux", &[("EDITOR", "nvim"), ("TERMINAL", "kitty")]);
    let spec = resolve(OpenTarget::Editor, &e).expect("editor");
    assert_eq!(spec.program, "kitty");
    assert_eq!(args(&spec), ["nvim", "."]);

    let emacs_tty = env("linux", &[("EDITOR", "emacs -nw"), ("TERMINAL", "xterm")]);
    let spec = resolve(OpenTarget::Editor, &emacs_tty).expect("editor");
    assert_eq!(spec.program, "xterm");
    assert_eq!(args(&spec), ["-e", "emacs", "-nw", "."]);

    let emacs_gui = env("linux", &[("EDITOR", "emacs")]);
    let spec = resolve(OpenTarget::Editor, &emacs_gui).expect("editor");
    assert_eq!(spec.program, "emacs", "GUI emacs is started directly");
}

#[test]
fn test_open_in_terminal_found_on_path() {
    let mut e = env("linux", &[("EDITOR", "vim")]);
    e.path = vec![fake_path("gnome", &["gnome-terminal", "xterm"])];

    let terminal = resolve(OpenTarget::Terminal, &e).expect("terminal");
    assert_eq!(
        terminal.program, "gnome-terminal",
        "first in preference order"
    );
    // Several terminals are servers whose child keeps the server's working
    // directory, so the directory is named by starting a login shell there.
    assert_eq!(args(&terminal), ["--", "/bin/sh", "-l"]);
    let editor = resolve(OpenTarget::Editor, &e).expect("editor");
    assert_eq!(editor.program, "gnome-terminal");
    assert_eq!(args(&editor), ["--", "vim", "."]);
}

#[test]
fn test_open_in_no_terminal_hides_terminal_and_terminal_editor() {
    let mut e = env("linux", &[("EDITOR", "vim")]);
    e.path = vec![fake_path("empty", &[])];
    assert!(resolve(OpenTarget::Terminal, &e).is_none());
    assert!(resolve(OpenTarget::Editor, &e).is_none());
}

#[test]
fn test_open_in_macos_terminal_and_terminal_editor() {
    let e = env("macos", &[("EDITOR", "vim")]);
    let terminal = resolve(OpenTarget::Terminal, &e).expect("terminal");
    assert_eq!(terminal.program, "open");
    assert_eq!(args(&terminal), ["-a", "Terminal", DIR]);
    assert!(
        resolve(OpenTarget::Editor, &e).is_none(),
        "a terminal editor cannot be started in Terminal.app"
    );
}

#[test]
fn test_open_in_prefers_the_desktop_terminal_over_the_fallback_list() {
    // The fallback list would reach for `x-terminal-emulator`/`gnome-terminal`;
    // the desktop the TUI runs in names XFCE, so `xfce4-terminal` wins.
    let mut e = env("linux", &[("XDG_CURRENT_DESKTOP", "XFCE")]);
    e.path = vec![fake_path(
        "xfce",
        &["gnome-terminal", "x-terminal-emulator", "xfce4-terminal"],
    )];
    let terminal = resolve(OpenTarget::Terminal, &e).expect("terminal");
    assert_eq!(terminal.program, "xfce4-terminal");
    assert_eq!(args(&terminal), ["-e", "/bin/sh", "-l"]);
}

#[test]
fn test_open_in_desktop_entries_are_matched_case_insensitively() {
    // Ubuntu spells it `ubuntu:GNOME`; KDE's session name is `plasma`.
    let mut e = env("linux", &[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]);
    e.path = vec![fake_path(
        "ubuntu-gnome",
        &["gnome-terminal", "ptyxis", "konsole"],
    )];
    assert_eq!(
        resolve(OpenTarget::Terminal, &e).expect("terminal").program,
        "ptyxis",
        "GNOME 45+ ships ptyxis, which outranks gnome-terminal"
    );

    let mut plasma = env("linux", &[("XDG_SESSION_DESKTOP", "plasma")]);
    plasma.path = vec![fake_path("plasma", &["konsole", "x-terminal-emulator"])];
    assert_eq!(
        resolve(OpenTarget::Terminal, &plasma)
            .expect("terminal")
            .program,
        "konsole"
    );
}

#[test]
fn test_open_in_desktop_terminal_falls_through_when_not_installed() {
    // The desktop is XFCE but xfce4-terminal is not installed: the generic
    // alternatives link is the next best answer, not "no terminal".
    let mut e = env("linux", &[("XDG_CURRENT_DESKTOP", "XFCE")]);
    e.path = vec![fake_path("xfce-missing", &["x-terminal-emulator"])];
    assert_eq!(
        resolve(OpenTarget::Terminal, &e).expect("terminal").program,
        "x-terminal-emulator"
    );
}

#[test]
fn test_open_in_terminal_env_var_beats_desktop_detection() {
    // `$TERMINAL` is an explicit user instruction and outranks every guess.
    let mut e = env(
        "linux",
        &[("XDG_CURRENT_DESKTOP", "XFCE"), ("TERMINAL", "alacritty")],
    );
    e.path = vec![fake_path("explicit", &["xfce4-terminal", "alacritty"])];
    assert_eq!(
        resolve(OpenTarget::Terminal, &e).expect("terminal").program,
        "alacritty"
    );
}

#[test]
fn test_open_in_prefers_xdg_terminal_exec_when_present() {
    // `xdg-terminal-exec` reads the user's own configuration, so it outranks
    // the desktop guess; it takes the command without an exec separator.
    let mut e = env("linux", &[("XDG_CURRENT_DESKTOP", "GNOME")]);
    e.path = vec![fake_path(
        "xdg-exec",
        &["xdg-terminal-exec", "gnome-terminal"],
    )];
    let terminal = resolve(OpenTarget::Terminal, &e).expect("terminal");
    assert_eq!(terminal.program, "xdg-terminal-exec");
    assert_eq!(args(&terminal), ["/bin/sh", "-l"]);
}

#[test]
fn test_open_in_uses_the_current_term_program_as_a_last_resort() {
    // No `$TERMINAL`, no desktop variable and nothing from the fallback list:
    // the terminal the TUI itself runs in is still a good answer.
    let mut e = env("linux", &[("TERM_PROGRAM", "ghostty")]);
    e.path = vec![fake_path("term-program", &["ghostty"])];
    assert_eq!(
        resolve(OpenTarget::Terminal, &e).expect("terminal").program,
        "ghostty"
    );
}

#[test]
fn test_open_in_term_program_must_be_installed() {
    let mut e = env("linux", &[("TERM_PROGRAM", "ghostty")]);
    e.path = vec![fake_path("term-program-missing", &[])];
    assert!(resolve(OpenTarget::Terminal, &e).is_none());
}
