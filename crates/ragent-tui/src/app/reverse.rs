//! `/reverse` slash command handling for the TUI.
//!
//! Takes a public repository URL (or `owner/repo` shorthand), fetches
//! the repo metadata, root file tree, and README via the GitHub or GitLab API,
//! then dispatches a prompt to the currently selected LLM model to generate a
//! synthetic creation prompt.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_agent::event::Event;
use ragent_agent::github::{GitHubClient, build_reverse_prompt};
use ragent_agent::gitlab::GitLabClient;
use ragent_agent::message::Message;
use ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo};

use crate::app::state::{App, LogLevel};

/// Parsed `/reverse` command arguments.
struct ReverseArgs {
    /// Repository identifier (URL, SSH URL, or `owner/repo` shorthand).
    repo_input: String,
    /// Optional technology-stack constraint (`--tech <stack>`).
    tech: Option<String>,
    /// Optional spec-create flag (`--create <name>`).
    create: Option<String>,
    /// Optional tree-fetch depth (`--depth <N>`), raw string. Validated to
    /// `u32` in the range 1–10 by `handle_reverse_command` (FR-025, FR-029).
    depth: Option<String>,
}

/// Parse `/reverse` arguments.
///
/// Accepts:
/// - `/reverse <repo>` — basic form
/// - `/reverse <repo> --tech <stack>` — with tech constraint
/// - `/reverse <repo> --create <name>` — chain into `/spec create`
/// - `/reverse <repo> --depth <N>` — tree-fetch depth (FR-025)
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
    let mut depth: Option<String> = None;

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
            "--depth" => {
                let val = tokens.next()?;
                depth = Some(val.to_string());
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
        depth,
    })
}

/// Build a human-readable provider label and a short repo identifier from
/// the resolved [`VcsProvider`] (FR-016, FR-017).
///
/// Returns `(repo_id, provider_label)`:
/// - GitHub → `("owner/repo", "GitHub")`
/// - GitLab with explicit host → `("namespace/project", "GitLab (host)")`
/// - GitLab with configured instance → `("namespace/project", "GitLab")`
fn provider_label_and_id(provider: &VcsProvider) -> (String, String) {
    match provider {
        VcsProvider::GitHub { owner, repo } => (format!("{owner}/{repo}"), "GitHub".to_string()),
        VcsProvider::GitLab { host, project_path } => {
            let label = match host {
                Some(url) => {
                    let host_part = url
                        .strip_prefix("https://")
                        .or_else(|| url.strip_prefix("http://"))
                        .unwrap_or(url);
                    format!("GitLab ({host_part})")
                }
                None => "GitLab".to_string(),
            };
            (project_path.clone(), label)
        }
    }
}

/// Resolve the GitLab Personal Access Token from the `GITLAB_TOKEN`
/// environment variable (FR-007). The App method
/// [`App::resolve_gitlab_token_for_reverse`] layers ragent.json and the
/// encrypted database on top of this.
fn resolve_gitlab_token() -> Option<String> {
    std::env::var("GITLAB_TOKEN").ok().filter(|t| !t.is_empty())
}

/// Resolve the GitLab instance base URL (FR-002).
///
/// Priority: explicit `host` from the identifier → `GITLAB_URL` env var →
/// default `https://gitlab.com`.
fn resolve_gitlab_host(host: Option<&str>) -> String {
    if let Some(h) = host {
        return h.to_string();
    }
    std::env::var("GITLAB_URL")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "https://gitlab.com".to_string())
}

/// Validate the `--depth <N>` flag value (FR-025, FR-029).
///
/// Returns the validated depth on success:
/// - `None` → `1` (the default, FR-025)
/// - `Some(val)` where `val` parses as a `u32` in `1..=10` → that value
///
/// Returns an error message on failure (FR-029):
/// - `Some(val)` that doesn't parse as `u32` (e.g. `"abc"`, `"-1"`)
/// - `Some(val)` that parses but is outside `1..=10` (e.g. `"0"`, `"11"`)
fn validate_depth(depth_raw: Option<&str>) -> Result<u32, String> {
    match depth_raw {
        None => Ok(1),
        Some(val) => match val.parse::<u32>() {
            Ok(n) if (1..=10).contains(&n) => Ok(n),
            Ok(n) => Err(format!(
                "Invalid --depth value: {n}. The depth must be an integer between 1 and 10."
            )),
            Err(_) => Err(format!(
                "Invalid --depth value: {val}. The depth must be an integer between 1 and 10."
            )),
        },
    }
}

/// Build the LLM instruction prompt that wraps the context block.
fn build_llm_task(context: &str, tech: Option<&str>) -> String {
    let tech_line = tech
        .map(|t| format!("\nThe generated prompt should target this technology stack: {t}\n"))
        .unwrap_or_default();
    format!(
        "You are analysing a public repository to reverse-engineer the \
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
    /// Resolve the GitLab Personal Access Token for the `/reverse` command
    /// (FR-007).
    ///
    /// Priority: `GITLAB_TOKEN` env → `ragent.json` → encrypted database via
    /// `/gitlab setup`. Uses the App's `storage` handle to access the
    /// encrypted database.
    fn resolve_gitlab_token_for_reverse(&self) -> Option<String> {
        // 1. Environment variable.
        if let Some(token) = resolve_gitlab_token() {
            return Some(token);
        }

        // 2. ragent.json config file.
        if let Ok(cfg) = ragent_config::Config::load()
            && let Some(ref t) = cfg.gitlab.token
            && !t.is_empty()
        {
            return Some(t.clone());
        }

        // 3. Encrypted database via `/gitlab setup`.
        ragent_agent::gitlab::auth::load_token(self.storage.as_ref())
    }

    /// Handle the `/reverse` slash command (FR-001, FR-004, FR-009, FR-010,
    /// FR-018).
    ///
    /// Parses the repo identifier, validates auth, shows a `[wait]`-prefixed
    /// status, then spawns an async task that fetches repo metadata + tree +
    /// README, builds a context block, and dispatches it to the LLM via
    /// `process_message`.
    pub(crate) fn handle_reverse_command(&mut self, args: &str) {
        // FR-013: parse args via the shared `parse_reverse_args` helper.
        let parsed = match parse_reverse_args(args) {
            Some(p) => p,
            None => {
                self.append_assistant_text(&reverse_help_message());
                self.status = "reverse: usage".to_string();
                return;
            }
        };

        // FR-024: delegate repo validation to `parse_reverse_repo` so that
        // provider dispatch happens in one place (FR-014).
        let provider = match parse_reverse_repo(&parsed.repo_input) {
            Ok(p) => p,
            Err(msg) => {
                self.append_assistant_text(&format!("From: /reverse\n\n{msg}"));
                self.status = "reverse: invalid repo".to_string();
                return;
            }
        };

        // FR-014: dispatch to the GitHub or GitLab fetch path based on the
        // resolved VcsProvider.
        let (repo_id, provider_label) = provider_label_and_id(&provider);

        // FR-025 / FR-029: validate the --depth flag. Default is 1; valid
        // range is 1–10. An invalid value surfaces a human-readable error
        // without making any API calls.
        let depth: u32 = match validate_depth(parsed.depth.as_deref()) {
            Ok(n) => n,
            Err(msg) => {
                self.append_assistant_text(&format!("From: /reverse\n\n[err] **{msg}**"));
                self.status = "reverse: invalid depth".to_string();
                return;
            }
        };

        match &provider {
            VcsProvider::GitHub { .. } => {
                if ragent_agent::github::auth::load_token().is_none() {
                    self.append_assistant_text(
                        "From: /reverse\n\n[err] **No GitHub token configured.**\n\n\
                         Run `/github login` to authenticate, then re-run `/reverse`.",
                    );
                    self.status = "reverse: no token".to_string();
                    return;
                }
            }
            VcsProvider::GitLab { host, .. } => {
                // FR-007: validate GitLab token.
                let token = self.resolve_gitlab_token_for_reverse();
                if token.is_none() {
                    self.append_assistant_text(
                        "From: /reverse\n\n[err] **No GitLab token configured.**\n\n\
                         Run `/gitlab setup` to configure your GitLab instance and \
                         Personal Access Token, or set the `GITLAB_TOKEN` environment \
                         variable.",
                    );
                    self.status = "reverse: no gitlab token".to_string();
                    return;
                }
                let _ = resolve_gitlab_host(host.as_deref());
            }
        }

        // Ensure we have a session
        if !self.ensure_session() {
            return;
        }

        // FR-017: status messages and log entries include the provider label.
        self.status = format!("[wait] reverse: {provider_label}: {repo_id}…");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("reverse: fetching {repo_id} via {provider_label}"),
        );

        // FR-009: resolve the currently selected model
        let explore_agent = self
            .cycleable_agents
            .iter()
            .find(|a| a.name == "explore")
            .cloned();
        let mut agent = explore_agent.unwrap_or_else(|| self.agent_info.clone());
        self.apply_selected_model_and_thinking(&mut agent);
        agent.permission = ragent_agent::agent::default_permissions();

        let sid = self.session_id.clone().unwrap_or_default();
        let context_msg = Message::user_text(
            &sid,
            format!("[wait] Reverse-engineering {provider_label}: {repo_id}…"),
        );
        self.messages.push(context_msg);

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        self.is_processing = true;

        let event_bus = self.event_bus.clone();
        let provider_for_spawn = provider.clone();
        let provider_label_for_spawn = provider_label.clone();
        let tech_for_spawn = parsed.tech.clone();
        let create_for_spawn = parsed.create.clone();
        let repo_id_for_spawn = repo_id.clone();
        let depth_for_spawn = depth;
        let gitlab_token = self.resolve_gitlab_token_for_reverse();
        let gitlab_host = match &provider {
            VcsProvider::GitLab { host, .. } => Some(resolve_gitlab_host(host.as_deref())),
            _ => None,
        };

        // FR-012: store the spec name so MessageEnd can chain into
        // `/spec create <name> <generated-prompt>` after the LLM finishes.
        if let Some(ref name) = parsed.create {
            self.pending_reverse_create = Some(name.clone());
        }

        // FR-017: user feedback includes the provider label.
        self.append_assistant_text(&format!(
            "From: /reverse\n\n[wait] **Fetching {repo_id} via {provider_label}…**\n\n\
             Gathering repository metadata, file tree, and README, then generating \
             a synthetic creation prompt."
        ));

        let sid_clone = sid.clone();

        tokio::spawn(async move {
            // FR-014: dispatch fetch based on the VcsProvider.
            let (metadata, tree, readme) = match &provider_for_spawn {
                VcsProvider::GitHub { owner, repo } => {
                    let client = match GitHubClient::new() {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!(error = %e, "reverse: GitHub client failed");
                            event_bus.publish(Event::AgentError {
                                session_id: sid_clone.clone(),
                                error: format!("reverse: GitHub client failed: {e}"),
                            });
                            return;
                        }
                    };

                    let metadata = client
                        .fetch_repo_metadata(owner, repo)
                        .await
                        .unwrap_or_default();

                    // FR-025/FR-026/FR-027: use recursive fetch when depth > 1.
                    let tree = if depth_for_spawn > 1 {
                        client
                            .fetch_tree_recursive(owner, repo, depth_for_spawn)
                            .await
                            .unwrap_or_default()
                    } else {
                        client
                            .fetch_root_tree(owner, repo)
                            .await
                            .unwrap_or_default()
                    };

                    let readme = client.fetch_readme(owner, repo).await.ok().flatten();

                    (metadata, tree, readme)
                }
                VcsProvider::GitLab { host, project_path } => {
                    let token = match &gitlab_token {
                        Some(t) => t.clone(),
                        None => {
                            event_bus.publish(Event::AgentError {
                                session_id: sid_clone.clone(),
                                error: "reverse: No GitLab token configured. Run /gitlab setup."
                                    .to_string(),
                            });
                            return;
                        }
                    };
                    let base_url = gitlab_host
                        .clone()
                        .unwrap_or_else(|| resolve_gitlab_host(host.as_deref()));
                    let client = GitLabClient::with_credentials(base_url, token);

                    // FR-020 / FR-021: surface specific errors for 401 and 404.
                    let metadata = match client.fetch_project_metadata(project_path).await {
                        Ok(m) => m,
                        Err(e) => {
                            let msg = if e.to_string().contains("401") {
                                format!(
                                    "reverse: GitLab authentication failed for \
                                         {repo_id_for_spawn}. Run /gitlab setup to update \
                                         your Personal Access Token."
                                )
                            } else if e.to_string().contains("404") {
                                format!(
                                    "reverse: GitLab project '{repo_id_for_spawn}' was not \
                                         found on the target instance. Verify the namespace and \
                                         project path."
                                )
                            } else {
                                format!("reverse: {repo_id_for_spawn}: {e}")
                            };
                            event_bus.publish(Event::AgentError {
                                session_id: sid_clone.clone(),
                                error: msg,
                            });
                            return;
                        }
                    };

                    // FR-025/FR-026/FR-027: use recursive fetch when depth > 1.
                    let tree = if depth_for_spawn > 1 {
                        client
                            .fetch_repository_tree_recursive(project_path, depth_for_spawn)
                            .await
                            .unwrap_or_default()
                    } else {
                        client
                            .fetch_repository_tree(project_path)
                            .await
                            .unwrap_or_default()
                    };
                    let readme = client.fetch_readme(project_path).await.ok().flatten();

                    (metadata, tree, readme)
                }
            };

            // FR-008/FR-016: assemble context block with the provider label.
            let context = build_reverse_prompt(
                &metadata,
                &tree,
                readme.as_deref(),
                tech_for_spawn.as_deref(),
                Some(&provider_label_for_spawn),
            );
            let task = build_llm_task(&context, tech_for_spawn.as_deref());

            if let Err(e) = processor.process_message(&sid, &task, &agent, flag).await {
                tracing::warn!(error = %e, "reverse: LLM generation failed");
                event_bus.publish(Event::AgentError {
                    session_id: sid.clone(),
                    error: format!("reverse: LLM generation failed: {e}"),
                });
                return;
            }

            // FR-017: notices include the provider label.
            if let Some(name) = &create_for_spawn {
                let notice = format!(
                    "reverse: generated prompt for {repo_id_for_spawn} via \
                     {provider_label_for_spawn}. Chaining into /spec create {name}…"
                );
                event_bus.publish(Event::AgentNotice {
                    session_id: sid.clone(),
                    message: notice,
                });
            } else {
                event_bus.publish(Event::AgentNotice {
                    session_id: sid,
                    message: format!(
                        "reverse: completed {repo_id_for_spawn} via {provider_label_for_spawn}"
                    ),
                });
            }
        });
    }
}

/// Build the help message for `/reverse help` (FR-013).
fn reverse_help_message() -> String {
    "From: /reverse\n\n\
     **Reverse-engineer a repository into a synthetic creation prompt.**\n\n\
     **Usage:**\n\
     `/reverse <repo> [--tech <stack>] [--create <name>] [--depth <N>]`\n\n\
     **Arguments:**\n\
     - `<repo>` — repository identifier. Accepts:\n\
       - Shorthand: `owner/repo` (defaults to GitHub)\n\
       - `github:owner/repo` or `github:<github-url>`\n\
       - `gitlab:namespace/project` (uses configured GitLab instance)\n\
       - `gitlab:host/namespace/project` (self-hosted GitLab)\n\
       - HTTPS URL: `https://github.com/owner/repo` or `https://gitlab.com/ns/proj`\n\
       - SSH URL: `git@github.com:owner/repo.git` or `git@gitlab.com:ns/proj.git`\n\n\
     **Optional flags:**\n\
     - `--tech <stack>` — constrain the generated prompt to a technology stack\n\
     - `--create <name>` — after generation, chain into `/spec create <name>`\n\
     - `--depth <N>` — directory levels to fetch (1–10, default 1)\n\n\
     **Prerequisites:**\n\
     - GitHub: run `/github login` first to configure a GitHub token.\n\
     - GitLab: run `/gitlab setup` first (or set GITLAB_TOKEN + GITLAB_URL env vars)."
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
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert!(args.tech.is_some());
        assert_eq!(args.create.as_deref(), Some("spec1"));
    }

    #[test]
    fn test_parse_reverse_args_with_depth() {
        let args = parse_reverse_args("octocat/Hello-World --depth 3").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert_eq!(args.depth.as_deref(), Some("3"));
    }

    #[test]
    fn test_parse_reverse_args_with_all_flags() {
        let args =
            parse_reverse_args("octocat/Hello-World --tech Rust --create spec1 --depth 5").unwrap();
        assert_eq!(args.repo_input, "octocat/Hello-World");
        assert_eq!(args.tech.as_deref(), Some("Rust"));
        assert_eq!(args.create.as_deref(), Some("spec1"));
        assert_eq!(args.depth.as_deref(), Some("5"));
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
        assert!(parse_reverse_args("--tech Rust").is_none());
    }

    #[test]
    fn test_parse_reverse_args_missing_flag_value() {
        assert!(parse_reverse_args("octocat/Hello-World --tech").is_none());
    }

    #[test]
    fn test_parse_reverse_args_missing_depth_value() {
        assert!(parse_reverse_args("octocat/Hello-World --depth").is_none());
    }

    #[test]
    fn test_parse_reverse_args_flags_before_repo() {
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
    fn test_provider_label_and_id_github() {
        let provider = VcsProvider::GitHub {
            owner: "octocat".to_string(),
            repo: "Hello-World".to_string(),
        };
        let (repo_id, label) = provider_label_and_id(&provider);
        assert_eq!(repo_id, "octocat/Hello-World");
        assert_eq!(label, "GitHub");
    }

    #[test]
    fn test_provider_label_and_id_gitlab_default() {
        let provider = VcsProvider::GitLab {
            host: None,
            project_path: "group/project".to_string(),
        };
        let (repo_id, label) = provider_label_and_id(&provider);
        assert_eq!(repo_id, "group/project");
        assert_eq!(label, "GitLab");
    }

    #[test]
    fn test_provider_label_and_id_gitlab_self_hosted() {
        let provider = VcsProvider::GitLab {
            host: Some("https://gitlab.example.com".to_string()),
            project_path: "group/project".to_string(),
        };
        let (repo_id, label) = provider_label_and_id(&provider);
        assert_eq!(repo_id, "group/project");
        assert_eq!(label, "GitLab (gitlab.example.com)");
    }

    #[test]
    fn test_reverse_help_message_content() {
        let msg = reverse_help_message();
        assert!(msg.contains("Usage:"));
        assert!(msg.contains("--tech"));
        assert!(msg.contains("--create"));
        assert!(msg.contains("--depth"));
        assert!(msg.contains("/github login"));
        assert!(msg.contains("/gitlab setup"));
        assert!(msg.contains("gitlab:"));
    }

    #[test]
    fn test_reverse_help_message_lists_all_formats() {
        let msg = reverse_help_message();
        assert!(msg.contains("owner/repo"));
        assert!(msg.contains("github:"));
        assert!(msg.contains("gitlab:"));
        assert!(msg.contains("https://github.com"));
        assert!(msg.contains("https://gitlab.com"));
        assert!(msg.contains("git@github.com"));
        assert!(msg.contains("git@gitlab.com"));
    }
}
