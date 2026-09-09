//! Tests for the `/new` TUI command surface (spec `newproj` T-011):
//! FR-001 trigger availability, FR-012 help surface, FR-003 validation
//! surface, FR-002 guard surface, and FR-014 foreground scaffold execution.
//!
//! Scaffold runs execute real file writes and real `git` inside a tempdir
//! that the test process chdirs into (guarded by a process-wide cwd mutex).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use ragent_tui::App;
use support::make_app;

mod support;

/// Serialize tests that change the process working directory. A poisoned
/// lock (a panicking sibling) must not cascade into unrelated failures, so
/// the guard ignores poison.
static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the cwd lock, ignoring a poisoned sibling test.
fn cwd_lock() -> MutexGuard<'static, ()> {
    CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run `/<cmd> help` and return the last assistant message text.
fn last_output(app: &mut App) -> String {
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

/// Drain the `/new` scaffold worker to completion (T-013/FR-014).
///
/// The scaffold pipeline runs on a worker thread so progress can stream;
/// tests drive the same `poll_newproj_result` loop the TUI event loop uses
/// until the run reaches its terminal state.
fn wait_newproj(app: &mut App) {
    for _ in 0..6000 {
        app.poll_newproj_result();
        if !app.newproj_running {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("/new scaffold worker did not reach a terminal state");
}

/// Restores the previous cwd on drop, so a panicking closure cannot leave
/// the process cwd inside a deleted tempdir and cascade failures into
/// sibling tests.
struct CwdGuard {
    prev: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Chdir into a fresh empty tempdir for the duration of the closure.
fn in_empty_cwd<T>(f: impl FnOnce(&Path) -> T) -> T {
    let _lock = cwd_lock();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let _guard = CwdGuard { prev };
    f(temp.path())
}

#[test]
fn test_new_trigger_registered_in_slash_commands() {
    assert!(
        ragent_tui::app::SLASH_COMMANDS
            .iter()
            .any(|cmd| cmd.trigger == "new"),
        "/new must be registered as a static slash trigger"
    );
}

#[test]
fn test_new_help_shows_usage_and_registry_values() {
    let mut app = make_app();
    app.execute_slash_command("/new help");
    let text = last_output(&mut app);
    assert!(text.contains("From: /new"));
    assert!(text.contains("--language"));
    assert!(text.contains("--type"));
    assert!(text.contains("--stack"));
    assert!(text.contains("--github"));
    assert!(text.contains("--gitlab"));
    // NFR-001: registry-derived value lists, not hardcoded prose.
    assert!(text.contains("rust"));
    assert!(text.contains("typescript"));
    assert!(text.contains("library"));
    assert!(text.contains("cmdline"));
    assert_eq!(app.status, "new: help");
}

#[test]
fn test_new_help_detailed_page_fully_documents_arguments() {
    // FR-018: the `/new help` page explains the purpose, documents every
    // argument (name, optionality, accepted values, omitted-default
    // behaviour), and includes at least two worked example invocations.
    let mut app = make_app();
    app.execute_slash_command("/new help");
    let text = last_output(&mut app);
    // The TUI surface renders the page through the markdown pipeline,
    // which re-wraps lines; phrase assertions run against a
    // whitespace-flattened view so wrapping cannot break matching.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // Purpose.
    assert!(text.contains("Purpose:"), "FR-018 purpose: {text}");
    assert!(text.contains("Scaffold a brand-new project"));

    // Per-argument docs: optionality and omitted defaults.
    assert!(flat.contains("Required. The computer language to scaffold"));
    assert!(flat.contains("Required. The kind of application to generate"));
    assert!(flat.contains("Optional. Layers a known framework starter"));
    assert!(flat.contains("Optional flag. Creates a private GitHub repository"));
    assert!(flat.contains("Omitted: validation error; nothing is created."));
    assert!(flat.contains("Omitted: no stack layer is applied."));
    assert!(flat.contains("Omitted: no remote is created and nothing is pushed"));
    assert!(flat.contains("mutually exclusive"));

    // NFR-001: registry-derived accepted values, including known stacks
    // from STACK_RECIPES (axum/warp/raylib/gtk4/ratatui for Rust).
    assert!(flat.contains("Accepted values: rust, python, go, typescript"));
    assert!(flat.contains("Accepted values: library, cmdline, tui, gui"));
    assert!(flat.contains("rust: axum, warp, raylib, gtk4, ratatui"));

    // At least two worked example invocations: one minimal, one hosted.
    assert!(text.contains("Worked examples:"), "FR-018 examples: {text}");
    assert!(flat.contains("/new --language rust --type cmdline"));
    assert!(flat.contains("/new --language python --type library --gitlab"));
    assert_eq!(app.status, "new: help");
}

#[test]
fn test_new_bare_invocation_is_help() {
    let mut app = make_app();
    app.execute_slash_command("/new");
    assert!(last_output(&mut app).contains("From: /new"));
    assert_eq!(app.status, "new: help");
}

#[test]
fn test_new_missing_flags_show_validation_error() {
    let mut app = make_app();
    app.execute_slash_command("/new --language rust");
    let text = last_output(&mut app);
    assert!(text.contains("missing required flag --type"));
    assert_eq!(app.status, "new: usage");
}

#[test]
fn test_new_unknown_flag_shows_validation_error() {
    let mut app = make_app();
    app.execute_slash_command("/new --language rust --type tui --bogus");
    let text = last_output(&mut app);
    assert!(text.contains("unknown flag '--bogus'"));
    assert_eq!(app.status, "new: usage");
}

#[test]
fn test_new_hosting_conflict_shows_conflict_error() {
    let mut app = make_app();
    app.execute_slash_command("/new --language rust --type cmdline --github --gitlab");
    let text = last_output(&mut app);
    assert!(text.contains("mutually exclusive"));
    assert_eq!(app.status, "new: usage");
}

#[test]
fn test_new_nonempty_directory_refuses() {
    in_empty_cwd(|dir| {
        std::fs::write(dir.join("stray-file.txt"), b"occupied").expect("seed file");
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline");
        let text = last_output(&mut app);
        assert!(text.contains("directory is not empty"));
        assert!(text.contains("stray-file.txt"));
        assert_eq!(app.status, "new: directory not empty");
        // Zero file mutations beyond the seeded entry.
        assert!(!dir.join("Cargo.toml").exists());
        assert!(!dir.join(".git").exists());
    });
}

#[test]
fn test_new_scaffolds_project_foreground() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline");
        wait_newproj(&mut app);

        // FR-014: foreground — no sub-agent processing state.
        assert!(!app.is_processing, "/new must run in the foreground");

        // FR-005/FR-006 code artifacts.
        assert!(dir.join("Cargo.toml").exists());
        assert!(dir.join("src/main.rs").exists());
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("read manifest");
        assert!(manifest.contains(dir.file_name().expect("dirname").to_string_lossy().as_ref()));

        // FR-004 workspace artifacts.
        assert!(dir.join(".ragent/agents").is_dir());
        assert!(dir.join("specs").is_dir());
        assert!(dir.join("log").is_dir());
        let gitignore = std::fs::read_to_string(dir.join(".gitignore")).expect("gitignore");
        assert!(gitignore.contains(".ragent/"));
        assert!(dir.join("AGENTS.md").exists());

        // FR-008 local half: repo initialised with the initial commit.
        assert!(dir.join(".git").is_dir());

        // FR-011 summary text.
        let text = last_output(&mut app);
        assert!(text.contains("From: /new"));
        assert!(text.contains("Scaffolded"));
        assert!(text.contains("Created"));
        assert!(text.contains("initialised new repository"));
        assert!(text.contains("Remote: none"));
        assert!(app.status.starts_with("new: scaffolded"));
    });
}

#[test]
fn test_new_github_hosting_without_token_fails_contained() {
    in_empty_cwd(|dir| {
        // Make the GitHub token resolution deterministic: no env token, and
        // a HOME without the credential file. The remote-init step then
        // fails contained (FR-010) without any network access.
        struct EnvGuard {
            saved: Vec<(&'static str, Option<String>)>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // SAFETY: test-process-only env restoration; the workspace
                // denies `unsafe_code`, so the allowance is scoped to this
                // test-only guard.
                #[allow(unsafe_code)]
                unsafe {
                    for (key, value) in &self.saved {
                        match value {
                            Some(val) => std::env::set_var(key, val),
                            None => std::env::remove_var(key),
                        }
                    }
                }
            }
        }
        let saved = vec![
            ("GITHUB_TOKEN", std::env::var("GITHUB_TOKEN").ok()),
            ("HOME", std::env::var("HOME").ok()),
        ];
        let _env = EnvGuard { saved };
        // SAFETY: test-process-only env mutation, restored by EnvGuard.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            std::env::set_var("HOME", dir);
        }

        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --github");
        wait_newproj(&mut app);

        // FR-010: the local scaffold completed and is reported, the failed
        // remote step is named with actionable remediation, and no partial
        // remote state exists.
        let text = last_output(&mut app);
        assert!(
            text.contains("Scaffolded"),
            "local half must complete: {text}"
        );
        assert!(text.contains("Remote: failed at github auth"));
        assert!(text.contains("/github login"));
        assert!(text.contains("local scaffold is intact"));
        assert!(dir.join(".git").is_dir());
        let origin = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(dir)
            .output()
            .expect("git spawn");
        assert!(
            !origin.status.success(),
            "failed auth must not register a remote"
        );
        assert!(app.status.starts_with("new: scaffolded"));
    });
}

#[test]
fn test_new_gitlab_hosting_without_token_fails_contained() {
    in_empty_cwd(|dir| {
        // Make the GitLab token resolution deterministic: no env token, and
        // a HOME without the credential file. The remote-init step then
        // fails contained (FR-010) without any network access.
        struct EnvGuard {
            saved: Vec<(&'static str, Option<String>)>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // SAFETY: test-process-only env restoration; the workspace
                // denies `unsafe_code`, so the allowance is scoped to this
                // test-only guard.
                #[allow(unsafe_code)]
                unsafe {
                    for (key, value) in &self.saved {
                        match value {
                            Some(val) => std::env::set_var(key, val),
                            None => std::env::remove_var(key),
                        }
                    }
                }
            }
        }
        let saved = vec![
            ("GITLAB_TOKEN", std::env::var("GITLAB_TOKEN").ok()),
            ("GITLAB_URL", std::env::var("GITLAB_URL").ok()),
            ("HOME", std::env::var("HOME").ok()),
        ];
        let _env = EnvGuard { saved };
        // SAFETY: test-process-only env mutation, restored by EnvGuard.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("GITLAB_TOKEN");
            std::env::remove_var("GITLAB_URL");
            std::env::set_var("HOME", dir);
        }

        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --gitlab");
        wait_newproj(&mut app);

        // FR-010: the local scaffold completed and is reported, the failed
        // remote step is named with actionable remediation, and no partial
        // remote state exists.
        let text = last_output(&mut app);
        assert!(
            text.contains("Scaffolded"),
            "local half must complete: {text}"
        );
        assert!(text.contains("Remote: failed at gitlab auth"));
        assert!(text.contains("/gitlab setup"));
        assert!(text.contains("local scaffold is intact"));
        assert!(dir.join(".git").is_dir());
        let origin = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(dir)
            .output()
            .expect("git spawn");
        assert!(
            !origin.status.success(),
            "failed auth must not register a remote"
        );
        assert!(app.status.starts_with("new: scaffolded"));
    });
}

// ---------------------------------------------------- FR-007 overlays ---

#[test]
fn test_new_known_stack_layers_dependency_and_snippet() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --stack axum");
        wait_newproj(&mut app);

        // Dependency appended to the manifest (FR-007).
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        assert!(manifest.contains("axum = \"0.7\""));
        assert!(manifest.contains("tokio = { version = \"1\""));

        // Import block + framework starter layered on the source.
        let source = std::fs::read_to_string(dir.join("src/main.rs")).expect("source");
        assert!(source.starts_with("use axum::routing::get;\nuse axum::Router;\n"));
        assert!(source.contains("Hello, world!"));
        assert!(source.contains("axum::serve"));
    });
}

#[test]
fn test_new_unknown_stack_warns_and_continues_with_base_layout() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --stack nosuchstack");
        wait_newproj(&mut app);

        // FR-007: warn and continue with the base layout — the scaffold
        // still completes.
        let text = last_output(&mut app);
        assert!(text.contains("[warn] unknown stack 'nosuchstack'"));
        assert!(text.contains("continuing with the base layout"));
        assert!(text.contains("Scaffolded"));
        assert!(app.status.starts_with("new: scaffolded"));

        // Base layout verbatim: no framework dependency or snippet.
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        assert!(!manifest.contains("axum"));
        let source = std::fs::read_to_string(dir.join("src/main.rs")).expect("source");
        assert!(source.contains("Hello, world!"));
        assert!(!source.contains("axum"));
    });
}

#[test]
fn test_new_stack_case_insensitive_and_docs_match_recipe() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --stack AXUM");
        wait_newproj(&mut app);

        // Case-insensitive registry match applies the overlay.
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        assert!(manifest.contains("axum = \"0.7\""));
        let text = last_output(&mut app);
        assert!(text.contains("stack: axum"));
        assert!(!text.contains("[warn]"));

        // NFR-002: STATS records the applied stack.
        let stats = std::fs::read_to_string(dir.join("STATS.md")).expect("stats");
        assert!(stats.contains("axum"));
    });
}

#[test]
fn test_new_stack_rerun_never_overwrites_overlay_files() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --stack warp");
        wait_newproj(&mut app);
        let manifest_first = std::fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");

        // FR-016: a second run in the same (now non-empty) directory is
        // refused by the guard before any writes.
        let mut app2 = make_app();
        app2.execute_slash_command("/new --language rust --type cmdline --stack warp");
        wait_newproj(&mut app2);
        let text2 = last_output(&mut app2);
        assert!(text2.contains("directory is not empty"));

        // The first-run overlay output is untouched.
        let manifest_second = std::fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        assert_eq!(manifest_first, manifest_second);
        assert_eq!(manifest_second.matches("warp = \"0.3\"").count(), 1);
    });
}
// ------------------------------------------------- T-013/FR-014 streaming ---

#[test]
fn test_new_streams_progress_lines_into_message_window() {
    in_empty_cwd(|dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline");
        wait_newproj(&mut app);

        // The run start is visible immediately: the seeded progress message
        // carries the `Scaffolding `slug`...` header (T-013).
        let started = app
            .messages
            .iter()
            .any(|m| m.text_content().contains("Scaffolding '"));
        assert!(started, "run-start progress message must be present");

        // Streamed step lines land in the message window: guard, emission,
        // git, and completion markers are all rendered.
        let streamed = app
            .messages
            .iter()
            .any(|m| m.text_content().contains("[ ok ]"));
        assert!(streamed, "streamed [ ok ] step lines must be present");

        // FR-011: the final summary lands in its own message with the
        // full report content.
        let summary = app
            .messages
            .iter()
            .find(|m| m.text_content().contains("Scaffolded "))
            .map(|m| m.text_content())
            .expect("summary message");
        assert!(summary.contains("Created"));
        assert!(summary.contains("Git: initialised new repository"));
        assert!(summary.contains("Remote: none"));
        assert!(dir.join("Cargo.toml").exists());
        assert_eq!(app.status, "new: scaffolded");
    });
}

#[test]
fn test_new_progress_updates_one_message_in_place() {
    in_empty_cwd(|_dir| {
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline");
        wait_newproj(&mut app);

        // The progress panel updates a single message in place: exactly one
        // message carries the `Scaffolding 'slug'...` header (no stacking of
        // one message per streamed step), and it accumulates every step
        // marker.
        let panels: Vec<String> = app
            .messages
            .iter()
            .filter(|m| m.text_content().contains("Scaffolding '"))
            .map(|m| m.text_content())
            .collect();
        assert_eq!(panels.len(), 1, "progress panel must update in place");
        let panel = &panels[0];
        assert!(panel.contains("[ ok ]"), "completed steps are rendered");
        assert!(panel.contains("writing project files"));
        assert!(panel.contains("initialising git repository"));
        assert!(panel.contains("scaffold complete"));
        // The streamed panel is separate from the FR-011 summary message.
        let summary_count = app
            .messages
            .iter()
            .filter(|m| m.text_content().contains("Scaffolded "))
            .count();
        assert_eq!(summary_count, 1);
    });
}

#[test]
fn test_new_guard_refusal_is_synchronous_without_streaming() {
    in_empty_cwd(|dir| {
        std::fs::write(dir.join("stray-file.txt"), b"occupied").expect("seed file");
        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline");

        // The FR-002 guard runs inline: the refusal is present without any
        // poll loop, no worker was spawned, and no progress panel exists.
        let text = last_output(&mut app);
        assert!(text.contains("directory is not empty"));
        assert!(text.contains("stray-file.txt"));
        assert!(
            !app.newproj_running,
            "guard refusal must not spawn a worker"
        );
        assert!(
            !app.messages
                .iter()
                .any(|m| m.text_content().contains("Scaffolding '")),
            "no progress panel for a refused run"
        );
        assert_eq!(app.status, "new: directory not empty");
    });
}

#[test]
fn test_new_remote_failure_streams_contained_summary() {
    in_empty_cwd(|dir| {
        struct EnvGuard {
            saved: Vec<(&'static str, Option<String>)>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // SAFETY: test-process-only env restoration; the workspace
                // denies `unsafe_code`, so the allowance is scoped to this
                // test-only guard.
                #[allow(unsafe_code)]
                unsafe {
                    for (key, value) in &self.saved {
                        match value {
                            Some(val) => std::env::set_var(key, val),
                            None => std::env::remove_var(key),
                        }
                    }
                }
            }
        }
        let saved = vec![
            ("GITHUB_TOKEN", std::env::var("GITHUB_TOKEN").ok()),
            ("HOME", std::env::var("HOME").ok()),
        ];
        let _env = EnvGuard { saved };
        // SAFETY: test-process-only env mutation, restored by EnvGuard.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            std::env::set_var("HOME", dir);
        }

        let mut app = make_app();
        app.execute_slash_command("/new --language rust --type cmdline --github");
        wait_newproj(&mut app);

        // T-013: the streamed run-start header plus step lines are present,
        // and the contained remote failure is reported in the summary.
        assert!(
            app.messages
                .iter()
                .any(|m| m.text_content().contains("Scaffolding '")),
            "streamed progress panel must exist for a hosting run"
        );
        let summary = app
            .messages
            .iter()
            .find(|m| m.text_content().contains("Scaffolded "))
            .map(|m| m.text_content())
            .expect("summary message");
        assert!(summary.contains("Remote: failed at github auth"));
        assert!(summary.contains("local scaffold is intact"));
        assert!(dir.join(".git").is_dir());
        assert_eq!(app.status, "new: scaffolded");
    });
}
