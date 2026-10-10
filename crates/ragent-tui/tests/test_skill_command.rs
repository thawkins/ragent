//! `/skill` (and `/skills`) TUI integration tests (spec `openhands` T-013;
//! FR-005).
//!
//! Drives the `/skill` dispatch arm through `App::execute_slash_command`: the
//! list form (`/skill`, `/skill list`, `/skills`) must render an AgentSkills
//! pack discovered under the project `.agents/skills/` directory, the load form
//! (`/skill <name>`) must start the invocation, and a name clash must prefer the
//! existing higher-scope ragent pack (A7).
//!
//! The command reads the working directory and the resolved config, so every
//! test enters an isolated tempdir project (holding `.ragent/ragent.json` and
//! the skill packs) under a process-global mutex that serialises the cwd swap.

use std::sync::{Mutex, OnceLock};

use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};

#[path = "support/mod.rs"]
mod support;

/// The cwd swap below is process-global; one mutex serialises every test in
/// this binary against it.
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Restores the original working directory on drop.
struct CwdGuard {
    cwd: std::path::PathBuf,
}

impl CwdGuard {
    fn new() -> Self {
        // A prior test can momentarily leave the cwd pointing at a
        // just-deleted tempdir; fall back to `.` so setup never panics.
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Self { cwd }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// A tempdir project ready for skill discovery: an empty `skill_dirs` config,
/// plus whatever packs the test writes under it.
///
/// Field order is drop order: the cwd is restored *before* the mutex is
/// released, so the next test never acquires the lock with the cwd still
/// pointing at this (about-to-be-deleted) tempdir.
struct SkillProject {
    _cwd: CwdGuard,
    _guard: std::sync::MutexGuard<'static, ()>,
    temp: tempfile::TempDir,
}

impl SkillProject {
    /// Enter a fresh tempdir project with an isolated config.
    fn new() -> Self {
        let guard = test_lock();
        let cwd = CwdGuard::new();
        let temp = tempfile::tempdir().expect("tempdir");

        let ragent_dir = temp.path().join(".ragent");
        std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
        // An explicit empty `skill_dirs` so a developer's global config cannot
        // inject extra discovery roots into the assertions.
        std::fs::write(ragent_dir.join("ragent.json"), r#"{"skill_dirs": []}"#)
            .expect("write ragent.json");

        std::env::set_current_dir(temp.path()).expect("set cwd");

        Self {
            _guard: guard,
            _cwd: cwd,
            temp,
        }
    }

    /// Write `<root>/<dir_name>/SKILL.md` with the given file contents.
    fn write_pack(&self, root: &[&str], dir_name: &str, contents: &str) {
        let pack = root
            .iter()
            .fold(self.temp.path().to_path_buf(), |p, seg| p.join(seg))
            .join(dir_name);
        std::fs::create_dir_all(&pack).expect("create pack dir");
        std::fs::write(pack.join("SKILL.md"), contents).expect("write SKILL.md");
    }

    /// Build an `App` bound to this project.
    fn app(&self) -> App {
        let mut app = support::make_app();
        // The slash-command gate creates a session on demand; a pre-set id
        // keeps the invocation form from touching session storage.
        app.session_id = Some("skill-test-session".to_string());
        app
    }
}

/// The text of the most recently appended message.
fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// Every message body currently in the transcript.
fn all_message_text(app: &App) -> String {
    app.messages
        .iter()
        .map(ragent_types::message::Message::text_content)
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_help_renders_usage() {
    let project = SkillProject::new();
    let mut app = project.app();

    app.execute_slash_command("/skill help").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /skill help") && text.contains("/skill <name>"),
        "help must render the usage table, got: {text}"
    );
    assert_eq!(app.status, "skill: help", "/skill help status");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_list_shows_agentskills_pack() {
    let project = SkillProject::new();
    project.write_pack(
        &[".agents", "skills"],
        "tiny",
        "---\nname: tiny\ndescription: A trivial pack used by the manual test plan.\n---\n\n# Tiny skill\n\nWhen asked, reply with exactly: TINY-OK\n",
    );
    let mut app = project.app();

    app.execute_slash_command("/skill").await;

    let text = last_message_text(&app);
    assert!(text.contains("From: /skill"), "list header, got: {text}");
    assert!(
        text.contains("/tiny"),
        "the AgentSkills pack must appear in the list, got: {text}"
    );
    assert!(
        text.contains("A trivial pack used by the manual test plan."),
        "frontmatter description must be rendered, got: {text}"
    );
    assert!(
        text.contains("scope: openskills-project"),
        "a .agents/skills/ pack loads at project scope, got: {text}"
    );
    assert_eq!(app.status, "skill", "/skill list status");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_list_via_list_subcommand_and_skills_alias() {
    let project = SkillProject::new();
    project.write_pack(
        &[".agents", "skills"],
        "tiny",
        "---\nname: tiny\ndescription: A trivial pack used by the manual test plan.\n---\nBody\n",
    );

    // `/skill list` and the legacy `/skills` both render the same pack.
    let mut app = project.app();
    app.execute_slash_command("/skill list").await;
    let text = last_message_text(&app);
    assert!(
        text.contains("From: /skill") && text.contains("/tiny"),
        "`/skill list` must render the pack, got: {text}"
    );

    let mut app = project.app();
    app.execute_slash_command("/skills").await;
    let text = last_message_text(&app);
    assert!(
        text.contains("From: /skills") && text.contains("/tiny"),
        "`/skills` must render the pack, got: {text}"
    );
    assert_eq!(app.status, "skills", "/skills status");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_load_starts_invocation() {
    let project = SkillProject::new();
    project.write_pack(
        &[".agents", "skills"],
        "tiny",
        "---\nname: tiny\ndescription: A trivial pack.\n---\n\nReply with exactly: TINY-OK\n",
    );
    let mut app = project.app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.selected_model = Some("ollama/qwen3:latest".to_string());

    app.execute_slash_command("/skill tiny").await;

    assert_eq!(
        app.status, "invoking skill /tiny...",
        "loading a pack must start the invocation"
    );
    assert!(
        all_message_text(&app).contains("/tiny"),
        "the load form must echo the invoked pack, got: {}",
        all_message_text(&app)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_unknown_pack_reports_not_found() {
    let project = SkillProject::new();
    let mut app = project.app();

    app.execute_slash_command("/skill nosuchpack").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("No skill named `nosuchpack`"),
        "an unknown pack must report not-found, got: {text}"
    );
    assert_eq!(app.status, "skill: 'nosuchpack' not found");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_skill_clash_prefers_existing_pack() {
    let project = SkillProject::new();
    // Existing ragent project pack (`.ragent/skills/`, Project scope).
    project.write_pack(
        &[".ragent", "skills"],
        "clash",
        "---\nname: clash\ndescription: from the existing ragent pack\n---\nragent body\n",
    );
    // AgentSkills pack with the same name (`.agents/skills/`, openskills-project
    // scope) must not shadow the existing pack (A7).
    project.write_pack(
        &[".agents", "skills"],
        "clash",
        "---\nname: clash\ndescription: from the AgentSkills pack\n---\nagentskills body\n",
    );
    let mut app = project.app();

    app.execute_slash_command("/skill").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("from the existing ragent pack"),
        "the existing ragent pack must win the clash, got: {text}"
    );
    assert!(
        !text.contains("from the AgentSkills pack"),
        "the AgentSkills pack must not shadow the existing pack, got: {text}"
    );
    assert!(
        text.contains("scope: project"),
        "the winner is the higher-scope ragent pack, got: {text}"
    );
}

#[test]
fn test_skill_command_registered_in_slash_catalog() {
    let triggers: Vec<&str> = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .map(|c| c.trigger)
        .collect();
    assert!(
        triggers.contains(&"skill"),
        "/skill must be registered in SLASH_COMMANDS"
    );
    assert!(
        triggers.contains(&"skills"),
        "/skills must remain registered in SLASH_COMMANDS"
    );
}
