use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

use super::{Tool, ToolContext, ToolOutput};

pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Recursively searches directories."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Base directory (default: working directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let pattern = input["pattern"]
            .as_str()
            .context("Missing 'pattern' parameter")?;
        let base_dir = input["path"]
            .as_str()
            .map(|p| resolve_path(&ctx.working_dir, p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let glob = globset::GlobBuilder::new(pattern)
            .case_insensitive(false)
            .build()
            .with_context(|| format!("Invalid glob pattern: {}", pattern))?;
        let matcher = glob.compile_matcher();

        let mut matches = Vec::new();
        const MAX_MATCHES: usize = 1000;

        collect_matches(&base_dir, &base_dir, &matcher, &mut matches, MAX_MATCHES)?;

        matches.sort();

        if matches.is_empty() {
            Ok(ToolOutput {
                content: format!("No files matching '{}' in {}", pattern, base_dir.display()),
                metadata: None,
            })
        } else {
            let truncated = matches.len() >= MAX_MATCHES;
            let content = matches.join("\n");
            Ok(ToolOutput {
                content: format!(
                    "{} file{} found{}\n\n{}",
                    matches.len(),
                    if matches.len() == 1 { "" } else { "s" },
                    if truncated { " (truncated)" } else { "" },
                    content
                ),
                metadata: Some(json!({
                    "count": matches.len(),
                    "truncated": truncated,
                })),
            })
        }
    }
}

fn collect_matches(
    root: &PathBuf,
    dir: &PathBuf,
    matcher: &globset::GlobMatcher,
    matches: &mut Vec<String>,
    max: usize,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        if matches.len() >= max {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        // Skip hidden entries
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }

        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                dir_name,
                "node_modules" | "target" | "__pycache__" | "dist" | "build"
            ) {
                continue;
            }
            collect_matches(root, &path, matcher, matches, max)?;
        } else {
            // Match relative path against glob
            if let Ok(rel) = path.strip_prefix(root)
                && matcher.is_match(rel)
            {
                matches.push(rel.display().to_string());
            }
        }
    }
    Ok(())
}

fn resolve_path(working_dir: &PathBuf, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
