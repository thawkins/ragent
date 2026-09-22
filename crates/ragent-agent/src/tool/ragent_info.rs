//! `ragent_info` — Report build and version information about ragent itself.
//!
//! Implements a read-only introspection tool that returns the running ragent
//! version, when the binary was built, the git commit it was built from (when
//! available), and the compiler that produced it. This lets the LLM answer
//! "what version of ragent is running and when was it built?" without running a
//! shell command.

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Compile-time build time, embedded by `build.rs`.
const BUILD_TIME: &str = env!("BUILD_TIME");
/// Best-effort git commit hash, embedded by `build.rs` when available.
const GIT_COMMIT: Option<&str> = option_env!("GIT_COMMIT");
/// Best-effort git commit subject, embedded by `build.rs` when available.
const GIT_COMMIT_SUBJECT: Option<&str> = option_env!("GIT_COMMIT_SUBJECT");
/// Best-effort compiler version, embedded by `build.rs` when available.
const RUSTC_VERSION: Option<&str> = option_env!("RUSTC_VERSION");

/// Read-only tool that reports ragent build and version information.
pub struct RagentInfoTool;

/// Normalized build metadata returned by [`RagentInfoTool`].
#[derive(Debug, Clone, Serialize)]
struct RagentInfo {
    version: String,
    build_time: String,
    commit: Option<String>,
    commit_subject: Option<String>,
    rustc_version: Option<String>,
}

impl RagentInfoTool {
    /// Collect the build metadata for the running ragent binary.
    fn build_info() -> RagentInfo {
        RagentInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: BUILD_TIME.to_string(),
            commit: GIT_COMMIT.map(str::to_string),
            commit_subject: GIT_COMMIT_SUBJECT.map(str::to_string),
            rustc_version: RUSTC_VERSION.map(str::to_string),
        }
    }
}

#[async_trait::async_trait]
impl Tool for RagentInfoTool {
    fn name(&self) -> &'static str {
        "ragent_info"
    }

    fn description(&self) -> &'static str {
        "Report build and version information about the running ragent binary. \
         Returns the ragent version, the build timestamp, the git commit it was \
         built from (when available), and the compiler version. No parameters \
         required. This tool does not access the network or the filesystem and \
         always succeeds."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["text", "json"],
                    "description": "Output format: 'text' (human-readable markdown, default) or 'json' (structured metadata only)"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    /// # Errors
    ///
    /// This tool never returns an error — it always succeeds with build info.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let format = input["format"].as_str().unwrap_or("text");
        let info = Self::build_info();
        let value = serde_json::to_value(&info)?;

        match format {
            "json" => Ok(ToolOutput {
                content: serde_json::to_string_pretty(&value)?,
                metadata: Some(value),
            }),
            _ => Ok(ToolOutput {
                content: render_text(&info),
                metadata: Some(value),
            }),
        }
    }
}

/// Render a human-readable markdown report.
fn render_text(info: &RagentInfo) -> String {
    let mut lines = vec![
        "## ragent Build Information".to_string(),
        String::new(),
        format!("- **Version**: {}", info.version),
        format!("- **Built**: {}", info.build_time),
    ];

    if let Some(commit) = &info.commit {
        match &info.commit_subject {
            Some(subject) => lines.push(format!("- **Commit**: {commit} ({subject})")),
            None => lines.push(format!("- **Commit**: {commit}")),
        }
    } else {
        lines.push("- **Commit**: (unavailable)".to_string());
    }

    match &info.rustc_version {
        Some(rustc) => lines.push(format!("- **Compiler**: {rustc}")),
        None => lines.push("- **Compiler**: (unavailable)".to_string()),
    }

    lines.join("\n")
}
