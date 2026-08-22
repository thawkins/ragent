//! Slash command handling for the spec management system.
//!
//! Provides parsing and helpers for `/spec` subcommands so the TUI can delegate
//! spec logic to this crate rather than embedding it inline.

/// Parsed `/spec` subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecCommand {
    /// Show the help reference.
    Help,
    /// Create a new spec directory with SPEC.md and PLAN.md.
    Create {
        /// URL-safe identifier used as the directory name.
        specname: String,
        /// Free-text feature description.
        feature: String,
        /// Optional research artifact name linked via `--from-research` (FR-010).
        from_research: Option<String>,
    },
    /// Validate a spec or all specs for EARS compliance.
    Validate {
        /// Optional spec ID; if omitted, validates all specs.
        spec_id: Option<String>,
    },
    /// List specs with optional filtering.
    List {
        /// Raw filter arguments (e.g. "--status draft --prefix test").
        args: String,
    },
    /// Search specs by full-text query.
    Search {
        /// Search query string.
        query: String,
    },
    /// Show or transition a spec's status.
    Status {
        /// Spec identifier.
        spec_id: String,
        /// Optional new status to transition to.
        new_status: Option<String>,
    },
    /// Manage tasks within a spec's plan.
    Task {
        /// Spec identifier.
        spec_id: String,
        /// Optional task ID to show or update.
        task_id: Option<String>,
        /// Optional new status to set on the task.
        new_status: Option<String>,
    },
    /// Activate a spec for context injection into agent prompts.
    Activate {
        /// Spec identifier.
        spec_id: String,
    },
    /// Deactivate the currently active spec.
    Deactivate,
    /// Show requirement coverage for a spec.
    Coverage {
        /// Spec identifier.
        spec_id: String,
    },
    /// Implement a spec by executing its PLAN.md tasks in dependency order.
    Impl {
        /// Spec identifier.
        spec_id: String,
        /// Optional task ID to execute (with its transitive dependencies).
        task_id: Option<String>,
        /// If true, display execution plan without actually running tasks.
        dry_run: bool,
    },
    /// Incrementally add requirements to an existing spec and update its plan.
    Add {
        /// Spec identifier.
        spec_id: String,
        /// Free-text feature description for the new requirements.
        feature: String,
    },
    /// Regenerate PLAN.md and TESTPLAN.md from an edited SPEC.md.
    Update {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
    },
    /// Delete a spec directory from the workspace.
    Delete {
        /// Spec identifier.
        spec_id: String,
        /// If true, skip the confirmation prompt.
        yes: bool,
    },
    /// Perform a Jobs-To-Be-Done analysis of an existing spec's SPEC.md,
    /// writing the result to `specs/<specname>/JTBD.md`.
    Jtbd {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
        /// If true, overwrite an existing `JTBD.md`.
        force: bool,
        /// Optional override agent name; if `None`, the default explore agent
        /// is used (falling back to the currently selected agent).
        agent: Option<String>,
    },
    /// Create a SPEC.md only (no PLAN.md) from a feature prompt.
    ///
    /// This separates the specification stage from the planning stage,
    /// matching SDD's `/speckit.specify` workflow.
    Specify {
        /// URL-safe identifier used as the directory name.
        specname: String,
        /// Free-text feature description.
        feature: String,
        /// Optional research artifact name linked via `--from-research` (FR-010).
        from_research: Option<String>,
    },
    /// Generate or regenerate a `PLAN.md` from an existing `SPEC.md` using
    /// the provided technology context as guidance.
    ///
    /// This separates plan generation from spec creation, matching SDD's
    /// `/speckit.plan` command. Coexists with `/spec update` which
    /// regenerates the plan from an edited `SPEC.md` without a tech-context
    /// argument.
    Plan {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
        /// Free-text technology context that informs technology choices and
        /// rationale documented in the plan.
        tech_context: String,
    },
    /// Generate a `TASKS.md` file containing an ordered task list derived
    /// from the existing `PLAN.md`.
    ///
    /// This creates a standalone task-list artifact distinct from the
    /// PLAN.md task table, matching SDD's `/speckit.tasks` command.
    Tasks {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
    },
    /// Append a production feedback note to `specs/<spec-id>/FEEDBACK.md`
    /// (FR-017).
    ///
    /// The note is advisory — it is surfaced during `/spec plan` regeneration
    /// but does not block validation or status transitions.
    Feedback {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
        /// Free-text feedback note (production metric, incident, user report).
        note: String,
    },
    /// Unknown subcommand (preserves the raw name for error messages).
    Unknown(String),
}

/// Extract `specname`, `feature`, and optional `--from-research` name from
/// the raw argument tail of `create` or `specify` (FR-010).
///
/// The `--from-research <name>` flag may appear at the end of the feature
/// text. If present, it is stripped from the feature and returned as the
/// third element. If absent, `None` is returned.
fn parse_feature_with_research(rest: &str) -> (&str, &str, Option<&str>) {
    let (specname, after_specname) = rest
        .split_once(char::is_whitespace)
        .map_or((rest, ""), |(s, r)| (s.trim(), r.trim()));
    let (feature, from_research) = extract_from_research(after_specname);
    (specname, feature, from_research)
}

/// Split `--from-research <name>` from the end of a feature string.
///
/// Returns `(feature_without_flag, Some(research_name))` when the flag is
/// present, or `(feature, None)` when it is not.
fn extract_from_research(feature: &str) -> (&str, Option<&str>) {
    // Look for the flag as a standalone token.
    if let Some(pos) = feature.find("--from-research") {
        let before = feature[..pos].trim_end();
        let after = feature[pos + "--from-research".len()..].trim();
        if after.is_empty() {
            // Flag present but no name — treat as not provided.
            return (feature, None);
        }
        return (before, Some(after));
    }
    (feature, None)
}

impl SpecCommand {
    /// Parse a `/spec` argument string into a command.
    ///
    /// The first word is treated as the subcommand; everything after the first
    /// whitespace is the subcommand-specific payload.
    pub fn parse(args: &str) -> Self {
        let (sub, rest) = args
            .split_once(char::is_whitespace)
            .map_or((args.trim(), ""), |(s, r)| (s.trim(), r.trim()));

        match sub {
            "help" | "" => Self::Help,
            "create" => {
                let (specname, feature, from_research) = parse_feature_with_research(rest);
                if specname.is_empty() || feature.is_empty() {
                    // Caller should treat this as a usage error.
                    Self::Unknown("create".to_string())
                } else {
                    Self::Create {
                        specname: specname.to_string(),
                        feature: feature.to_string(),
                        from_research: from_research.map(|s| s.to_string()),
                    }
                }
            }
            "validate" => {
                let spec_id = rest.trim();
                if spec_id.is_empty() {
                    Self::Validate { spec_id: None }
                } else {
                    Self::Validate {
                        spec_id: Some(spec_id.to_string()),
                    }
                }
            }
            "list" => Self::List {
                args: rest.trim().to_string(),
            },
            "search" => Self::Search {
                query: rest.trim().to_string(),
            },
            "status" => {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    Self::Unknown("status".to_string())
                } else {
                    let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
                    Self::Status {
                        spec_id: parts[0].to_string(),
                        new_status: parts.get(1).map(|s| s.trim().to_string()),
                    }
                }
            }
            "task" => {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    Self::Unknown("task".to_string())
                } else {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let spec_id = parts[0].to_string();
                    let task_id = parts.get(1).map(|s| s.trim().to_string());
                    let new_status = parts.get(2).map(|s| s.trim().to_string());
                    Self::Task {
                        spec_id,
                        task_id,
                        new_status,
                    }
                }
            }
            "activate" => {
                let spec_id = rest.trim();
                if spec_id.is_empty() {
                    Self::Unknown("activate".to_string())
                } else {
                    Self::Activate {
                        spec_id: spec_id.to_string(),
                    }
                }
            }
            "deactivate" => Self::Deactivate,
            "coverage" => {
                let spec_id = rest.trim();
                if spec_id.is_empty() {
                    Self::Unknown("coverage".to_string())
                } else {
                    Self::Coverage {
                        spec_id: spec_id.to_string(),
                    }
                }
            }
            "impl" | "implement" => {
                // Parse: /spec impl <specname> [--task <ID>] [--dry-run]
                // Alias: /spec implement <specname> [--task <ID>] [--dry-run]
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    Self::Unknown("impl".to_string())
                } else {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let spec_id = parts[0].to_string();
                    let mut task_id = None;
                    let mut dry_run = false;
                    let mut i = 1;
                    while i < parts.len() {
                        match parts[i] {
                            "--task" => {
                                i += 1;
                                if let Some(tid) = parts.get(i) {
                                    task_id = Some(tid.to_string());
                                }
                            }
                            "--dry-run" => {
                                dry_run = true;
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    Self::Impl {
                        spec_id,
                        task_id,
                        dry_run,
                    }
                }
            }
            "add" => {
                let (spec_id, feature) = rest
                    .split_once(char::is_whitespace)
                    .map_or(("", rest), |(s, r)| (s.trim(), r.trim()));
                if spec_id.is_empty() || feature.is_empty() {
                    Self::Unknown("add".to_string())
                } else {
                    Self::Add {
                        spec_id: spec_id.to_string(),
                        feature: feature.to_string(),
                    }
                }
            }
            "delete" => {
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    Self::Unknown("delete".to_string())
                } else {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let spec_id = parts[0].to_string();
                    let yes = parts.contains(&"--yes");
                    Self::Delete { spec_id, yes }
                }
            }
            "jtbd" => {
                // Parse: /spec jtbd <specname> [--force] [--agent <name>]
                let trimmed = rest.trim();
                if trimmed.is_empty() {
                    Self::Unknown("jtbd".to_string())
                } else {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    let spec_id = parts[0].to_string();
                    let mut force = false;
                    let mut agent: Option<String> = None;
                    let mut i = 1;
                    while i < parts.len() {
                        match parts[i] {
                            "--force" => {
                                force = true;
                            }
                            "--agent" => {
                                i += 1;
                                if let Some(name) = parts.get(i) {
                                    agent = Some(name.to_string());
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    Self::Jtbd {
                        spec_id,
                        force,
                        agent,
                    }
                }
            }
            "update" => {
                let spec_id = rest.trim();
                if spec_id.is_empty() {
                    Self::Unknown("update".to_string())
                } else {
                    Self::Update {
                        spec_id: spec_id.to_string(),
                    }
                }
            }
            "specify" => {
                let (specname, feature, from_research) = parse_feature_with_research(rest);
                if specname.is_empty() || feature.is_empty() {
                    Self::Unknown("specify".to_string())
                } else {
                    Self::Specify {
                        specname: specname.to_string(),
                        feature: feature.to_string(),
                        from_research: from_research.map(|s| s.to_string()),
                    }
                }
            }
            "plan" => {
                let (spec_id, tech_context) = rest
                    .split_once(char::is_whitespace)
                    .map_or(("", rest), |(s, r)| (s.trim(), r.trim()));
                if spec_id.is_empty() || tech_context.is_empty() {
                    Self::Unknown("plan".to_string())
                } else {
                    Self::Plan {
                        spec_id: spec_id.to_string(),
                        tech_context: tech_context.to_string(),
                    }
                }
            }
            "tasks" => {
                let spec_id = rest.trim();
                if spec_id.is_empty() {
                    Self::Unknown("tasks".to_string())
                } else {
                    Self::Tasks {
                        spec_id: spec_id.to_string(),
                    }
                }
            }
            "feedback" => {
                let (spec_id, note) = rest
                    .split_once(char::is_whitespace)
                    .map_or(("", rest), |(s, r)| (s.trim(), r.trim()));
                if spec_id.is_empty() || note.is_empty() {
                    Self::Unknown("feedback".to_string())
                } else {
                    Self::Feedback {
                        spec_id: spec_id.to_string(),
                        note: note.to_string(),
                    }
                }
            }
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Returns `true` if this is a usage-error variant.
    #[must_use]
    pub fn is_usage_error(&self) -> bool {
        matches!(
            self,
            Self::Unknown(s) if s == "create"
                || s == "validate"
                || s == "status"
                || s == "task"
                || s == "activate"
                || s == "coverage"
                || s == "impl"
                || s == "add"
                || s == "delete"
                || s == "jtbd"
                || s == "update"
                || s == "specify"
                || s == "plan"
                || s == "tasks"
                || s == "feedback"
        )
    }

    /// Build the static help message shown by `/spec help`.
    #[must_use]
    pub const fn build_help_message() -> &'static str {
        "From: /spec help\n\
                    ## /spec command reference\n\n\
                    | Command | Arguments | Description |\n\
                    |---|---|---|\n\
                    | `/spec help` | none | Show this command reference table. |\n\
                    | `/spec create <specname> <feature description> [--from-research <name>]` | required `specname` + `feature description`, optional `--from-research` | Generate `specs/<specname>/SPEC.md` (EARS spec) and `specs/<specname>/PLAN.md` (implementation plan). `--from-research` pre-populates a `## Related Research` section. |\n\
                    | `/spec add <spec-id> <feature description>` | required `spec-id` + `feature description` | Incrementally add requirements to an existing spec and update its plan. |\n\
                    | `/spec delete <spec-id> [--yes]` | required `spec-id`, optional `--yes` | Delete a spec directory. Use `--yes` to skip the confirmation prompt. |\n\
                    | `/spec validate [specname]` | optional `specname` | Validate EARS compliance. Without argument, validates all specs. |\n\
                    | `/spec list [--status <status>] [--prefix <prefix>]` | optional filters | List all specs with optional filtering by status or ID prefix. |\n\
                    | `/spec search <query>` | required `query` | Full-text search across all specs. |\n\
                    | `/spec status <spec-id> [<new-status>]` | required `spec-id`, optional `new-status` | Show current status or transition to a new status. |\n\
                    | `/spec task <spec-id> [<task-id>] [<new-status>]` | required `spec-id`, optional `task-id` and `new-status` | List tasks, show a task, or update its status. |\n\
                    | `/spec activate <spec-id>` | required `spec-id` | Activate a spec for context injection into agent prompts. |\n\
                    | `/spec deactivate` | none | Deactivate the currently active spec. |\n\
                    | `/spec coverage <spec-id>` | required `spec-id` | Show requirement coverage report. |\n\
                    | `/spec impl <spec-id> [--task <ID>] [--dry-run]` | required `spec-id`, optional flags | Implement a spec by executing its PLAN.md tasks in dependency order. Use `--task` to run a single task, `--dry-run` to preview the plan. Alias: `/spec implement`. |\n\
                    | `/spec jtbd <spec-id> [--force] [--agent <name>]` | required `spec-id`, optional `--force` and `--agent` | Perform a Jobs-To-Be-Done analysis of `specs/<spec-id>/SPEC.md` and write `specs/<spec-id>/JTBD.md`. Use `--force` to overwrite an existing file, `--agent <name>` to dispatch to a specific agent. |\n\
                    | `/spec update <spec-id>` | required `spec-id` | Re-read the existing `SPEC.md` and regenerate `PLAN.md` and `TESTPLAN.md` from its current content. |\n\
                    | `/spec specify <specname> <feature description> [--from-research <name>]` | required `specname` + `feature description`, optional `--from-research` | Generate `specs/<specname>/SPEC.md` only (EARS spec with requirements and `[NEEDS CLARIFICATION]` markers). Does NOT generate PLAN.md — use `/spec plan` after clarification. When `sdd.branch_per_spec` is enabled, also creates a `spec/<specname>` git branch. `--from-research` links the spec to a research artifact via YAML frontmatter. |\n\
                    | `/spec plan <spec-id> <tech-context>` | required `spec-id` + `tech-context` | Generate (or regenerate) `specs/<spec-id>/PLAN.md` from the existing `SPEC.md` using the provided technology context as guidance. Coexists with `/spec update` which regenerates from an edited `SPEC.md` without tech context. |\n\
                    | `/spec tasks <spec-id>` | required `spec-id` | Generate `specs/<spec-id>/TASKS.md` containing an ordered task list derived from the existing `PLAN.md`, plus `specs/<spec-id>/quickstart.md` with key validation scenarios derived from `SPEC.md`. |\n\
                    | `/spec feedback <spec-id> <note>` | required `spec-id` + `note` | Append a production feedback note to `specs/<spec-id>/FEEDBACK.md`. Notes are advisory and surfaced during `/spec plan` regeneration. |\n\
                    Example: `/spec create websocket Add a real-time collaborative editing feature using WebSockets --from-research realtime-collab`"
    }
    /// Build the user-facing status string for a create operation.
    #[must_use]
    pub fn build_create_status(specname: &str) -> String {
        format!(
            "spec: writing specs/{specname}/SPEC.md + specs/{specname}/PLAN.md + \
             specs/{specname}/TESTPLAN.md…"
        )
    }

    /// Build the assistant message shown when a spec generation starts.
    #[must_use]
    pub fn build_create_message(specname: &str, _feature: &str) -> String {
        format!(
            "From: /spec\n📝 **Generating specification and plan…**\n\n\
             Creating spec directory `specs/{specname}` with:\n\
             - `specs/{specname}/SPEC.md` — EARS requirements specification\n\
             - `specs/{specname}/PLAN.md` — implementation plan with tasks\n\
             - `specs/{specname}/TESTPLAN.md` — manual test plan with test cases\n\n\
             This may take a few moments.\n\
             ⚠️ **Tip:** After creation, you can validate with `/spec validate {specname}`."
        )
    }

    /// Build the log entry for a create operation.
    #[must_use]
    pub fn build_create_log(specname: &str, feature: &str) -> String {
        format!(
            "Creating spec '{specname}' for feature: {feature} → specs/{specname}/SPEC.md, \
             specs/{specname}/PLAN.md, specs/{specname}/TESTPLAN.md"
        )
    }

    /// Build the prompt sent to the explore agent for spec generation.
    ///
    /// When `from_research` is `Some(name)`, the prompt instructs the agent
    /// to include a `research:` field in the SPEC.md YAML frontmatter linking
    /// to the named research artifact (FR-010) and a `## Related Research`
    /// section in the SPEC.md body (T-020).
    #[must_use]
    pub fn build_create_prompt(
        specname: &str,
        feature: &str,
        from_research: Option<&str>,
    ) -> String {
        let research_frontmatter = Self::build_research_frontmatter_instruction(from_research);
        let research_section = Self::build_research_section_instruction(from_research);
        format!(
            r"You are an expert specification writer. Create a specification and implementation plan for the following feature.
          
                         **Feature:** {feature}
          
                         **Spec ID:** {specname}
          
                         Write the following files:
          
                         1. `specs/{specname}/SPEC.md` — A requirements specification using EARS notation:
                            - Use at least one of each EARS template: ubiquitous, event-driven, state-driven, optional, unwanted
                            - Number requirements as FR-001, FR-002, etc.
                            - Include a '## Requirements' section
                            - Start with YAML frontmatter containing `status: draft`{research_frontmatter}{research_section}                          2. `specs/{specname}/PLAN.md` — An implementation plan with:
                              - A '## Tasks' section with a markdown table
                              - Columns: ID, Title, Requirement, Effort, Priority, Status, Dependencies
                              - Task IDs as T-001, T-002, etc.
                              - Link each task to relevant requirements
                              - Effort values: S, M, L
                              - Priority values: Critical, High, Medium, Low
                              - Status values: Pending (set all new tasks to Pending)
          
                         3. `specs/{specname}/TESTPLAN.md` — A **manual** test plan (human-readable, not automated test code):
                            - Start with YAML frontmatter containing `status: draft`
                            - A `## Test Cases` section with one or more manual test cases
                            - Each test case has an ID (`TC-001`, `TC-002`, …), a title, preconditions, step-by-step instructions, test data to enter, and expected results
                            - When the feature involves user-interface navigation, enumerate every UI navigation step (keys pressed, menus opened, dialogs interacted with) and the exact data to enter into each field
                            - You MAY include a `## Prerequisites` section listing environment setup, provider configuration, or sample files needed before the manual tests can be executed
                            - You MAY include a `## Cleanup` section describing teardown steps to run after the manual tests complete
                            - Do NOT include automated test code, `#[test]` functions, or references to `cargo test`; this is a manual test plan only
          
                         Use the `write` tool to create all three files. Ensure the spec is clear, testable, and complete."
        )
    }

    // ── Specify helpers (FR-001) ──────────────────────────────────────────

    /// Build the user-facing status string for a specify operation.
    #[must_use]
    pub fn build_specify_status(specname: &str) -> String {
        format!("spec: writing specs/{specname}/SPEC.md…")
    }

    /// Build the assistant message shown when a specify operation starts.
    #[must_use]
    pub fn build_specify_message(specname: &str, _feature: &str) -> String {
        format!(
            "From: /spec specify\n📝 **Generating specification…**\n\n\
       Creating spec directory `specs/{specname}` with:\n\
       - `specs/{specname}/SPEC.md` — EARS requirements specification\n\n\
       No PLAN.md is generated at this stage. After reviewing and resolving \
       any `[NEEDS CLARIFICATION]` markers, use `/spec plan {specname} \
       <tech-context>` to generate the implementation plan.\n\n\
       This may take a few moments."
        )
    }

    /// Build the log entry for a specify operation.
    #[must_use]
    pub fn build_specify_log(specname: &str, feature: &str) -> String {
        format!(
            "Specifying '{specname}' for feature: {feature} → specs/{specname}/SPEC.md (no PLAN.md)"
        )
    }

    /// Build the prompt sent to the explore agent for SPEC.md-only generation.
    ///
    /// Unlike `build_create_prompt`, this prompt instructs the agent to write
    /// **only** `SPEC.md` — no `PLAN.md` or `TESTPLAN.md`. It also directs the
    /// agent to insert `[NEEDS CLARIFICATION: <question>]` markers wherever a
    /// requirement is ambiguous, matching FR-002.
    ///
    /// When `from_research` is `Some(name)`, the prompt instructs the agent
    /// to include a `research:` field in the SPEC.md YAML frontmatter linking
    /// to the named research artifact (FR-010) and a `## Related Research`
    /// section in the SPEC.md body (T-020).
    #[must_use]
    pub fn build_specify_prompt(
        specname: &str,
        feature: &str,
        from_research: Option<&str>,
    ) -> String {
        let research_frontmatter = Self::build_research_frontmatter_instruction(from_research);
        let research_section = Self::build_research_section_instruction(from_research);
        format!(
            r#"You are an expert specification writer. Create a SPECIFICATION ONLY (no implementation plan) for the following feature.

**Feature:** {feature}

**Spec ID:** {specname}

Write the following file:

1. `specs/{specname}/SPEC.md` — A requirements specification using EARS notation:
   - Use at least one of each EARS template: ubiquitous, event-driven, state-driven, optional, unwanted
   - Number requirements as FR-001, FR-002, etc.
   - Include a '## Requirements' section
   - Start with YAML frontmatter containing `status: draft`{research_frontmatter}{research_section}
   - Where a requirement is ambiguous or lacks sufficient detail, insert a `[NEEDS CLARIFICATION: <question>]` marker on the line immediately after the requirement so it can be detected by `/spec validate`.

Do NOT create `PLAN.md`, `TESTPLAN.md`, or any other file. Only write `specs/{specname}/SPEC.md`.

Use the `write` tool to create the file. Ensure the spec is clear, testable, and complete."#
        )
    }

    // ── Branch helpers (FR-009) ────────────────────────────────────────────

    /// Build the user-facing message for a branch-creation result.
    ///
    /// Returns a short one-line summary suitable for appending to the
    /// assistant message after the specify prompt has been queued.
    #[must_use]
    pub fn build_branch_message(result: &crate::git::BranchResult) -> String {
        use crate::git::BranchResult;
        match result {
            BranchResult::Created { branch_name } => {
                format!(
                    "🌿 **Git branch created:** `{branch_name}` — spec work is now \
                     isolated on this branch."
                )
            }
            BranchResult::NotARepo => {
                "ℹ️ Not a git repository — skipping branch creation.".to_string()
            }
            BranchResult::AlreadyExists { branch_name } => {
                format!("ℹ️ Branch `{branch_name}` already exists — reusing existing branch.")
            }
            BranchResult::Failed { msg } => {
                format!("⚠️ Could not create git branch: {msg}")
            }
        }
    }

    /// Build the log entry for a branch-creation result.
    #[must_use]
    pub fn build_branch_log(specname: &str, result: &crate::git::BranchResult) -> String {
        use crate::git::BranchResult;
        match result {
            BranchResult::Created { branch_name } => {
                format!("Created git branch '{branch_name}' for spec '{specname}'")
            }
            BranchResult::NotARepo => {
                format!("No git repo found — skipped branch creation for spec '{specname}'")
            }
            BranchResult::AlreadyExists { branch_name } => {
                format!("Branch '{branch_name}' already exists for spec '{specname}'")
            }
            BranchResult::Failed { msg } => {
                format!("Branch creation failed for spec '{specname}': {msg}")
            }
        }
    }

    // ── Research linking helpers (FR-010) ────────────────────────────────────

    /// Build the YAML frontmatter instruction for linking a research artifact.
    ///
    /// When `from_research` is `Some(name)`, returns a prompt instruction
    /// telling the agent to add a `research:` field to the SPEC.md YAML
    /// frontmatter. When `None`, returns an empty string so the prompt is
    /// unchanged for specs without a research link.
    #[must_use]
    pub fn build_research_frontmatter_instruction(from_research: Option<&str>) -> String {
        match from_research {
            Some(name) => format!(
                "\n                              - Include a `research:` field in the \
                 frontmatter linking to this research artifact:\n                                \
                 `research: [\"{name}\"]`"
            ),
            None => String::new(),
        }
    }

    /// Build the prompt instruction for a `## Related Research` body section
    /// (FR-010, T-020).
    ///
    /// When `from_research` is `Some(name)`, returns a prompt instruction
    /// telling the agent to include a `## Related Research` section in the
    /// SPEC.md body that references the research artifact. When `None`,
    /// returns an empty string so the prompt is unchanged for specs without a
    /// research link.
    #[must_use]
    pub fn build_research_section_instruction(from_research: Option<&str>) -> String {
        match from_research {
            Some(name) => format!(
                "\n                              - Include a `## Related Research` section \
                 in the body of SPEC.md referencing the research artifact:\n                                \
                 `This spec was informed by [`{name}`](../research/{name}/RESEARCH.md).`"
            ),
            None => String::new(),
        }
    }

    // ── Plan helpers (FR-004) ───────────────────────────────────────────────

    /// Build the user-facing status string for a plan generation operation.
    #[must_use]
    pub fn build_plan_status(spec_id: &str) -> String {
        format!("spec: writing specs/{spec_id}/PLAN.md…")
    }

    /// Build the assistant message shown when a plan generation starts.
    ///
    /// When `data_model_enabled` is `true`, the message also notes that a
    /// `data-model.md` artifact may be generated if the spec involves data
    /// entities (FR-011).  When `contracts_enabled` is `true`, the message
    /// notes that a `contracts/` directory may be generated if the spec
    /// defines API endpoints or inter-service contracts (FR-012).  When
    /// `feedback_enabled` is `true`, the message notes that production
    /// feedback notes from `FEEDBACK.md` will be considered during plan
    /// regeneration (FR-017).
    #[must_use]
    pub fn build_plan_message(
        spec_id: &str,
        tech_context: &str,
        data_model_enabled: bool,
        contracts_enabled: bool,
        feedback_enabled: bool,
    ) -> String {
        let data_model_note = if data_model_enabled {
            "\n\
             If the spec involves data entities, a `data-model.md` will also be \
             generated describing the domain data models."
        } else {
            ""
        };
        let contracts_note = if contracts_enabled {
            "\n\
             If the spec defines API endpoints or inter-service contracts, a \
             `contracts/` directory will also be generated with individual contract \
             definition files."
        } else {
            ""
        };
        let feedback_note = if feedback_enabled {
            "\n\
             Production feedback notes from `FEEDBACK.md` will be considered \
             during plan regeneration."
        } else {
            ""
        };
        format!(
            "From: /spec plan\n📋 **Generating implementation plan…**\n\n\
             Reading `specs/{spec_id}/SPEC.md` and generating `specs/{spec_id}/PLAN.md` \
             using the provided technology context:\n\
             > {tech_context}\n\n\
             If an existing PLAN.md is present, task statuses for unchanged task IDs \
             will be preserved.{data_model_note}{contracts_note}{feedback_note}\n\n\
             This may take a few moments."
        )
    }

    /// Build the log entry for a plan generation operation.
    #[must_use]
    pub fn build_plan_log(spec_id: &str, tech_context: &str) -> String {
        format!(
            "Generating PLAN.md for spec '{spec_id}' from SPEC.md + tech context: \
             {tech_context} → specs/{spec_id}/PLAN.md"
        )
    }

    /// Build the prompt sent to the explore agent for PLAN.md generation.
    ///
    /// Unlike `build_update_prompt`, this prompt accepts a technology-context
    /// argument that informs technology choices and rationale documented in the
    /// plan. The existing `SPEC.md` content is included so the agent generates a
    /// plan matching the current requirements. If `plan_md` is non-empty, the
    /// agent is instructed to preserve task statuses for unchanged task IDs.
    ///
    /// When `data_model_enabled` is `true`, the prompt also instructs the agent
    /// to generate an optional `data-model.md` artifact if the spec involves
    /// data entities (FR-011).  When `contracts_enabled` is `true`, the prompt
    /// instructs the agent to generate an optional `contracts/` directory if
    /// the spec defines API endpoints or inter-service contracts (FR-012).
    /// When `feedback_md` is non-empty, the prompt includes production feedback
    /// notes from `FEEDBACK.md` and instructs the agent to consider them during
    /// plan regeneration (FR-017).
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    /// * `tech_context` — Free-text technology context guiding plan decisions.
    /// * `spec_md` — The current `SPEC.md` content.
    /// * `plan_md` — The current `PLAN.md` content (may be empty if none exists).
    /// * `data_model_enabled` — When `true`, append a data-model.md generation
    ///   instruction to the prompt (gated by `sdd.data_model` config flag).
    /// * `contracts_enabled` — When `true`, append a contracts/ directory
    ///   generation instruction to the prompt (gated by `sdd.contracts` flag).
    /// * `feedback_md` — Production feedback notes from `FEEDBACK.md`. When
    ///   non-empty, the notes are included in the prompt and the agent is
    ///   instructed to address them in the regenerated plan (FR-017).
    #[must_use]
    pub fn build_plan_prompt(
        spec_id: &str,
        tech_context: &str,
        spec_md: &str,
        plan_md: &str,
        data_model_enabled: bool,
        contracts_enabled: bool,
        feedback_md: &str,
    ) -> String {
        let preservation_note = if plan_md.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\n\n**Existing PLAN.md content (preserve task statuses for unchanged IDs):**\n\n\
                 {plan_md}\n"
            )
        };
        let task_id_rule = if plan_md.trim().is_empty() {
            "Task IDs as T-001, T-002, etc.".to_string()
        } else {
            "Task IDs as T-001, T-002, etc. Preserve the existing task IDs and their \
             statuses where the task is unchanged; only add new tasks with the next \
             available ID."
                .to_string()
        };
        let data_model_instruction = if data_model_enabled {
            Self::build_data_model_instruction(spec_id)
        } else {
            String::new()
        };
        let contracts_instruction = if contracts_enabled {
            Self::build_contracts_instruction(spec_id)
        } else {
            String::new()
        };
        let feedback_instruction = if !feedback_md.trim().is_empty() {
            Self::build_feedback_instruction(spec_id, feedback_md)
        } else {
            String::new()
        };
        format!(
            r#"You are an expert software architect. Generate an implementation plan from an existing specification and a technology context.

**Spec ID:** {spec_id}

**Technology context:** {tech_context}

**Existing SPEC.md content:**

{spec_md}{preservation_note}

Write `specs/{spec_id}/PLAN.md` with:

1. A `## Tasks` section with a markdown table.
2. Columns: ID, Title, Requirement, Effort, Priority, Status, Dependencies.
3. {task_id_rule}
4. Link each task to relevant FR-NNN / NFR-NNN requirement IDs from the SPEC.md.
5. Effort values: S, M, L.
6. Priority values: Critical, High, Medium, Low.
7. Set the Status column to `Pending` for every new task.
8. A `## Technology Choices` section documenting the key technology decisions informed by the technology context above, with rationale.

Do NOT modify `SPEC.md` or any other file. Only write `specs/{spec_id}/PLAN.md`.{data_model_instruction}{contracts_instruction}{feedback_instruction}

Use the `write` tool to create the file. Ensure the plan is clear, actionable, and complete."#
        )
    }

    // ── Data-model helpers (FR-011, T-021) ───────────────────────────────────

    /// Build the prompt instruction for optional `data-model.md` generation.
    ///
    /// Returns an instruction appended to the `/spec plan` prompt telling the
    /// agent to analyse the SPEC.md for data entities and, if any are
    /// identified, write a `data-model.md` file describing the domain data
    /// models. When no data entities are present the file is not created
    /// (FR-011).
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    #[must_use]
    pub fn build_data_model_instruction(spec_id: &str) -> String {
        format!(
            "\n\nAdditionally, analyse the SPEC.md for data entities (domain objects, \
             persistent state, message payloads, configuration structures). If \
             data entities are identified, also write `specs/{spec_id}/data-model.md` \
             with:\n\
             \n  - A `## Entities` section listing each entity with its name, \
             description, and key attributes.\n  - A `## Relationships` section \
             describing how entities relate to each other.\n  - A `## Constraints` \
             section documenting any data integrity rules or invariants.\n\
             \nIf no data entities are identified, do NOT create `data-model.md`."
        )
    }

    // ── Contracts helpers (FR-012, T-022) ────────────────────────────────────

    /// Build the prompt instruction for optional `contracts/` directory
    /// generation.
    ///
    /// Returns an instruction appended to the `/spec plan` prompt telling the
    /// agent to analyse the SPEC.md for API endpoints or inter-service
    /// contracts and, if any are identified, create a `contracts/` directory
    /// with individual contract definition files. When no API endpoints or
    /// contracts are present the directory is not created (FR-012).
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    #[must_use]
    pub fn build_contracts_instruction(spec_id: &str) -> String {
        format!(
            "\n\nAdditionally, analyse the SPEC.md for API endpoints or inter-service \
             contracts (REST routes, gRPC services, event topics, message queues, \
             inter-process interfaces). If any are identified, also create a \
             `specs/{spec_id}/contracts/` directory with individual contract \
             definition files, one per endpoint or contract. Each contract file \
             shall document:\n  - The contract name and type (REST, gRPC, event, \
             etc.).\n  - Request/response schemas or message payloads with field \
             names, types, and descriptions.\n  - Error codes or failure modes.\n  \
             - Authentication and authorisation requirements.\n\nIf no API endpoints \
             or inter-service contracts are identified, do NOT create the \
             `contracts/` directory."
        )
    }

    // ── Feedback surfacing helpers (FR-017, T-033) ───────────────────────────

    /// Build the prompt instruction for surfacing production feedback notes
    /// during plan regeneration.
    ///
    /// Returns an instruction appended to the `/spec plan` prompt that embeds
    /// the `FEEDBACK.md` content and tells the agent to consider the notes
    /// when generating or regenerating the plan. The agent is instructed to
    /// address each feedback note by adding or adjusting tasks as needed
    /// (FR-017).
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    /// * `feedback_md` — The contents of `FEEDBACK.md` (production metrics,
    ///   incident reports, user feedback notes).
    #[must_use]
    pub fn build_feedback_instruction(spec_id: &str, feedback_md: &str) -> String {
        format!(
            "\n\n**Production feedback notes from `specs/{spec_id}/FEEDBACK.md`:**\n\n\
             {feedback_md}\n\n\
             Consider the above production feedback notes when generating the \
             plan. Where a feedback note identifies an issue, gap, or improvement \
             opportunity, add or adjust tasks to address it. Reference the \
             relevant feedback note in the task's rationale where applicable."
        )
    }

    // ── Tasks helpers (FR-005, T-006, FR-013, T-023) ───────────────────────

    /// Build the user-facing status string for a tasks extraction operation.
    #[must_use]
    pub fn build_tasks_status(spec_id: &str) -> String {
        format!("spec: extracting specs/{spec_id}/TASKS.md from PLAN.md…")
    }

    /// Build the assistant message shown when a tasks extraction starts.
    #[must_use]
    pub fn build_tasks_message(spec_id: &str) -> String {
        format!(
            "From: /spec tasks\n📋 **Extracting task list…**\n\n\
             Reading `specs/{spec_id}/PLAN.md` and generating a standalone \
             `specs/{spec_id}/TASKS.md` with the ordered task table. \
             Also generating `specs/{spec_id}/quickstart.md` with key \
             validation scenarios from `SPEC.md`.\n\n\
             This is a deterministic extraction — no LLM call required."
        )
    }

    /// Build the log entry for a tasks extraction operation.
    #[must_use]
    pub fn build_tasks_log(spec_id: &str) -> String {
        format!(
            "Extracting TASKS.md for spec '{spec_id}' from PLAN.md → \
             specs/{spec_id}/TASKS.md"
        )
    }

    /// Build the completion message shown after TASKS.md and quickstart.md are
    /// written (FR-013, T-023).
    #[must_use]
    pub fn build_tasks_completion_message(spec_id: &str, task_count: usize) -> String {
        format!(
            "From: /spec tasks\n✅ **TASKS.md + quickstart.md generated** — \
             `specs/{spec_id}/TASKS.md` contains {task_count} task(s) \
             extracted from `PLAN.md`. `specs/{spec_id}/quickstart.md` \
             contains key validation scenarios derived from `SPEC.md`."
        )
    }

    /// Build the error message shown when PLAN.md has no parseable tasks.
    #[must_use]
    pub fn build_tasks_no_tasks_error(spec_id: &str) -> String {
        format!(
            "From: /spec tasks\n\n**Error:** `specs/{spec_id}/PLAN.md` does \
             not contain a valid `## Tasks` table. Ensure the plan has been \
             generated first (e.g. via `/spec plan {spec_id} <tech-context>` \
             or `/spec create`)."
        )
    }

    /// Build the error message shown when PLAN.md is missing.
    #[must_use]
    pub fn build_tasks_no_plan_error(spec_id: &str) -> String {
        format!(
            "From: /spec tasks\n\n**Error:** `specs/{spec_id}/PLAN.md` not \
             found. Generate a plan first using `/spec plan {spec_id} \
             <tech-context>` or `/spec create {spec_id} <feature>`."
        )
    }

    /// Generate the standalone `TASKS.md` content from a PLAN.md's task table.
    ///
    /// Parses the `## Tasks` section from `plan_md` using [`PlanParser`] and
    /// formats the parsed tasks into a standalone markdown file with a header,
    /// the task table, and a footer noting the source.
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    /// * `title` — The spec title (extracted from SPEC.md H1).
    /// * `plan_md` — The raw `PLAN.md` content.
    ///
    /// # Returns
    ///
    /// `Some(tasks_md)` if tasks were found and formatted, or `None` if the
    /// PLAN.md contains no parseable task table.
    #[must_use]
    pub fn build_tasks_md(spec_id: &str, title: &str, plan_md: &str) -> Option<String> {
        let tasks = crate::plan_parser::PlanParser::parse(plan_md).ok()?;
        if tasks.is_empty() {
            return None;
        }

        let mut md = String::new();
        md.push_str("# Tasks\n\n");
        md.push_str(&format!("> Derived from `specs/{spec_id}/PLAN.md`\n\n"));
        if !title.is_empty() {
            md.push_str(&format!("**Spec:** {title}\n\n"));
        }

        md.push_str("## Task Table\n\n");
        md.push_str("| ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n");
        md.push_str("|---|---|---|---|---|---|---|\n");

        for task in &tasks {
            let deps = if task.dependencies.is_empty() {
                "—".to_string()
            } else {
                task.dependencies.join(", ")
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                task.id,
                task.title,
                task.requirement,
                task.effort,
                task.priority,
                task.status.as_str(),
                deps,
            ));
        }

        md.push_str("\n---\n\n");
        md.push_str(
            "*This file was generated by `/spec tasks` from the \
                      `## Tasks` table in `PLAN.md`. Edit `PLAN.md` and \
                      re-run `/spec tasks` to update.*\n",
        );

        Some(md)
    }

    /// Generate the standalone `quickstart.md` containing key validation
    /// scenarios derived from the spec's acceptance criteria (FR-013, T-023).
    ///
    /// Parses the requirements from `spec_md` using
    /// [`validate::parse_requirements`] and formats each requirement into a
    /// concise smoke-test scenario. The output is a standalone markdown file
    /// intended for quick validation, distinct from the full `TESTPLAN.md`.
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    /// * `title` — The spec title (extracted from SPEC.md H1).
    /// * `spec_md` — The raw `SPEC.md` content containing the requirements.
    ///
    /// # Returns
    ///
    /// `Some(quickstart_md)` if requirements were found, or `None` if the
    /// SPEC.md contains no parseable requirements.
    #[must_use]
    pub fn build_quickstart_md(spec_id: &str, title: &str, spec_md: &str) -> Option<String> {
        let reqs = crate::validate::parse_requirements(spec_md);
        if reqs.is_empty() {
            return None;
        }

        let mut md = String::new();
        md.push_str("# Quickstart Validation Scenarios\n\n");
        md.push_str(&format!("> Derived from `specs/{spec_id}/SPEC.md`\n\n"));
        if !title.is_empty() {
            md.push_str(&format!("**Spec:** {title}\n\n"));
        }

        md.push_str(
            "These smoke-test scenarios verify the key acceptance criteria from \
             the spec. Run through them after implementation to confirm the \
             core functionality works before executing the full \
             `TESTPLAN.md`.\n\n",
        );

        md.push_str("## Scenarios\n\n");

        for req in &reqs {
            md.push_str(&format!("### {}\n", req.id));
            md.push_str(&format!("**{}**\n\n", req.title));
            if !req.ears_text.is_empty() {
                md.push_str(&format!("Verify that: {}\n\n", req.ears_text));
            } else {
                md.push_str("Verify that the requirement is satisfied.\n\n");
            }
        }

        md.push_str("---\n\n");
        md.push_str(
            "*This file was generated by `/spec tasks` from the requirements \
             in `SPEC.md`. Edit `SPEC.md` and re-run `/spec tasks` to \
             update.*\n",
        );

        Some(md)
    }

    // ── Add helpers ────────────────────────────────────────────────────────

    /// Build the user-facing status string for an add operation.
    #[must_use]
    pub fn build_add_status(spec_id: &str) -> String {
        format!("spec: updating specs/{spec_id}/SPEC.md + specs/{spec_id}/PLAN.md…")
    }

    /// Build the assistant message shown when an incremental spec update starts.
    #[must_use]
    pub fn build_add_message(spec_id: &str, feature: &str) -> String {
        format!(
            "From: /spec add\n📝 **Adding requirements to spec…**\n\n\
                   Updating spec `specs/{spec_id}/` with new feature:\n\
                   > {feature}\n\n\
                   - Reading existing `SPEC.md` and `PLAN.md`\n\
                   - Generating incremental requirements and tasks\n\
                   - Inserting new content without modifying existing sections\n\n\
                   This may take a few moments."
        )
    }

    /// Build the log entry for an add operation.
    #[must_use]
    pub fn build_add_log(spec_id: &str, feature: &str) -> String {
        format!("Adding requirements to spec '{spec_id}' for feature: {feature}")
    }

    /// Build the prompt sent to the LLM agent for incremental spec updates.
    ///
    /// The prompt includes the existing `SPEC.md` and `PLAN.md` content and
    /// instructs the LLM to generate **only** the incremental additions — new
    /// requirement blocks and new task rows — rather than rewriting the entire
    /// documents.
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (e.g. `"my-feature"`).
    /// * `feature` — The free-text feature description for the new requirements.
    /// * `spec_md` — The current `SPEC.md` content.
    /// * `plan_md` — The current `PLAN.md` content.
    /// * `next_fr` — The next available `FR-NNN` number.
    /// * `next_nfr` — The next available `NFR-NNN` number.
    /// * `next_task` — The next available `T-NNN` number.
    #[must_use]
    pub fn build_add_prompt(
        spec_id: &str,
        feature: &str,
        spec_md: &str,
        plan_md: &str,
        next_fr: u32,
        next_nfr: u32,
        next_task: u32,
    ) -> String {
        format!(
            r"You are an expert specification writer. Incrementally add new requirements to an existing spec.
    
    **Feature to add:** {feature}
    
    **Spec ID:** {spec_id}
    
    **IMPORTANT:** You are UPDATING an existing spec, NOT creating a new one. Generate ONLY the new requirement blocks and task rows that should be inserted. Do NOT rewrite the entire spec or plan. Do NOT modify, reorder, or delete any existing content.
    
    **Existing SPEC.md content:**
    
    {spec_md}
    
    **Existing PLAN.md content:**
    
    {plan_md}
    
    **Numbering rules:**
    - New functional requirements start at **FR-{next_fr:03}** and increment sequentially.
    - New non-functional requirements start at **NFR-{next_nfr:03}** and increment sequentially.
    - New tasks start at **T-{next_task:03}** and increment sequentially.
    - Do NOT renumber or rename any existing IDs.
    
    **Output format — use the exact delimiters below:**
    
    ---NEW REQUIREMENTS---
    
    (Insert new requirement blocks here, using EARS notation with the correct FR-NNN or NFR-NNN IDs. Each requirement should be under a `###` heading indicating its section. Use at least one EARS template type. Group requirements under the existing section heading if one matches, or create a new `##` section heading.)
    
    ---NEW TASKS---
    
    (Insert new task table rows here, one per line, in the same markdown table format as the existing PLAN.md. Columns: | ID | Title | Requirement | Effort | Priority | Status | Dependencies |. Link each task to the new requirement IDs. Effort: S, M, L. Priority: Critical, High, Medium, Low. Status: Pending.)
    
    ---NEW TASK DETAILS---
    
    (Insert optional task detail subsections here, one `### T-NNN — Title` per task, with a short description of the implementation approach.)
    
    ---END---
    
    Ensure the spec is clear, testable, and complete.",
        )
    }

    /// Build the completion summary shown after a successful add operation.
    #[must_use]
    pub fn build_add_completion_summary(
        spec_id: &str,
        new_req_ids: &[String],
        new_task_ids: &[String],
    ) -> String {
        let req_list = if new_req_ids.is_empty() {
            "none".to_string()
        } else {
            new_req_ids.join(", ")
        };
        let task_list = if new_task_ids.is_empty() {
            "none".to_string()
        } else {
            new_task_ids.join(", ")
        };
        format!(
            "From: /spec add\n✅ **Spec updated successfully.**\n\n\
                   Spec `specs/{}/` now includes:\n\
                   - **New requirements:** {} ({})\n\
                   - **New tasks:** {} ({})\n\n\
                   Validate with `/spec validate {}`.",
            spec_id,
            new_req_ids.len(),
            req_list,
            new_task_ids.len(),
            task_list,
            spec_id,
        )
    }

    // ── JTBD helpers ───────────────────────────────────────────────────────

    /// Build the user-facing status string for a JTBD analysis operation.
    #[must_use]
    pub fn build_jtbd_status(spec_id: &str) -> String {
        format!("spec jtbd: {spec_id}")
    }

    /// Build the assistant message shown when a JTBD analysis starts.
    #[must_use]
    pub fn build_jtbd_message(spec_id: &str) -> String {
        format!(
            "From: /spec jtbd\n📋 **Performing JTBD analysis…**\n\n\
             Analyzing `specs/{spec_id}/SPEC.md` to extract Jobs-To-Be-Done.\n\n\
             - Reading the spec's overview and numbered requirements\n\
             - Identifying functional, emotional, and social jobs\n\
             - Tracing each job to FR/NFR requirement IDs\n\
             - Writing `specs/{spec_id}/JTBD.md`\n\n\
             This may take a few moments."
        )
    }

    /// Build the log entry for a JTBD analysis operation.
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier being analyzed.
    /// * `force` — Whether `--force` was supplied (overwrite existing file).
    /// * `agent` — Optional override agent name, if `--agent` was supplied.
    #[must_use]
    pub fn build_jtbd_log(spec_id: &str, force: bool, agent: Option<&str>) -> String {
        let force_tag = if force { " --force" } else { "" };
        match agent {
            Some(a) => format!(
                "JTBD analysis for spec '{spec_id}'{force_tag} --agent {a} → specs/{spec_id}/JTBD.md"
            ),
            None => {
                format!("JTBD analysis for spec '{spec_id}'{force_tag} → specs/{spec_id}/JTBD.md")
            }
        }
    }

    /// Build the prompt sent to the explore agent for JTBD analysis.
    ///
    /// The prompt instructs the agent to read the spec's `SPEC.md`, extract
    /// jobs using the JTBD framework, trace each job to requirement IDs, and
    /// write the result to `specs/<spec_id>/JTBD.md`.
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    #[must_use]
    pub fn build_jtbd_prompt(spec_id: &str) -> String {
        format!(
            r#"You are an expert product analyst performing a Jobs-To-Be-Done (JTBD) analysis.

**Spec ID:** {spec_id}

Read the file `specs/{spec_id}/SPEC.md` using the `read` tool. If the file is very large, read it in sections. Then analyse the spec's overview and numbered requirements to identify the underlying "jobs" the feature is hired to do.

Write your analysis to `specs/{spec_id}/JTBD.md` using the `write` tool. The document must contain:

1. **YAML frontmatter** with `status: draft`.

2. A **`## Overview`** section summarising the spec in one or two sentences.

3. A **`## Jobs`** section. For each job, use a `### Job N — <short title>` heading and include:

   - **Job statement** — expressed using the grammar:
     *"When <situation>, I want to <motivation>, so I can <expected outcome>."*
   - **Job type** — one of: *functional*, *emotional*, or *social*.
   - **Performer** — who is hiring the product (the spec's primary user).
   - **Related requirements** — list every `FR-NNN` and/or `NFR-NNN` identifier from `SPEC.md` that this job traces to. If no requirement traces to the job, write *untraced* explicitly so coverage gaps are visible.
   - **Success signals** — one or more observable indicators that the job is being fulfilled.

4. A **`## Out-of-Scope Jobs`** section listing jobs explicitly rejected or deferred (if any).

5. A **`## Coverage Matrix`** section with a markdown table mapping each `FR-NNN` / `NFR-NNN` to the job(s) it supports. Requirements with no corresponding job should be marked *unmapped*.

Use the `write` tool to create `specs/{spec_id}/JTBD.md`. Ensure the markdown is well-formed and every job follows the grammar above."#,
        )
    }

    // ── Update helpers ─────────────────────────────────────────────────────

    /// Build the user-facing status string for an update operation.
    #[must_use]
    pub fn build_update_status(spec_id: &str) -> String {
        format!("spec: updating specs/{spec_id}/PLAN.md + specs/{spec_id}/TESTPLAN.md…")
    }

    /// Build the assistant message shown when a spec update starts.
    #[must_use]
    pub fn build_update_message(spec_id: &str) -> String {
        format!(
            "From: /spec update\n🔄 **Regenerating plan and test plan…**\n\n\
             Re-reading `specs/{spec_id}/SPEC.md` and regenerating:\n\
             - `specs/{spec_id}/PLAN.md` — implementation plan with tasks\n\
             - `specs/{spec_id}/TESTPLAN.md` — manual test plan with test cases\n\n\
             The `SPEC.md` file will not be modified.\n\
             This may take a few moments."
        )
    }

    /// Build the log entry for an update operation.
    #[must_use]
    pub fn build_update_log(spec_id: &str) -> String {
        format!("Regenerating PLAN.md + TESTPLAN.md for spec '{spec_id}' from existing SPEC.md")
    }

    /// Build the prompt sent to the LLM agent for spec plan/test-plan regeneration.
    ///
    /// The prompt instructs the agent to read the existing `SPEC.md` and
    /// regenerate `PLAN.md` and `TESTPLAN.md` to match the current requirements.
    /// It explicitly forbids modifying `SPEC.md`. The existing `PLAN.md` content
    /// is included so the agent can preserve task statuses for unchanged task
    /// IDs (FR-011).
    ///
    /// # Arguments
    ///
    /// * `spec_id` — The spec identifier (directory name under `specs/`).
    /// * `plan_md` — The current `PLAN.md` content, used for status preservation.
    #[must_use]
    pub fn build_update_prompt(spec_id: &str, plan_md: &str) -> String {
        format!(
            r#"You are an expert specification writer. An existing spec has been updated.
Re-read the current `specs/{spec_id}/SPEC.md` and regenerate `specs/{spec_id}/PLAN.md` and `specs/{spec_id}/TESTPLAN.md` to match.

**Spec ID:** {spec_id}

**IMPORTANT:** Do NOT modify the `SPEC.md` file. Only regenerate `PLAN.md` and `TESTPLAN.md`.

Read `specs/{spec_id}/SPEC.md` using the `read` tool first. If the file is large, read it in sections.

Then write the following files using the `write` tool:

1. `specs/{spec_id}/PLAN.md` — An implementation plan with:
     - A `## Tasks` section with a markdown table.
     - Columns: ID, Title, Requirement, Effort, Priority, Status, Dependencies.
     - Task IDs as T-001, T-002, etc.
     - Link each task to relevant requirements from `SPEC.md`.
     - Effort values: S, M, L.
     - Priority values: Critical, High, Medium, Low.
     - Status values: Pending for all new tasks.
     - Preserve the status of any existing task IDs that remain unchanged.

2. `specs/{spec_id}/TESTPLAN.md` — A **manual** test plan (human-readable, not automated test code):
   - YAML frontmatter with `status: draft`.
   - A `## Test Cases` section with manual test cases.
   - Each test case has an ID (`TC-001`, `TC-002`, …), a title, preconditions, step-by-step instructions, test data to enter, and expected results.
   - Do NOT include automated test code, `#[test]` functions, or references to `cargo test`.

**Existing PLAN.md content (for reference — preserve task statuses where IDs are unchanged):**

{plan_md}

Use the `write` tool to overwrite `PLAN.md` and `TESTPLAN.md`. Ensure the plan and test plan are clear, testable, and complete."#,
        )
    }

    // ── Feedback helpers (FR-017, T-032) ─────────────────────────────────────

    /// Build the user-facing status string for a feedback append operation.
    #[must_use]
    pub fn build_feedback_status(spec_id: &str) -> String {
        format!("spec: appending feedback to specs/{spec_id}/FEEDBACK.md…")
    }

    /// Build the assistant message shown when a feedback note is appended.
    #[must_use]
    pub fn build_feedback_message(spec_id: &str, note: &str) -> String {
        format!(
            "From: /spec feedback\n📝 **Feedback note appended** to \
             `specs/{spec_id}/FEEDBACK.md`.\n\n\
             > {note}\n\n\
             This note is advisory — it will be surfaced during the next \
             `/spec plan` regeneration but does not block validation or \
             status transitions."
        )
    }

    /// Build the log entry for a feedback append operation.
    #[must_use]
    pub fn build_feedback_log(spec_id: &str, note: &str) -> String {
        // Truncate very long notes in the log for readability.
        let preview = if note.len() > 80 {
            format!("{}…", &note[..80])
        } else {
            note.to_string()
        };
        format!("Appended feedback note to specs/{spec_id}/FEEDBACK.md: {preview}")
    }

    /// Format a feedback note as a markdown table row with the current UTC date.
    ///
    /// The row follows the `FEEDBACK.md` template format:
    /// `| YYYY-MM-DD | user | <note> |`.
    #[must_use]
    pub fn format_feedback_row(note: &str) -> String {
        let date = current_utc_date_string();
        // Escape pipe characters in the note so they don't break the table.
        let escaped = note.replace('|', "\\|");
        format!("| {date} | user | {escaped} |")
    }

    /// Append a feedback note to existing `FEEDBACK.md` content.
    ///
    /// If `existing` is empty, a new file is generated from
    /// [`FeedbackTemplate::generate`] using `title` as the spec title, and the
    /// note is inserted as the first real row (replacing the placeholder row).
    ///
    /// If `existing` already has content, the new row is inserted before the
    /// `---` separator (or appended at the end if no separator is found).
    #[must_use]
    pub fn append_feedback_note(existing: &str, title: &str, note: &str) -> String {
        let row = Self::format_feedback_row(note);

        if existing.is_empty() {
            // Generate from template and replace the placeholder row.
            let template = crate::templates::FeedbackTemplate::generate(title);
            return replace_placeholder_row(&template, &row);
        }

        // Try to insert before the trailing "---" separator.
        if let Some(pos) = existing.rfind("\n---\n") {
            let before = &existing[..pos];
            let after = &existing[pos..];
            // Ensure there's a blank line between the table and the separator.
            let before_trimmed = before.trim_end();
            format!("{before_trimmed}\n{row}\n{after}")
        } else {
            // No separator found — just append.
            let trimmed = existing.trim_end();
            format!("{trimmed}\n{row}\n")
        }
    }
}

/// Replace the placeholder row in a freshly-generated FEEDBACK.md template
/// with the actual feedback row.
///
/// The template contains a row like `| [YYYY-MM-DD] | [metric/incident/user] | [Feedback note] |`.
/// We replace that line with the real row.
fn replace_placeholder_row(template: &str, real_row: &str) -> String {
    template
        .lines()
        .map(|line| {
            if line.starts_with("| [YYYY-MM-DD]") {
                real_row.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Get the current UTC date as a `YYYY-MM-DD` string.
fn current_utc_date_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Convert Unix timestamp to YYYY-MM-DD using simple date arithmetic.
    let days = secs / 86_400;
    date_from_days_since_epoch(days as i64)
}

/// Convert days-since-epoch to a `YYYY-MM-DD` string.
///
/// Uses the proleptic Gregorian calendar. Based on the well-known
/// Howard Hinnant date algorithm.
fn date_from_days_since_epoch(days: i64) -> String {
    let z = days + 719_468; // days from 0000-03-01
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}
