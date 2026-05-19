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
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Returns `true` if this is a usage-error variant.
    pub fn is_usage_error(&self) -> bool {
        matches!(
            self,
            Self::Unknown(s) if s == "create"
                || s == "validate"
                || s == "status"
                || s == "task"
                || s == "activate"
                || s == "coverage"
        )
    }

    /// Build the static help message shown by `/spec help`.
    pub fn build_help_message() -> &'static str {
        "From: /spec help\n\
         ## /spec command reference\n\n\
         | Command | Arguments | Description |\n\
         |---|---|---|\n\
         | `/spec help` | none | Show this command reference table. |\n\
         | `/spec create <specname> <feature description>` | required `specname` + `feature description` | Generate `specs/<specname>/SPEC.md` (EARS spec) and `specs/<specname>/PLAN.md` (implementation plan). |\n\
         | `/spec validate [specname]` | optional `specname` | Validate EARS compliance. Without argument, validates all specs. |\n\
         | `/spec list [--status <status>] [--prefix <prefix>]` | optional filters | List all specs with optional filtering by status or ID prefix. |\n\
         | `/spec search <query>` | required `query` | Full-text search across all specs. |\n\
         | `/spec status <spec-id> [<new-status>]` | required `spec-id`, optional `new-status` | Show current status or transition to a new status. |\n\
         | `/spec task <spec-id> [<task-id>] [<new-status>]` | required `spec-id`, optional `task-id` and `new-status` | List tasks, show a task, or update its status. |\n\
         | `/spec activate <spec-id>` | required `spec-id` | Activate a spec for context injection into agent prompts. |\n\
         | `/spec deactivate` | none | Deactivate the currently active spec. |\n\
         | `/spec coverage <spec-id>` | required `spec-id` | Show requirement coverage report. |\n\n\
         Example: `/spec create websocket Add a real-time collaborative editing feature using WebSockets`"
    }

    /// Build the user-facing status string for a create operation.
    pub fn build_create_status(specname: &str) -> String {
        format!(
            "spec: writing specs/{}/SPEC.md + specs/{}/PLAN.md…",
            specname, specname
        )
    }

    /// Build the assistant message shown when a spec generation starts.
    pub fn build_create_message(specname: &str, _feature: &str) -> String {
        format!(
            "From: /spec\n📝 **Generating specification and plan…**\n\n\
             Creating spec directory `specs/{}` with:\n\
             - `specs/{}/SPEC.md` — EARS requirements specification\n\
             - `specs/{}/PLAN.md` — implementation plan with tasks\n\n\
             This may take a few moments.\n\
             ⚠️ **Tip:** After creation, you can validate with `/spec validate {}`.",
            specname, specname, specname, specname
        )
    }

    /// Build the log entry for a create operation.
    pub fn build_create_log(specname: &str, feature: &str) -> String {
        format!(
            "Creating spec '{}' for feature: {} → specs/{}/SPEC.md",
            specname, feature, specname
        )
    }

    /// Build the prompt sent to the explore agent for spec generation.
    pub fn build_create_prompt(specname: &str, feature: &str) -> String {
        format!(
            r#"You are an expert specification writer. Create a specification and implementation plan for the following feature.
    
                   **Feature:** {}
    
                   **Spec ID:** {}
    
                   Write the following files:
    
                   1. `specs/{}/SPEC.md` — A requirements specification using EARS notation:
                      - Use at least one of each EARS template: ubiquitous, event-driven, state-driven, optional, unwanted
                      - Number requirements as FR-001, FR-002, etc.
                      - Include a '## Requirements' section
                      - Start with YAML frontmatter containing `status: draft`
    
                   2. `specs/{}/PLAN.md` — An implementation plan with:
                      - A '## Tasks' section with a markdown table
                      - Columns: ID, Title, Requirement, Effort, Priority, Dependencies
                      - Task IDs as T-001, T-002, etc.
                      - Link each task to relevant requirements
                      - Effort values: S, M, L
                      - Priority values: Critical, High, Medium, Low
    
                   Use the `write` tool to create both files. Ensure the spec is clear, testable, and complete."#,
            feature, specname, specname, specname
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
}
