//! Open/reveal files, folders, and URLs in the desktop environment.
//!
//! Provides [`OpenTool`], a thin cross-platform wrapper around `xdg-open` (Linux),
//! `open` (macOS), and `start` (Windows). It supports opening a target,
//! revealing its parent directory, and validating URL schemes before handing
//! them to the OS handler.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};

/// Opens or reveals files, folders, and URLs using the desktop default handler.
pub struct OpenTool;

#[async_trait::async_trait]
impl Tool for OpenTool {
    fn name(&self) -> &'static str {
        "open"
    }

    fn description(&self) -> &'static str {
        "Open or reveal a file, folder, or URL using the desktop default handler. \
         Required parameter: `target` (string). Optional: `action` (string, one \
         of `open`, `reveal`, `url`; default `open`). On Linux uses `xdg-open`, \
         on macOS uses `open`, and on Windows uses `start`. The `reveal` action \
         opens the item's parent directory. URL schemes are validated against an \
         allowlist before launching. This tool interacts with the desktop \
         environment and may fail in headless or sandboxed contexts."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "REQUIRED. File path, folder path, or URL to open"
                },
                "action": {
                    "type": "string",
                    "enum": ["open", "reveal", "url"],
                    "description": "How to handle the target: open it (default), reveal its parent directory, or validate and open as a URL"
                }
            },
            "required": ["target"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "shell:execute"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let target = input["target"]
            .as_str()
            .context("Missing required 'target' parameter")?;
        let action = input["action"].as_str().unwrap_or("open");

        let (program, args) = build_command(target, action, &ctx.working_dir)?;

        let status = Command::new(&program)
            .args(&args)
            .status()
            .await
            .with_context(|| format!("Failed to launch {program} for target {target}"))?;

        let content = if status.success() {
            format!("Opened '{target}' using {program}")
        } else {
            format!(
                "{program} exited with status {} for target '{target}'",
                status.code().unwrap_or(-1)
            )
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "target": target,
                "action": action,
                "program": program,
                "args": args,
                "success": status.success(),
                "exit_code": status.code()
            })),
        })
    }
}

/// Returns the program name and argument list for the given action.
pub fn build_command(
    target: &str,
    action: &str,
    working_dir: &Path,
) -> Result<(String, Vec<String>)> {
    match action {
        "url" => {
            validate_url_scheme(target)?;
            Ok(platform_open_command(target))
        }
        "reveal" => {
            let path = resolve_target_path(target, working_dir)?;
            let dir = if path.is_file() {
                path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| path.clone())
            } else {
                path
            };
            Ok(platform_open_command(&dir.to_string_lossy()))
        }
        "open" => {
            let path = resolve_target_path(target, working_dir)?;
            Ok(platform_open_command(&path.to_string_lossy()))
        }
        _ => {
            bail!("Unknown action '{action}'. Use 'open', 'reveal', or 'url'")
        }
    }
}

/// Resolve a target path relative to the working directory and validate that it
/// does not escape the project root.
fn resolve_target_path(target: &str, working_dir: &Path) -> Result<std::path::PathBuf> {
    let path = super::path_util::resolve_path(working_dir, target);
    super::check_path_within_root(&path, working_dir)?;
    Ok(path)
}

/// Validate that a URL uses an allowed scheme.
fn validate_url_scheme(url: &str) -> Result<()> {
    const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto", "file"];
    let Some((scheme, _)) = url.split_once("://") else {
        bail!("Invalid URL: missing '://' separator");
    };
    let scheme = scheme.to_lowercase();
    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        bail!(
            "URL scheme '{}' is not allowed. Allowed schemes: {}",
            scheme,
            ALLOWED_SCHEMES.join(", ")
        );
    }
    Ok(())
}

/// Return the platform-specific open command and arguments.
#[must_use]
fn platform_open_command(target: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        (
            "cmd".to_string(),
            vec![
                "/c".to_string(),
                "start".to_string(),
                "".to_string(),
                target.to_string(),
            ],
        )
    } else if cfg!(target_os = "macos") {
        ("open".to_string(), vec![target.to_string()])
    } else {
        ("xdg-open".to_string(), vec![target.to_string()])
    }
}
