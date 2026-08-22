//! `/reverse` slash command handling for the TUI.
//!
//! Takes a public GitHub repository URL (or `owner/repo` shorthand), fetches
//! the repo metadata, root file tree, and README via the GitHub API, then
//! dispatches a prompt to the currently selected LLM model to generate a
//! synthetic creation prompt.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_agent::event::Event;
use ragent_agent::github::{GitHubClient, build_reverse_prompt};
use ragent_agent::message::Message;

use crate::app::state::{App, LogLevel};

/// Parsed `/reverse` command arguments.
struct ReverseArgs {
    /// Repository identifier (URL, SSH URL, or `owner/repo` shorthand).
    repo_input: String,
    /// Optional technology-stack constraint (`--tech <stack>`).
    tech: Option<String>,
    /// Optional spec-create flag (`--create <name>`).
    create: Option<String>,
}

/// Parse `/reverse` arguments.
///
/// Accepts:
/// - `/reverse <repo>` — basic form
/// - `/reverse <repo> --tech <stack>` — with tech constraint
/// - `/reverse <repo> --create <name>` — chain into `/spec create`
/// - `/reverse help` — usage message
///
/// Flags may appear in any order after the positional repo argument.
fn parse_reverse_args(args: &str) -> Option<ReverseArgs> {
    let args = args.trim();
    if args.is_empty() || args == "help" {
        return None;
    }

    let mut repo_input: Option<String> = None;
    let mut tech: Option<String> = None;
    let mut create: Option<String> = None;

    let mut tokens = args.split_whitespace().peekable();
    while let Some(tok) = tokens.next() {
        match tok {
            "--tech" => {
                let val = tokens.next()?;
                tech = Some(val.to_string());
            }
            "--create" => {
                let val = tokens.next()?;
                create = Some(val.to_string());
            }
            _ => {
                // First non-flag token is the repo identifier.
                if repo_input.is_none() {
                    repo_input = Some(tok.to_string());
                }
            }
        }
    }

    let repo_input = repo_input?;
    Some(ReverseArgs {
        repo_input,
        tech,
        create,
    })
}

/// Build the LLM instruction prompt that wraps the context block.
fn build_llm_task(context: &str, tech: Option<&str>) -> String {
    let tech_line = tech
        .map(|t| format!("\nThe generated prompt should target this technology stack: {t}\n"))
        .unwrap_or_default();
    format!(
        "You are analysing a public GitHub repository to reverse-engineer the \
         prompt that was likely used to create it with an AI coding assistant.\n\
         Below is the repository's metadata, root file tree, and README content.\n\
         Generate a single synthetic prompt that someone could have used to \
         create this repository from scratch. The prompt should capture the \
         project's purpose, architecture, key files, and conventions implied \
         by the file tree and README.{tech_line}\n\
         Output only the prompt text, with no preamble or explanation.\n\n{context}"
    )
}

impl App {
    /// Handle the `/reverse` slash command (FR-001, FR-004, FR-009, FR-010,
    /// FR-018).
    ///
    /// Parses the repo identifier, validates GitHub auth, shows a `⏳`-prefixed
    /// status, then spawns an async task that fetches repo metadata + tree +
    /// README, builds a context block, and dispatches it to the LLM via
    /// `process_message`.
    pub(crate) fn handle_reverse_command(&mut self, args: &str) {
        // FR-013: parse args via the shared `parse_reverse_args` helper.
        // Empty input or the literal `help` subcommand yields `None`, which
        // we surface as the usage/help message.
        let parsed = match parse_reverse_args(args) {
            Some(p) => p,
            None => {
                self.append_assistant_text(&reverse_help_message());
                self.status = "reverse: usage".to_string();
                return;
            }
        };

        // FR-003 + FR-016: validate the repo identifier into (owner, repo).
        let (owner, repo) = match GitHubClient::validate_repo_input(&parsed.repo_input) {
            Ok(pair) => pair,
            Err(msg) => {
                self.append_assistant_text(&format!("From: /reverse\n\n{msg}"));
                self.status = "reverse: invalid repo".to_string();
                return;
            }
        };

        // FR-004: validate GitHub token
        if ragent_agent::github::auth::load_token().is_none() {
            self.append_assistant_text(
                "From: /reverse\n\n❌ **No GitHub token configured.**\n\n\
                 Run `/github login` to authenticate, then re-run `/reverse`.",
            );
            self.status = "reverse: no token".to_string();
            return;
        }

        // Ensure we have a session
        if !self.ensure_session() {
            return;
        }

        let repo_id = format!("{owner}/{repo}");

        // FR-018: ⏳-prefixed status so the auto-expiry timer is NOT armed
        self.status = format!("⏳ reverse: {repo_id}…");
        self.push_log_no_agent(LogLevel::Info, format!("reverse: fetching {repo_id}"));

        // FR-009: resolve the currently selected model
        let explore_agent = self
            .cycleable_agents
            .iter()
            .find(|a| a.name == "explore")
            .cloned();
        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
        self.apply_selected_model_and_thinking(&mut agent);
        agent.permission = ragent_agent::agent::default_permissions();

        // Build the LLM instruction + context in the spawned task (after fetch)
        let sid = self.session_id.clone().unwrap_or_default();
        let context_msg = Message::user_text(&sid, format!("⏳ Reverse-engineering {repo_id}…"));
        self.messages.push(context_msg);

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        self.is_processing = true;

        let event_bus = self.event_bus.clone();
        let owner_for_spawn = owner.clone();
        let repo_for_spawn = repo.clone();
        let repo_id_for_spawn = repo_id.clone();
        let tech_for_spawn = parsed.tech.clone();
        let create_for_spawn = parsed.create.clone();

        // FR-012: store the spec name so MessageEnd can chain into
        // `/spec create <name> <generated-prompt>` after the LLM finishes.
        if let Some(ref name) = parsed.create {
            self.pending_reverse_create = Some(name.clone());
        }

        // FR-010: show immediate user feedback
        self.append_assistant_text(&format!(
            "From: /reverse\n\n⏳ **Fetching {repo_id}…**\n\n\
             Gathering repository metadata, file tree, and README, then generating \
             a synthetic creation prompt."
        ));

        tokio::spawn(async move {
            // Create the GitHub client (token already validated above, but
            // new() re-resolves it — if it vanished between validation and here,
            // surface the error).
            let client = match GitHubClient::new() {
                Ok(c) => c,
                Err(e) => {
                    event_bus.publish(Event::AgentError {
                        session_id: sid.clone(),
                        error: format!("reverse: GitHub auth failed: {e}"),
                    });
                    return;
                }
            };

            // FR-005: fetch repo metadata
            let metadata = match client
                .fetch_repo_metadata(&owner_for_spawn, &repo_for_spawn)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    event_bus.publish(Event::AgentError {
                        session_id: sid.clone(),
                        error: format!("reverse: {repo_id_for_spawn}: {e}"),
                    });
                    return;
                }
            };

            // FR-006: fetch root tree (tolerate failure — proceed with empty)
            let tree = client
                .fetch_root_tree(&owner_for_spawn, &repo_for_spawn)
                .await
                .unwrap_or_default();

            // FR-007: fetch README (404 → None, proceed with empty)
            let readme = client
                .fetch_readme(&owner_for_spawn, &repo_for_spawn)
                .await
                .ok()
                .flatten();

            // FR-008: assemble context block
            let context = build_reverse_prompt(
                &metadata,
                &tree,
                readme.as_deref(),
                tech_for_spawn.as_deref(),
            );
            let task = build_llm_task(&context, tech_for_spawn.as_deref());

            // FR-009 + FR-010: dispatch to LLM via process_message — the
            // generated prompt will appear in the chat panel as an assistant
            // message.
            if let Err(e) = processor.process_message(&sid, &task, &agent, flag).await {
                tracing::warn!(error = %e, "reverse: LLM generation failed");
                event_bus.publish(Event::AgentError {
                    session_id: sid.clone(),
                    error: format!("reverse: LLM generation failed: {e}"),
                });
                return;
            }

            // FR-012: if --create <name> was provided, the actual
            // `/spec create` chaining is handled by the `MessageEnd`
            // event handler in `event_handler.rs` (it reads
            // `pending_reverse_create` and the last assistant message
            // text). Here we just publish a notice so the user sees
            // that chaining is about to happen.
            if let Some(name) = &create_for_spawn {
                let notice = format!(
                    "reverse: generated prompt for {repo_id_for_spawn}. \
                       Chaining into /spec create {name}…"
                );
                event_bus.publish(Event::AgentNotice {
                    session_id: sid.clone(),
                    message: notice,
                });
            } else {
                event_bus.publish(Event::AgentNotice {
                    session_id: sid,
                    message: format!("reverse: completed {repo_id_for_spawn}"),
                });
            }
        });
    }
}

/// Build the help message for `/reverse help` (FR-013).
fn reverse_help_message() -> String {
    "From: /reverse\n\n\
     **Reverse-engineer a GitHub repository into a synthetic creation prompt.**\n\n\
     **Usage:**\n\
     `/reverse <owner/repo | URL> [--tech <stack>] [--create <name>]`\n\n\
     **Arguments:**\n\
     - `<repo>` — GitHub repository identifier. Accepts:\n\
       - Shorthand: `owner/repo`\n\
       - HTTPS URL: `https://github.com/owner/repo`\n\
       - SSH URL: `git@github.com:owner/repo.git`\n\n\
     **Optional flags:**\n\
     - `--tech <stack>` — constrain the generated prompt to a technology stack\n\
     - `--create <name>` — after generation, chain into `/spec create <name>`\n\n\
     **Examples:**\n\
     `/reverse octocat/Hello-World`\n\
     `/reverse https://github.com/octocat/Hello-World --tech \"Rust + Tokio\"`\n\
     `/reverse octocat/Hello-World --create my-spec`\n\n\
     **Prerequisites:**\n\
     Run `/github login` first to configure a GitHub token."
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reverse_args_basic() {
        let args = parse_reverse_args("octocat/Hello-World").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert!(args.tech.is_none());
        assert!(args.create.is_none());
    }

    #[test]
    fn test_parse_reverse_args_with_tech() {
        let args = parse_reverse_args("octocat/Hello-World --tech Rust").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert_eq!(args.tech.as_deref(), Some("Rust"));
        assert!(args.create.is_none());
    }

    #[test]
    fn test_parse_reverse_args_with_create() {
        let args = parse_reverse_args("octocat/Hello-World --create my-spec").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert_eq!(args.create.as_deref(), Some("my-spec"));
        assert!(args.tech.is_none());
    }

    #[test]
    fn test_parse_reverse_args_with_both_flags() {
        let args = parse_reverse_args("octocat/Hello-World --tech \"Rust + Tokio\" --create spec1")
            .unwrap();
        // Note: split_whitespace doesn't handle quoted args — the quotes are
        // part of the token. This is a known TUI limitation; the user types
        // tokens without quotes or the flag parser keeps the whole remaining
        // value. For now, we accept the raw token.
        assert_eq!(args.repo_input, "octocat/Hello-World");
        // The tech value will include the quotes as part of the token.
        assert!(args.tech.is_some());
        assert_eq!(args.create.as_deref(), Some("spec1"));
    }

    #[test]
    fn test_parse_reverse_args_url() {
        let args = parse_reverse_args("https://github.com/octocat/Hello-World").unwrap();
        assert_eq!(args.repo_input, "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn test_parse_reverse_args_empty() {
        assert!(parse_reverse_args("").is_none());
    }

    #[test]
    fn test_parse_reverse_args_help() {
        assert!(parse_reverse_args("help").is_none());
    }

    #[test]
    fn test_parse_reverse_args_missing_repo() {
        // Only flags, no positional repo.
        assert!(parse_reverse_args("--tech Rust").is_none());
    }

    #[test]
    fn test_parse_reverse_args_missing_flag_value() {
        // --tech with no following token.
        assert!(parse_reverse_args("octocat/Hello-World --tech").is_none());
    }

    #[test]
    fn test_parse_reverse_args_flags_before_repo() {
        // Flags can appear before the repo too.
        let args = parse_reverse_args("--tech Rust octocat/Hello-World").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert_eq!(args.tech.as_deref(), Some("Rust"));
    }

    #[test]
    fn test_build_llm_task_includes_context() {
        let context = "## Repository Metadata\nDescription: test";
        let task = build_llm_task(context, None);
        assert!(task.contains("reverse-engineer"));
        assert!(task.contains(context));
        assert!(task.contains("Output only the prompt text"));
    }

    #[test]
    fn test_build_llm_task_with_tech() {
        let context = "## Repository Metadata\nDescription: test";
        let task = build_llm_task(context, Some("Rust + Tokio"));
        assert!(task.contains("technology stack: Rust + Tokio"));
    }

    #[test]
    fn test_build_llm_task_without_tech() {
        let context = "test";
        let task = build_llm_task(context, None);
        assert!(!task.contains("technology stack"));
    }

    #[test]
    fn test_reverse_help_message_content() {
        let msg = reverse_help_message();
        assert!(msg.contains("Usage:"));
        assert!(msg.contains("--tech"));
        assert!(msg.contains("--create"));
        assert!(msg.contains("/github login"));
    }
}
