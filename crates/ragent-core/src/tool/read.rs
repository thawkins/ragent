//! File reading tool.
//!
//! Provides [`ReadTool`], which reads file contents and returns them with
//! line numbers. Supports optional line-range selection for viewing subsets.
//!
//! For large files (>100 lines) without a specified range, returns the first
//! 100 lines plus a section map summarising the file's structure so the agent
//! can request specific sections.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Lines threshold above which we return a summary instead of the full file.
const LARGE_FILE_THRESHOLD: usize = 100;

/// Number of initial lines to include when summarising a large file.
const PREVIEW_LINES: usize = 100;

/// Reads a file's contents and returns them with line numbers prefixed.
///
/// Supports optional `start_line` and `end_line` parameters (1-based, inclusive)
/// for reading a specific range of lines.
///
/// For files larger than [`LARGE_FILE_THRESHOLD`] lines, when no line range is
/// specified the tool returns the first [`PREVIEW_LINES`] lines together with a
/// structural section map so the caller can decide which sections to read next.
pub struct ReadTool;

#[async_trait::async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &'static str {
        "read"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Read file contents. For large files (>100 lines) called without a line range, \
         returns the first 100 lines plus a section map of the file's structure. \
         Use start_line/end_line to read specific sections."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Starting line number (1-based, inclusive)"
                },
                "end_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Ending line number (1-based, inclusive)"
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    /// # Errors
    ///
    /// Returns an error if the `path` parameter is missing, if the path points to
    /// a directory, or if the file cannot be read.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        if path.is_dir() {
            anyhow::bail!(
                "'{}' is a directory, not a file. Use the 'list' tool to view directory contents.",
                path.display()
            );
        }

        let content = tokio::fs::read_to_string(&path).await.with_context(|| {
            format!(
                "Cannot read file '{}': file may not exist or is not accessible",
                path.display()
            )
        })?;

        let start_line = input["start_line"].as_u64().map(|n| n as usize);
        let end_line = input["end_line"].as_u64().map(|n| n as usize);

        if let Some(start) = start_line
            && start == 0
        {
            anyhow::bail!("Invalid 'start_line': must be >= 1");
        }
        if let Some(end) = end_line
            && end == 0
        {
            anyhow::bail!("Invalid 'end_line': must be >= 1");
        }
        if let (Some(start), Some(end)) = (start_line, end_line)
            && start > end
        {
            anyhow::bail!(
                "Invalid line range: start_line ({start}) must be less than or equal to end_line ({end})"
            );
        }

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let (output, actual_start, actual_end, summarised) = match (start_line, end_line) {
            (Some(start), Some(end)) => {
                let start = start.saturating_sub(1).min(lines.len());
                let end = end.min(lines.len());
                let text = format_lines(&lines[start..end], start + 1);
                (text, start + 1, end, false)
            }
            (Some(start), None) => {
                let start = start.saturating_sub(1).min(lines.len());
                let text = format_lines(&lines[start..], start + 1);
                (text, start + 1, lines.len(), false)
            }
            _ if total_lines > LARGE_FILE_THRESHOLD => {
                let preview_end = PREVIEW_LINES.min(total_lines);
                let mut text = format_lines(&lines[..preview_end], 1);

                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let sections = detect_sections(&lines, ext);

                text.push_str("\n\n--- Section Map ---\n");
                text.push_str(&format!(
                    "File has {total_lines} total lines. Showing first {preview_end} lines above.\n"
                ));
                text.push_str("Use start_line/end_line to read specific sections:\n\n");

                if sections.is_empty() {
                    text.push_str(&format!(
                        "  No structural sections detected. Read in chunks using line ranges (1-{total_lines}).\n"
                    ));
                } else {
                    for s in &sections {
                        text.push_str(&format!(
                            "  Lines {:>5}-{:<5}  {}\n",
                            s.start_line, s.end_line, s.label
                        ));
                    }
                }

                (text, 1, preview_end, true)
            }
            _ => {
                let text = format_lines(&lines, 1);
                (text, 1, total_lines, false)
            }
        };

        let lines_read = actual_end.saturating_sub(actual_start - 1);

        let mut meta = serde_json::json!({
            "start_line": actual_start,
            "end_line": actual_end,
            "total_lines": total_lines,
            "lines": lines_read,
        });

        if summarised {
            meta["summarised"] = serde_json::json!(true);
            meta["message"] = serde_json::json!(
                "File is large. Only the first lines and a section map are shown. Use start_line/end_line to read specific sections."
            );
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(meta),
        })
    }
}

/// Format a slice of lines with 1-based line numbers starting at `first_num`.
fn format_lines(lines: &[&str], first_num: usize) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}  {}", first_num + i, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

// ── Section detection ────────────────────────────────────────────────

/// A detected structural section within a file.
struct Section {
    start_line: usize, // 1-based
    end_line: usize,   // 1-based, inclusive
    label: String,
}

/// Detect structural sections in a file based on its extension.
fn detect_sections(lines: &[&str], ext: &str) -> Vec<Section> {
    let markers: Vec<(usize, String)> = match ext {
        "rs" => detect_rust_sections(lines),
        "md" | "mdx" => detect_markdown_sections(lines),
        "py" => detect_python_sections(lines),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => detect_js_sections(lines),
        "toml" => detect_toml_sections(lines),
        "yaml" | "yml" => detect_yaml_sections(lines),
        "ini" | "cfg" => detect_ini_sections(lines),
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" => detect_c_sections(lines),
        "java" | "kt" | "kts" => detect_java_sections(lines),
        "go" => detect_go_sections(lines),
        "rb" => detect_ruby_sections(lines),
        "css" | "scss" | "less" => detect_css_sections(lines),
        _ => Vec::new(),
    };

    markers_to_sections(&markers, lines.len())
}

/// Convert a list of (1-based line, label) markers into contiguous sections.
fn markers_to_sections(markers: &[(usize, String)], total_lines: usize) -> Vec<Section> {
    if markers.is_empty() {
        return Vec::new();
    }

    let mut sections = Vec::with_capacity(markers.len());
    for (i, (start, label)) in markers.iter().enumerate() {
        let end = if i + 1 < markers.len() {
            markers[i + 1].0 - 1
        } else {
            total_lines
        };
        sections.push(Section {
            start_line: *start,
            end_line: end,
            label: label.clone(),
        });
    }
    sections
}

fn detect_rust_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("pub(crate) fn ")
            || trimmed.starts_with("pub(crate) async fn ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("pub struct ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub(crate) struct ")
        {
            let label = extract_until(trimmed, '{')
                .or_else(|| extract_until(trimmed, ';'))
                .unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("pub enum ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("pub(crate) enum ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("impl ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("trait ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ") {
            let label = extract_until(trimmed, '{')
                .or_else(|| extract_until(trimmed, ';'))
                .unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("macro_rules!") {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        }
    }
    markers
}

fn detect_markdown_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            markers.push((i + 1, trimmed.to_string()));
        }
    }
    markers
}

fn detect_python_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let label = extract_until(trimmed, ':').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("class ") {
            let label = extract_until(trimmed, ':').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        }
    }
    markers
}

fn detect_js_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("function ")
            || trimmed.starts_with("async function ")
            || trimmed.starts_with("export function ")
            || trimmed.starts_with("export async function ")
            || trimmed.starts_with("export default function ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("class ")
            || trimmed.starts_with("export class ")
            || trimmed.starts_with("export default class ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("export const ")
            || trimmed.starts_with("export let ")
            || trimmed.starts_with("export type ")
            || trimmed.starts_with("export interface ")
            || trimmed.starts_with("export enum ")
        {
            let label = extract_until(trimmed, '=')
                .or_else(|| extract_until(trimmed, '{'))
                .unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        }
    }
    markers
}

fn detect_toml_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            markers.push((i + 1, trimmed.to_string()));
        }
    }
    markers
}

fn detect_yaml_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        // Top-level keys (no leading whitespace, ends with ':')
        if !line.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !line.starts_with('#')
            && !line.starts_with('-')
        {
            let label = line.trim_end().to_string();
            markers.push((i + 1, label));
        }
    }
    markers
}

fn detect_ini_sections(lines: &[&str]) -> Vec<(usize, String)> {
    detect_toml_sections(lines) // same bracket-header format
}

fn detect_c_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Rough heuristic: lines that look like function definitions
        if !trimmed.starts_with("//")
            && !trimmed.starts_with('*')
            && !trimmed.starts_with('#')
            && !trimmed.is_empty()
            && trimmed.contains('(')
            && (trimmed.ends_with('{') || trimmed.ends_with(')'))
            && !trimmed.starts_with("if ")
            && !trimmed.starts_with("for ")
            && !trimmed.starts_with("while ")
            && !trimmed.starts_with("switch ")
            && !trimmed.starts_with("return ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        } else if trimmed.starts_with("typedef struct")
            || trimmed.starts_with("typedef enum")
            || (trimmed.starts_with("struct ") && trimmed.ends_with('{'))
            || (trimmed.starts_with("enum ") && trimmed.ends_with('{'))
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        }
    }
    markers
}

fn detect_java_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("class ") && trimmed.contains('{') && !trimmed.starts_with("//") {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        } else if trimmed.contains("interface ")
            && trimmed.contains('{')
            && !trimmed.starts_with("//")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        } else if (trimmed.starts_with("public ")
            || trimmed.starts_with("private ")
            || trimmed.starts_with("protected ")
            || trimmed.starts_with("static "))
            && trimmed.contains('(')
            && (trimmed.ends_with('{') || trimmed.ends_with(')'))
            && !trimmed.contains("class ")
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        }
    }
    markers
}

fn detect_go_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("func ") {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        } else if trimmed.starts_with("type ") {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label));
        }
    }
    markers
}

fn detect_ruby_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("module ")
        {
            markers.push((i + 1, trimmed.to_string()));
        }
    }
    markers
}

fn detect_css_sections(lines: &[&str]) -> Vec<(usize, String)> {
    let mut markers = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Top-level selectors (not indented, contain '{')
        if !line.starts_with(' ')
            && !line.starts_with('\t')
            && !trimmed.is_empty()
            && !trimmed.starts_with("/*")
            && !trimmed.starts_with("//")
            && trimmed.contains('{')
        {
            let label = extract_until(trimmed, '{').unwrap_or_else(|| trimmed.to_string());
            markers.push((i + 1, label.trim().to_string()));
        }
    }
    markers
}

/// Extract text up to (but not including) a delimiter character.
fn extract_until(s: &str, delim: char) -> Option<String> {
    s.find(delim).map(|pos| s[..pos].trim().to_string())
}
