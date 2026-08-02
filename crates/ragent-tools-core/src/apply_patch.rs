//! Codex-style patch tool (`apply_patch`).
//!
//! Parses `*** Begin Patch` / `*** End Patch` envelopes with `*** Add File:`,
//! `*** Delete File:`, and `*** Update File:` operations. Update operations
//! contain hunks introduced by `@@` with ` ` (context), `+` (add), and `-`
//! (remove) lines. All operations are validated before any file is written.
//!
//! This tool complements the existing unified-diff `patch` tool by supporting
//! the patch dialect emitted by OpenAI Codex agents.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::path_util::resolve_path;
use super::replace::find_replacement_range;
use super::{Tool, ToolContext, ToolOutput};

/// Applies a Codex-style patch to one or more files.
pub struct ApplyPatchTool;

#[async_trait::async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "apply_patch"
    }

    fn description(&self) -> &'static str {
        "Apply a Codex-style patch to one or more files. The patch must be wrapped \
         in `*** Begin Patch` / `*** End Patch` and contain `*** Add File:`, \
         `*** Delete File:`, or `*** Update File:` operations. Update operations \
         use `@@` hunks with ` ` (context), `+` (add), and `-` (remove) lines. \
         All operations are validated before any file is written."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Codex-style patch content to apply"
                },
                "path": {
                    "type": "string",
                    "description": "Optional: override the base directory for relative paths (default: working directory)"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "When true, validate the patch without writing any files (default: false)"
                }
            },
            "required": ["patch"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let patch_str = input["patch"]
            .as_str()
            .context("Missing required 'patch' parameter")?;
        let path_override = input["path"].as_str();
        let dry_run = input["dry_run"].as_bool().unwrap_or(false);

        let base = if let Some(p) = path_override {
            resolve_path(&ctx.working_dir, p)
        } else {
            ctx.working_dir.clone()
        };

        let ops = parse_codex_patch(patch_str)?;
        if ops.is_empty() {
            bail!(
                "No valid patch operations found. Ensure the patch is wrapped in \
                 `*** Begin Patch` / `*** End Patch` and includes an operation header."
            );
        }

        // Resolve all paths and validate root containment up-front.
        let mut ops = ops
            .into_iter()
            .map(|op| op.resolve_paths(&base))
            .collect::<Result<Vec<_>>>()?;

        // Deduplicate: if an earlier op is a pure rename of a file that a later op
        // references by its old name, adjust the later op to use the new name. This
        // lets a single patch both rename a file and apply edits to the new file.
        let mut rename_map: HashMap<PathBuf, PathBuf> = HashMap::new();
        for op in ops.iter_mut() {
            if let Some(new_name) = rename_map.get(&op.path).cloned() {
                op.path = new_name;
            }
            if let Some(move_to) = op.move_to.clone() {
                rename_map.insert(op.path.clone(), move_to);
            }
        }
        // Phase 1: read all files that exist and validate every operation.
        let mut file_contents: HashMap<PathBuf, String> = HashMap::new();
        let mut results: Vec<OpResult> = Vec::with_capacity(ops.len());
        for op in &ops {
            match &op.kind {
                OpKind::Add { content } => {
                    if op.path.exists() && !dry_run {
                        bail!("Add operation targets existing file: {}", op.path.display());
                    }
                    results.push(OpResult {
                        path: op.path.clone(),
                        kind: OpKindResult::Add {
                            lines: content.lines().count(),
                        },
                    });
                }
                OpKind::Delete => {
                    if !op.path.exists() {
                        bail!(
                            "Delete operation targets missing file: {}",
                            op.path.display()
                        );
                    }
                    let content = tokio::fs::read_to_string(&op.path)
                        .await
                        .with_context(|| format!("Failed to read file: {}", op.path.display()))?;
                    file_contents.insert(op.path.clone(), content);
                    results.push(OpResult {
                        path: op.path.clone(),
                        kind: OpKindResult::Delete {
                            lines: file_contents[&op.path].lines().count(),
                        },
                    });
                }
                OpKind::Update { hunks } => {
                    let content = if op.path.exists() {
                        tokio::fs::read_to_string(&op.path).await.with_context(|| {
                            format!("Failed to read file: {}", op.path.display())
                        })?
                    } else {
                        String::new()
                    };
                    let new_content = apply_update_hunks(&content, hunks, &op.path)?;
                    let old_lines = content.lines().count();
                    let new_lines = new_content.lines().count();
                    file_contents.insert(op.path.clone(), new_content);
                    results.push(OpResult {
                        path: op.path.clone(),
                        kind: OpKindResult::Update {
                            hunks: hunks.len(),
                            old_lines,
                            new_lines,
                        },
                    });
                }
            }
        }

        if dry_run {
            return Ok(build_output(results, true));
        }

        // Phase 2: write all changes.
        for op in &ops {
            let move_to = op.move_to.clone();
            match &op.kind {
                OpKind::Add { content } => {
                    if let Some(parent) = op.path.parent() {
                        tokio::fs::create_dir_all(parent).await.with_context(|| {
                            format!("Failed to create directory: {}", parent.display())
                        })?;
                    }
                    tokio::fs::write(&op.path, content)
                        .await
                        .with_context(|| format!("Failed to write file: {}", op.path.display()))?;
                }
                OpKind::Delete => {
                    tokio::fs::remove_file(&op.path)
                        .await
                        .with_context(|| format!("Failed to delete file: {}", op.path.display()))?;
                }
                OpKind::Update { hunks } => {
                    // Only write if there is no move, or if there are hunks.
                    // A bare update+move means the file is just renamed.
                    if !hunks.is_empty() {
                        let content = file_contents
                            .get(&op.path)
                            .expect("validated update must have content");
                        if let Some(parent) = op.path.parent() {
                            tokio::fs::create_dir_all(parent).await.with_context(|| {
                                format!("Failed to create directory: {}", parent.display())
                            })?;
                        }
                        tokio::fs::write(&op.path, content).await.with_context(|| {
                            format!("Failed to write file: {}", op.path.display())
                        })?;
                    }
                }
            }

            if let Some(move_to) = move_to {
                if let Some(parent) = move_to.parent() {
                    tokio::fs::create_dir_all(parent).await.with_context(|| {
                        format!("Failed to create directory: {}", parent.display())
                    })?;
                }
                tokio::fs::rename(&op.path, &move_to)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to move file from {} to {}",
                            op.path.display(),
                            move_to.display()
                        )
                    })?;
            }
        }

        Ok(build_output(results, false))
    }
}

#[derive(Debug)]
struct PatchOp {
    kind: OpKind,
    path: PathBuf,
    move_to: Option<PathBuf>,
}

#[derive(Debug)]
enum OpKind {
    Add { content: String },
    Delete,
    Update { hunks: Vec<Hunk> },
}

#[derive(Debug)]
struct Hunk {
    /// Unified-diff hunk header line (e.g. `@@ -1,5 +1,5 @@`). Currently
    /// retained for diagnostics and future diff-format round-tripping.
    #[allow(dead_code)]
    header: String,
    lines: Vec<HunkLine>,
    /// `true` when the hunk body included the end-of-file newline marker.
    /// Used by future newline-preservation logic.
    #[allow(dead_code)]
    end_of_file: bool,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

impl PatchOp {
    fn resolve_paths(self, base: &Path) -> Result<Self> {
        let path = resolve_path(base, &self.path.to_string_lossy());
        let move_to = self
            .move_to
            .map(|p| resolve_path(base, &p.to_string_lossy()));
        // Validate canonical containment after resolution to prevent escaping
        // via parent-directory traversal before any file operation runs.
        super::check_path_within_root(&path, base)?;
        if let Some(ref mt) = move_to {
            super::check_path_within_root(mt, base)?;
        }
        Ok(Self {
            kind: self.kind,
            path,
            move_to,
        })
    }
}

#[derive(Debug)]
struct OpResult {
    path: PathBuf,
    kind: OpKindResult,
}

#[derive(Debug)]
enum OpKindResult {
    Add {
        lines: usize,
    },
    Delete {
        lines: usize,
    },
    Update {
        hunks: usize,
        old_lines: usize,
        new_lines: usize,
    },
}

fn parse_codex_patch(text: &str) -> Result<Vec<PatchOp>> {
    let mut ops = Vec::new();
    let mut in_patch = false;
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("*** Begin Patch") {
            in_patch = true;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("*** End Patch") {
            in_patch = false;
            continue;
        }
        if !in_patch || trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();
        if lower.starts_with("*** add file:") {
            let path = parse_header_value(trimmed, "*** add file:")?;
            let mut content_lines = Vec::new();
            while let Some(peek) = lines.peek() {
                let peek_trim = peek.trim();
                if peek_trim.eq_ignore_ascii_case("*** End Patch") || peek_trim.starts_with("*** ")
                {
                    break;
                }
                let next = lines.next().unwrap();
                content_lines.push(strip_prefix(next, '+')?);
            }
            ops.push(PatchOp {
                kind: OpKind::Add {
                    content: content_lines.join("\n"),
                },
                path: PathBuf::from(path),
                move_to: None,
            });
        } else if lower.starts_with("*** delete file:") {
            let path = parse_header_value(trimmed, "*** delete file:")?;
            ops.push(PatchOp {
                kind: OpKind::Delete,
                path: PathBuf::from(path),
                move_to: None,
            });
        } else if lower.starts_with("*** update file:") {
            let path = parse_header_value(trimmed, "*** update file:")?;
            let mut move_to: Option<PathBuf> = None;
            let mut hunks: Vec<Hunk> = Vec::new();

            while let Some(peek) = lines.peek() {
                let peek_trim = peek.trim();
                if peek_trim.eq_ignore_ascii_case("*** End Patch") {
                    break;
                }
                if peek_trim.to_lowercase().starts_with("*** move to:") {
                    let next = lines.next().unwrap();
                    let target = parse_header_value(next.trim(), "*** move to:")?;
                    move_to = Some(PathBuf::from(target));
                    continue;
                }
                if peek_trim.starts_with("@@") {
                    hunks.push(parse_hunk(&mut lines)?);
                } else if peek_trim.starts_with("*** ") {
                    // New operation header: stop consuming for this update.
                    break;
                } else {
                    // Skip stray non-hunk lines inside an update section.
                    lines.next();
                }
            }
            ops.push(PatchOp {
                kind: OpKind::Update { hunks },
                path: PathBuf::from(path),
                move_to,
            });
        }
    }

    Ok(ops)
}

fn parse_header_value(line: &str, prefix: &str) -> Result<String> {
    let rest = line[prefix.len()..].trim();
    if rest.is_empty() {
        bail!("Missing path in patch header: {line}");
    }
    if Path::new(rest).is_absolute() {
        bail!("Patch paths must be relative, got absolute path: {rest}");
    }
    Ok(rest.to_string())
}

fn strip_prefix(line: &str, prefix: char) -> Result<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.first() == Some(&prefix) {
        Ok(chars[1..].iter().collect())
    } else {
        bail!("Expected line to start with '{prefix}', got: {line}");
    }
}

fn parse_hunk<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>) -> Result<Hunk> {
    let header_line = lines
        .next()
        .context("Unexpected end of patch at hunk header")?;
    let header = header_line.trim_start_matches('@').trim().to_string();
    let mut hunk_lines = Vec::new();
    let mut end_of_file = false;

    while let Some(peek) = lines.peek() {
        let peek_trim = peek.trim();
        if peek_trim.eq_ignore_ascii_case("*** End Patch")
            || peek_trim.starts_with("*** ")
            || peek_trim.starts_with("@@")
        {
            break;
        }
        if peek_trim.eq_ignore_ascii_case("*** End of File") {
            end_of_file = true;
            lines.next();
            break;
        }
        let next = lines.next().unwrap();
        if next.is_empty() {
            // Treat empty lines as context without prefix if they appear in the
            // middle of a hunk (Codex occasionally omits the leading space).
            hunk_lines.push(HunkLine::Context(String::new()));
            continue;
        }
        match next.chars().next().unwrap() {
            ' ' => hunk_lines.push(HunkLine::Context(next[1..].to_string())),
            '+' => hunk_lines.push(HunkLine::Add(next[1..].to_string())),
            '-' => hunk_lines.push(HunkLine::Remove(next[1..].to_string())),
            _ => bail!("Invalid hunk line prefix: {next}"),
        }
    }

    Ok(Hunk {
        header,
        lines: hunk_lines,
        end_of_file,
    })
}

fn apply_update_hunks(content: &str, hunks: &[Hunk], path: &Path) -> Result<String> {
    let mut result = content.to_string();

    for (i, hunk) in hunks.iter().enumerate() {
        let old_text = hunk_context(hunk);
        let new_text = hunk_replacement(hunk);

        match find_replacement_range(&result, &old_text, &new_text) {
            Ok((start, end, replacement)) => {
                result.replace_range(start..end, &replacement);
            }
            Err(err) => {
                bail!(
                    "Hunk {} in {} could not be applied: {:?}",
                    i + 1,
                    path.display(),
                    err
                );
            }
        }
    }

    Ok(result)
}

fn hunk_context(hunk: &Hunk) -> String {
    hunk.lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) => Some(s.clone()),
            HunkLine::Remove(s) => Some(s.clone()),
            HunkLine::Add(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn hunk_replacement(hunk: &Hunk) -> String {
    hunk.lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) => Some(s.clone()),
            HunkLine::Add(s) => Some(s.clone()),
            HunkLine::Remove(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_output(results: Vec<OpResult>, dry_run: bool) -> ToolOutput {
    let mut total_added = 0usize;
    let mut total_removed = 0usize;
    let mut updates = 0usize;
    let mut adds = 0usize;
    let mut deletes = 0usize;

    let mut per_file: Vec<Value> = Vec::new();
    for r in &results {
        match &r.kind {
            OpKindResult::Add { lines } => {
                adds += 1;
                total_added += lines;
                per_file.push(json!({
                    "path": r.path.to_string_lossy(),
                    "operation": "add",
                    "lines": lines,
                }));
            }
            OpKindResult::Delete { lines } => {
                deletes += 1;
                total_removed += lines;
                per_file.push(json!({
                    "path": r.path.to_string_lossy(),
                    "operation": "delete",
                    "lines": lines,
                }));
            }
            OpKindResult::Update {
                hunks,
                old_lines,
                new_lines,
            } => {
                updates += 1;
                total_added += new_lines.saturating_sub(*old_lines);
                total_removed += old_lines.saturating_sub(*new_lines);
                per_file.push(json!({
                    "path": r.path.to_string_lossy(),
                    "operation": "update",
                    "hunks": hunks,
                    "old_lines": old_lines,
                    "new_lines": new_lines,
                }));
            }
        }
    }

    let summary = format!(
        "{} {} add{}, {} update{}, {} delete{} across {} file{}",
        if dry_run { "Would apply" } else { "Applied" },
        adds,
        if adds == 1 { "" } else { "s" },
        updates,
        if updates == 1 { "" } else { "s" },
        deletes,
        if deletes == 1 { "" } else { "s" },
        per_file.len(),
        if per_file.len() == 1 { "" } else { "s" }
    );

    ToolOutput {
        content: summary,
        metadata: Some(json!({
            "dry_run": dry_run,
            "operations": adds + updates + deletes,
            "added": total_added,
            "removed": total_removed,
            "per_file": per_file,
        })),
    }
}
