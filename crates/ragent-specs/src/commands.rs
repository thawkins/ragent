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
   - Columns: ID, Title, Requirement, Effort, Priority, Dependencies.
   - Task IDs as T-001, T-002, etc.
   - Link each task to relevant requirements from `SPEC.md`.
   - Effort values: S, M, L.
   - Priority values: Critical, High, Medium, Low.
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
}
