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
    /// Delete a spec directory from the workspace.
    Delete {
        /// Spec identifier.
        spec_id: String,
        /// If true, skip the confirmation prompt.
        yes: bool,
    },
    /// Unknown subcommand (preserves the raw name for error messages).
    Unknown(String),
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
                let (specname, feature) = rest
                    .split_once(char::is_whitespace)
                    .map_or(("", rest), |(s, r)| (s.trim(), r.trim()));
                if specname.is_empty() || feature.is_empty() {
                    // Caller should treat this as a usage error.
                    Self::Unknown("create".to_string())
                } else {
                    Self::Create {
                        specname: specname.to_string(),
                        feature: feature.to_string(),
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
                    | `/spec impl <spec-id> [--task <ID>] [--dry-run]` | required `spec-id`, optional flags | Implement a spec by executing its PLAN.md tasks in dependency order. Use `--task` to run a single task, `--dry-run` to preview the plan. Alias: `/spec implement`. |\n\n\
                    Example: `/spec create websocket Add a real-time collaborative editing feature using WebSockets --from-research realtime-collab`"
    }
    /// Build the user-facing status string for a create operation.
    #[must_use]
    pub fn build_create_status(specname: &str) -> String {
        format!("spec: writing specs/{specname}/SPEC.md + specs/{specname}/PLAN.md…")
    }

    /// Build the assistant message shown when a spec generation starts.
    #[must_use]
    pub fn build_create_message(specname: &str, _feature: &str) -> String {
        format!(
            "From: /spec\n📝 **Generating specification and plan…**\n\n\
             Creating spec directory `specs/{specname}` with:\n\
             - `specs/{specname}/SPEC.md` — EARS requirements specification\n\
             - `specs/{specname}/PLAN.md` — implementation plan with tasks\n\n\
             This may take a few moments.\n\
             ⚠️ **Tip:** After creation, you can validate with `/spec validate {specname}`."
        )
    }

    /// Build the log entry for a create operation.
    #[must_use]
    pub fn build_create_log(specname: &str, feature: &str) -> String {
        format!("Creating spec '{specname}' for feature: {feature} → specs/{specname}/SPEC.md")
    }

    /// Build the prompt sent to the explore agent for spec generation.
    #[must_use]
    pub fn build_create_prompt(specname: &str, feature: &str) -> String {
        format!(
            r"You are an expert specification writer. Create a specification and implementation plan for the following feature.
          
                         **Feature:** {feature}
          
                         **Spec ID:** {specname}
          
                         Write the following files:
          
                         1. `specs/{specname}/SPEC.md` — A requirements specification using EARS notation:
                            - Use at least one of each EARS template: ubiquitous, event-driven, state-driven, optional, unwanted
                            - Number requirements as FR-001, FR-002, etc.
                            - Include a '## Requirements' section
                            - Start with YAML frontmatter containing `status: draft`
          
                         2. `specs/{specname}/PLAN.md` — An implementation plan with:
                            - A '## Tasks' section with a markdown table
                            - Columns: ID, Title, Requirement, Effort, Priority, Dependencies
                            - Task IDs as T-001, T-002, etc.
                            - Link each task to relevant requirements
                            - Effort values: S, M, L
                            - Priority values: Critical, High, Medium, Low
          
                         Use the `write` tool to create both files. Ensure the spec is clear, testable, and complete."
        )
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
    
    (Insert new task table rows here, one per line, in the same markdown table format as the existing PLAN.md. Columns: | ID | Title | Requirement | Effort | Priority | Dependencies |. Link each task to the new requirement IDs. Effort: S, M, L. Priority: Critical, High, Medium, Low.)
    
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_help() {
        assert_eq!(SpecCommand::parse("help"), SpecCommand::Help);
        assert_eq!(SpecCommand::parse(""), SpecCommand::Help);
    }

    #[test]
    fn parse_create() {
        let cmd = SpecCommand::parse("create my-spec Add auth");
        assert!(
            matches!(cmd, SpecCommand::Create { specname, feature } if specname == "my-spec" && feature == "Add auth")
        );
    }

    #[test]
    fn parse_create_missing_feature() {
        assert!(
            matches!(SpecCommand::parse("create my-spec"), SpecCommand::Unknown(s) if s == "create")
        );
    }

    #[test]
    fn parse_validate() {
        assert_eq!(
            SpecCommand::parse("validate my-spec"),
            SpecCommand::Validate {
                spec_id: Some("my-spec".to_string()),
            }
        );
    }

    #[test]
    fn parse_validate_all() {
        assert_eq!(
            SpecCommand::parse("validate"),
            SpecCommand::Validate { spec_id: None }
        );
    }

    #[test]
    fn parse_list() {
        assert_eq!(
            SpecCommand::parse("list --status draft"),
            SpecCommand::List {
                args: "--status draft".to_string(),
            }
        );
    }

    #[test]
    fn parse_search() {
        assert_eq!(
            SpecCommand::parse("search websocket"),
            SpecCommand::Search {
                query: "websocket".to_string(),
            }
        );
    }

    #[test]
    fn parse_status_show() {
        assert_eq!(
            SpecCommand::parse("status my-spec"),
            SpecCommand::Status {
                spec_id: "my-spec".to_string(),
                new_status: None,
            }
        );
    }

    #[test]
    fn parse_status_transition() {
        assert_eq!(
            SpecCommand::parse("status my-spec in_review"),
            SpecCommand::Status {
                spec_id: "my-spec".to_string(),
                new_status: Some("in_review".to_string()),
            }
        );
    }

    #[test]
    fn parse_status_missing_id() {
        assert!(matches!(SpecCommand::parse("status"), SpecCommand::Unknown(s) if s == "status"));
    }

    #[test]
    fn parse_task_list() {
        assert_eq!(
            SpecCommand::parse("task my-spec"),
            SpecCommand::Task {
                spec_id: "my-spec".to_string(),
                task_id: None,
                new_status: None,
            }
        );
    }

    #[test]
    fn parse_task_show() {
        assert_eq!(
            SpecCommand::parse("task my-spec T-001"),
            SpecCommand::Task {
                spec_id: "my-spec".to_string(),
                task_id: Some("T-001".to_string()),
                new_status: None,
            }
        );
    }

    #[test]
    fn parse_task_update() {
        assert_eq!(
            SpecCommand::parse("task my-spec T-001 completed"),
            SpecCommand::Task {
                spec_id: "my-spec".to_string(),
                task_id: Some("T-001".to_string()),
                new_status: Some("completed".to_string()),
            }
        );
    }

    #[test]
    fn parse_task_missing_id() {
        assert!(matches!(SpecCommand::parse("task"), SpecCommand::Unknown(s) if s == "task"));
    }

    #[test]
    fn parse_activate() {
        assert_eq!(
            SpecCommand::parse("activate my-spec"),
            SpecCommand::Activate {
                spec_id: "my-spec".to_string(),
            }
        );
    }

    #[test]
    fn parse_deactivate() {
        assert_eq!(SpecCommand::parse("deactivate"), SpecCommand::Deactivate);
    }

    #[test]
    fn parse_coverage() {
        assert_eq!(
            SpecCommand::parse("coverage my-spec"),
            SpecCommand::Coverage {
                spec_id: "my-spec".to_string(),
            }
        );
    }

    #[test]
    fn parse_unknown() {
        assert!(matches!(SpecCommand::parse("foobar"), SpecCommand::Unknown(s) if s == "foobar"));
    }

    #[test]
    fn help_message_contains_expected_fragments() {
        let help = SpecCommand::build_help_message();
        assert!(help.contains("/spec create"));
        assert!(help.contains("/spec validate"));
        assert!(help.contains("/spec list"));
        assert!(help.contains("/spec search"));
        assert!(help.contains("/spec status"));
        assert!(help.contains("/spec task"));
        assert!(help.contains("/spec activate"));
        assert!(help.contains("/spec deactivate"));
        assert!(help.contains("/spec coverage"));
        assert!(help.contains("/spec impl"));
        assert!(help.contains("/spec implement"));
    }

    #[test]
    fn create_status_is_well_formed() {
        let s = SpecCommand::build_create_status("foo");
        assert!(s.contains("specs/foo/SPEC.md"));
    }

    #[test]
    fn create_message_contains_paths() {
        let m = SpecCommand::build_create_message("bar", "baz");
        assert!(m.contains("specs/bar"));
    }

    #[test]
    fn create_prompt_contains_feature_and_files() {
        let p = SpecCommand::build_create_prompt("qux", "quux");
        assert!(p.contains("quux"));
        assert!(p.contains("specs/qux/SPEC.md"));
    }

    #[test]
    fn parse_impl_basic() {
        assert_eq!(
            SpecCommand::parse("impl myspec"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: None,
                dry_run: false,
            }
        );
    }

    #[test]
    fn parse_impl_with_task() {
        assert_eq!(
            SpecCommand::parse("impl myspec --task T-003"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: Some("T-003".to_string()),
                dry_run: false,
            }
        );
    }

    #[test]
    fn parse_impl_dry_run() {
        assert_eq!(
            SpecCommand::parse("impl myspec --dry-run"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: None,
                dry_run: true,
            }
        );
    }

    #[test]
    fn parse_impl_all_options() {
        assert_eq!(
            SpecCommand::parse("impl myspec --task T-005 --dry-run"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: Some("T-005".to_string()),
                dry_run: true,
            }
        );
    }

    #[test]
    fn parse_impl_missing_specname() {
        assert!(matches!(
            SpecCommand::parse("impl"),
            SpecCommand::Unknown(s) if s == "impl"
        ));
    }

    #[test]
    fn parse_implement_basic() {
        assert_eq!(
            SpecCommand::parse("implement myspec"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: None,
                dry_run: false,
            }
        );
    }

    #[test]
    fn parse_implement_with_task() {
        assert_eq!(
            SpecCommand::parse("implement myspec --task T-003"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: Some("T-003".to_string()),
                dry_run: false,
            }
        );
    }

    #[test]
    fn parse_implement_dry_run() {
        assert_eq!(
            SpecCommand::parse("implement myspec --dry-run"),
            SpecCommand::Impl {
                spec_id: "myspec".to_string(),
                task_id: None,
                dry_run: true,
            }
        );
    }

    #[test]
    fn parse_implement_missing_specname() {
        assert!(matches!(
            SpecCommand::parse("implement"),
            SpecCommand::Unknown(s) if s == "impl"
        ));
    }

    #[test]
    fn parse_delete() {
        let cmd = SpecCommand::parse("delete my-spec --yes");
        assert!(matches!(cmd, SpecCommand::Delete { spec_id, yes } if spec_id == "my-spec" && yes));
    }

    #[test]
    fn parse_delete_without_yes() {
        let cmd = SpecCommand::parse("delete my-spec");
        assert!(
            matches!(cmd, SpecCommand::Delete { spec_id, yes } if spec_id == "my-spec" && !yes)
        );
    }

    #[test]
    fn parse_delete_missing_spec_id() {
        assert!(matches!(SpecCommand::parse("delete"), SpecCommand::Unknown(s) if s == "delete"));
    }

    #[test]
    fn parse_delete_is_usage_error() {
        let cmd = SpecCommand::Unknown("delete".to_string());
        assert!(cmd.is_usage_error());
    }

    #[test]
    fn help_message_contains_delete() {
        let help = SpecCommand::build_help_message();
        assert!(help.contains("/spec delete"));
    }

    #[test]
    fn parse_add() {
        let cmd = SpecCommand::parse("add my-spec Add new feature for X");
        assert!(
            matches!(cmd, SpecCommand::Add { spec_id, feature } if spec_id == "my-spec" && feature == "Add new feature for X")
        );
    }

    #[test]
    fn parse_add_missing_feature() {
        assert!(matches!(SpecCommand::parse("add my-spec"), SpecCommand::Unknown(s) if s == "add"));
    }

    #[test]
    fn parse_add_missing_spec_id() {
        assert!(matches!(SpecCommand::parse("add"), SpecCommand::Unknown(s) if s == "add"));
    }

    #[test]
    fn parse_add_is_usage_error() {
        let cmd = SpecCommand::Unknown("add".to_string());
        assert!(cmd.is_usage_error());
    }

    #[test]
    fn add_status_is_well_formed() {
        let s = SpecCommand::build_add_status("foo");
        assert!(s.contains("specs/foo/SPEC.md"));
        assert!(s.contains("specs/foo/PLAN.md"));
    }

    #[test]
    fn add_message_contains_spec_id_and_feature() {
        let m = SpecCommand::build_add_message("bar", "Add new auth flow");
        assert!(m.contains("specs/bar"));
        assert!(m.contains("Add new auth flow"));
    }

    #[test]
    fn add_log_contains_spec_id_and_feature() {
        let l = SpecCommand::build_add_log("baz", "Add caching");
        assert!(l.contains("baz"));
        assert!(l.contains("Add caching"));
    }

    #[test]
    fn add_prompt_contains_existing_content_and_numbering() {
        let spec_md =
            "---\nstatus: draft\n---\n# My Spec\n\n**FR-001** (Ubiquitous) The system shall work.";
        let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|---|---|---|---|---|---|\n| T-001 | Task 1 | FR-001 | S | Critical | --- |";
        let p =
            SpecCommand::build_add_prompt("myspec", "Add new feature", spec_md, plan_md, 2, 1, 2);
        assert!(p.contains("Add new feature"));
        assert!(p.contains("myspec"));
        assert!(p.contains("FR-001"));
        assert!(p.contains("FR-002"));
        assert!(p.contains("NFR-001"));
        assert!(p.contains("T-002"));
        assert!(p.contains("---NEW REQUIREMENTS---"));
        assert!(p.contains("---NEW TASKS---"));
        assert!(p.contains("---NEW TASK DETAILS---"));
        assert!(p.contains("---END---"));
        assert!(p.contains("NOT rewrite"));
    }

    #[test]
    fn add_completion_summary_shows_ids() {
        let s = SpecCommand::build_add_completion_summary(
            "my-spec",
            &["FR-002".to_string(), "FR-003".to_string()],
            &["T-002".to_string()],
        );
        assert!(s.contains("my-spec"));
        assert!(s.contains("FR-002, FR-003"));
        assert!(s.contains("T-002"));
        assert!(s.contains('2'));
    }

    #[test]
    fn add_completion_summary_empty_ids() {
        let s = SpecCommand::build_add_completion_summary("my-spec", &[], &[]);
        assert!(s.contains("none"));
    }
}
