//! File I/O operations for spec management.
//!
//! Provides atomic writes, spec discovery, and safe directory creation.
//!
//! Repository content is untrusted, so the spec tree is treated as hostile
//! input: a spec directory or a temp file that is a symlink out of `specs_root`
//! is refused before any write (SEC-006, SEC-059).

use crate::error::SpecError;
use crate::spec::{Requirement, Spec, SpecId, SpecStatus};
use crate::validate::{detect_ears_template, parse_requirements};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;

/// Monotonic counter making concurrent atomic-write temp paths unique
/// (two writers racing on the same target file get distinct temp names).
static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Reject `path` when it, or any of its components, is a symlink.
///
/// SEC-006/SEC-059: the `specs/` tree is repository content. A committed
/// symlinked spec directory (`specs/evil -> ~/.config`) or a pre-placed
/// `.SPEC.md.0.tmp` symlink would otherwise redirect a write outside the
/// sandbox. Only the existing prefix is inspected, so a not-yet-created path is
/// accepted (there is no symlink to follow), and the check walks from `path`
/// upward to the filesystem root.
fn reject_symlink_components(path: &Path) -> Result<(), SpecError> {
    let mut current = path.to_path_buf();
    loop {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(SpecError::UnsafePath(format!(
                    "'{}' is a symlink; refusing to write through it",
                    current.display()
                )));
            }
            Ok(_) => {}
            // Not present (ENOENT) or otherwise unreadable: this component
            // cannot be a symlink, and neither can the ones below it (a
            // symlink's target must exist for the component to be traversed).
            Err(_) => return Ok(()),
        }
        match current.parent() {
            Some(parent) if parent.as_os_str().is_empty() => return Ok(()),
            Some(parent) if parent == current => return Ok(()),
            Some(parent) => current = parent.to_path_buf(),
            None => return Ok(()),
        }
    }
}

/// Resolve a spec directory under `specs_root`, refusing a symlinked directory
/// and any spec directory that canonicalises outside the root.
///
/// SEC-006: replaces the "exists() then create_dir_all" pattern, whose `exists()`
/// follows a committed symlink and skips the create, so the write lands in the
/// link target.
fn resolve_spec_dir(specs_root: &Path, dir_name: &str) -> Result<PathBuf, SpecError> {
    if dir_name.is_empty() {
        return Err(SpecError::UnsafePath("spec directory name is empty".into()));
    }
    // `dir_name` is already `..`-free by construction (`SpecId::dir_name`), but
    // defend the invariant here so this helper is safe to reuse.
    let candidate = Path::new(dir_name);
    if candidate.is_absolute()
        || candidate.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(SpecError::UnsafePath(format!(
            "spec directory '{dir_name}' is not a plain relative name"
        )));
    }
    let dir = specs_root.join(dir_name);
    reject_symlink_components(&dir)?;
    if let (Ok(real), Ok(real_root)) = (dir.canonicalize(), specs_root.canonicalize())
        && !real.starts_with(&real_root)
    {
        return Err(SpecError::UnsafePath(format!(
            "spec directory '{}' resolves outside the specs root via a symlink",
            dir.display()
        )));
    }
    Ok(dir)
}

/// Split `content` into its YAML frontmatter and the remaining body.
///
/// The frontmatter must open with a `---` line at the very start of the file
/// and close with a line that is exactly `---`. Returns `None` when there is
/// no well-formed frontmatter block. A `---` inside a value does not close
/// the block.
pub(crate) fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// Returns `true` when a frontmatter line starts a new top-level key.
///
/// Indented lines, `- item` list entries, blank lines and YAML comments
/// (`# ...`) are not key lines, so a comment inside a list does not end it.
pub(crate) fn is_frontmatter_key_line(line: &str) -> bool {
    let trimmed = line.trim();
    !line.starts_with([' ', '\t', '-']) && !trimmed.is_empty() && !trimmed.starts_with('#')
}

/// Read a frontmatter list value given as `key: [a, b]` or as a block of
/// `- item` lines under `key:`. Surrounding quotes are stripped.
fn frontmatter_list(fm: &str, key: &str) -> Vec<String> {
    let strip = |v: &str| v.trim().trim_matches(['"', '\'']).to_string();
    let prefix = format!("{key}:");
    let mut in_list = false;
    let mut items = Vec::new();
    for line in fm.lines() {
        let trimmed = line.trim();
        if is_frontmatter_key_line(line) {
            in_list = false;
            if let Some(val) = trimmed.strip_prefix(&prefix) {
                let val = val.trim();
                if let Some(inner) = val.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    items.extend(inner.split(',').map(strip).filter(|v| !v.is_empty()));
                } else {
                    in_list = val.is_empty();
                }
            }
        } else if in_list && let Some(item) = trimmed.strip_prefix("- ") {
            let item = strip(item);
            if !item.is_empty() {
                items.push(item);
            }
        }
    }
    items
}

/// Read `key`'s value from a flow-map body such as
/// `time: 1, from: "a", to: "b", actor: "c"`. Double-quoted values are
/// unescaped (inverse of `commands::yaml_double_quote`); bare values run to
/// the next comma.
fn inline_map_value(body: &str, key: &str) -> Option<String> {
    let needle = format!("{key}:");
    let mut search_from = 0;
    let start = loop {
        let idx = body[search_from..].find(&needle)? + search_from;
        // Require a field boundary so `to:` does not match inside `foo_to:`.
        let at_boundary = body[..idx]
            .chars()
            .next_back()
            .is_none_or(|c| c == ',' || c.is_whitespace());
        if at_boundary {
            break idx + needle.len();
        }
        search_from = idx + needle.len();
    };
    let value = body[start..].trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = quoted.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    other => out.push(other),
                },
                other => out.push(other),
            }
        }
        None
    } else {
        let end = value.find(',').unwrap_or(value.len());
        Some(value[..end].trim().to_string())
    }
}

/// I/O helper for spec management.
pub struct SpecIo;

impl SpecIo {
    /// Create a spec directory with a SPEC.md and PLAN.md from templates.
    ///
    /// Returns an error if the directory already exists or if file creation fails.
    pub async fn create_spec_dir(
        specs_root: &Path,
        id: &SpecId,
        spec_md: &str,
        plan_md: &str,
    ) -> Result<PathBuf, SpecError> {
        let dir = resolve_spec_dir(specs_root, id.dir_name())?;
        // create_dir (not create_dir_all) avoids the TOCTOU race of an
        // exists() check followed by a recursive create: an existing dir is
        // reported by the AlreadyExists error instead.
        match fs::create_dir(&dir).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(SpecError::AlreadyExists(id.to_string()));
            }
            Err(e) => return Err(e.into()),
        }
        Self::atomic_write(dir.join("SPEC.md"), spec_md).await?;
        Self::atomic_write(dir.join("PLAN.md"), plan_md).await?;
        Ok(dir)
    }

    /// Write content to a file atomically using a temporary file and rename.
    ///
    /// This ensures readers never see a partially-written file.
    pub async fn atomic_write(path: impl AsRef<Path>, content: &str) -> Result<(), SpecError> {
        let path = path.as_ref();
        // SEC-059: refuse to write through a symlinked path component (the
        // target file or a pre-placed temp symlink) before touching the disk.
        reject_symlink_components(path)?;
        // Unique temp name so concurrent writers to the same target never
        // race on the same temp path (a deterministic `.tmp` suffix made two
        // writers rename over each other's temp file).
        let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_name = path
            .file_name()
            .map_or_else(|| "tmp".to_string(), |n| n.to_string_lossy().into_owned());
        let temp_path = path.with_file_name(format!(".{file_name}.{seq}.tmp"));
        // SEC-059: create the temp with `create_new` so a pre-placed symlink at
        // the predictable temp path is refused (O_EXCL|O_NOFOLLOW semantics)
        // rather than followed by a plain `fs::write`.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .await?;
        use tokio::io::AsyncWriteExt;
        if let Err(e) = file.write_all(content.as_bytes()).await {
            drop(file);
            let _ = fs::remove_file(&temp_path).await;
            return Err(e.into());
        }
        // Sync the temp file before the rename so a crash cannot leave a
        // renamed-but-unflushed file behind. The sync failure is propagated:
        // swallowing it would silently void the crash-durability guarantee
        // while still performing the rename (FUNC-025).
        if let Err(e) = file.sync_all().await {
            drop(file);
            let _ = fs::remove_file(&temp_path).await;
            return Err(e.into());
        }
        drop(file);
        fs::rename(&temp_path, path).await?;
        Ok(())
    }

    /// Read a file to a string.
    pub async fn read_file(path: impl AsRef<Path>) -> Result<String, SpecError> {
        Ok(fs::read_to_string(path).await?)
    }

    /// Check whether a spec directory exists.
    pub async fn spec_exists(specs_root: &Path, id: &SpecId) -> bool {
        specs_root.join(id.dir_name()).is_dir()
    }

    /// Discover all specs under the given root directory.
    ///
    /// Each immediate subdirectory is considered a spec if it contains
    /// `SPEC.md`. `PLAN.md` is optional: a spec that has not yet had its
    /// plan generated is still discoverable, with an empty `plan_md`.
    /// Returns a list of `Spec` structs with basic metadata populated.
    pub async fn discover_specs(specs_root: &Path) -> Result<Vec<Spec>, SpecError> {
        let mut specs = Vec::new();
        let mut entries = fs::read_dir(specs_root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let spec_md_path = path.join("SPEC.md");
            if !spec_md_path.is_file() {
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let Some(id) = SpecId::new(dir_name) else {
                continue;
            };
            // One unreadable spec (bad encoding, permissions) must not hide
            // every other spec from `/spec list`, search and the agent tools.
            match Self::load_spec_from_dir(&path, id).await {
                Ok(spec) => specs.push(spec),
                Err(e) => {
                    tracing::warn!("skipping unreadable spec at {}: {e}", path.display());
                }
            }
        }
        Ok(specs)
    }

    /// Read a spec and plan from disk into a fully-populated `Spec`.
    ///
    /// Only `SPEC.md` is required. When `PLAN.md` is absent the returned
    /// `Spec` carries an empty `plan_md` and no tasks.
    pub async fn read_spec(specs_root: &Path, id: &SpecId) -> Result<Spec, SpecError> {
        let dir = specs_root.join(id.dir_name());
        let spec_md_path = dir.join("SPEC.md");
        if !spec_md_path.is_file() {
            return Err(SpecError::NotFound(spec_md_path.display().to_string()));
        }
        Self::load_spec_from_dir(&dir, id.clone()).await
    }

    /// Hydrate a [`Spec`] from an existing spec directory.
    ///
    /// Single implementation shared by [`discover_specs`] and [`read_spec`]
    /// so every `Spec` field is populated in exactly one place.
    async fn load_spec_from_dir(dir: &Path, id: SpecId) -> Result<Spec, SpecError> {
        let spec_md_path = dir.join("SPEC.md");
        let plan_md_path = dir.join("PLAN.md");
        let spec_md = Self::read_file(&spec_md_path).await?;
        // PLAN.md is optional: a SPEC-only spec (plan not yet generated)
        // must still load, yielding an empty plan and no tasks.
        let plan_md = Self::read_optional(&plan_md_path).await;
        let review_md = Self::read_optional(&dir.join("REVIEW.md")).await;
        let feedback_md = Self::read_optional(&dir.join("FEEDBACK.md")).await;
        let modified_at = Self::modified_time(&spec_md_path).await?;
        let title = Self::extract_title(&spec_md);
        let status = Self::extract_status(&spec_md).unwrap_or(SpecStatus::Draft);
        let reviewers = Self::extract_reviewers(&spec_md);
        let research = Self::extract_research(&spec_md);
        let audit_trail = Self::extract_audit(&spec_md);
        let mut spec = Spec::new(id, title);
        spec.status = status;
        // Keep the persisted history; `Spec::new` only seeds a creation entry
        // for specs that have never been transitioned.
        if !audit_trail.is_empty() {
            spec.audit_trail = audit_trail;
        }
        spec.tasks = Self::parse_tasks(&plan_md);
        // Build requirements before moving `spec_md` into the struct so no
        // clone of the full text is needed.
        spec.requirements = Self::build_requirements(&spec_md, &spec.tasks);
        spec.spec_md = spec_md;
        spec.plan_md = plan_md;
        spec.review_md = review_md;
        spec.feedback_md = feedback_md;
        spec.reviewers = reviewers;
        spec.research = research;
        spec.modified_at = modified_at;
        spec.path = Some(dir.to_path_buf());
        Ok(spec)
    }

    /// Read a file, returning an empty string when it does not exist or
    /// cannot be decoded.
    async fn read_optional(path: &Path) -> String {
        match fs::read_to_string(path).await {
            Ok(s) => s,
            Err(_) => String::new(),
        }
    }

    /// Write a `Spec` back to disk (SPEC.md, PLAN.md, and optionally
    /// REVIEW.md and FEEDBACK.md).
    pub async fn write_spec(specs_root: &Path, spec: &Spec) -> Result<(), SpecError> {
        let dir = resolve_spec_dir(specs_root, &spec.id.dir_name())?;
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        Self::atomic_write(dir.join("SPEC.md"), &spec.spec_md).await?;
        Self::write_plan(dir.join("PLAN.md"), &spec.plan_md).await?;
        Self::write_or_clear(dir.join("REVIEW.md"), &spec.review_md).await?;
        Self::write_or_clear(dir.join("FEEDBACK.md"), &spec.feedback_md).await?;
        Ok(())
    }

    /// Write auxiliary spec files (REVIEW.md / FEEDBACK.md) with clear-on-empty
    /// semantics: never creates an empty file, but DOES delete an existing file
    /// when the in-memory content has been cleared, so a stale file cannot
    /// re-populate the field on the next read.
    async fn write_or_clear(path: PathBuf, content: &str) -> Result<(), SpecError> {
        if content.is_empty() {
            if path.exists() {
                fs::remove_file(&path).await?;
            }
            return Ok(());
        }
        Self::atomic_write(path, content).await
    }

    /// Write `PLAN.md`, skipping the write entirely when the in-memory plan is
    /// empty. Unlike [`write_or_clear`](crate::io::SpecIo::write_or_clear) this
    /// never deletes an existing file: `PLAN.md` is optional, but an on-disk
    /// plan is real work that must not be destroyed by an empty in-memory
    /// value (for example a SPEC-only spec read before the plan was authored).
    async fn write_plan(path: PathBuf, content: &str) -> Result<(), SpecError> {
        if content.is_empty() {
            return Ok(());
        }
        Self::atomic_write(path, content).await
    }

    /// Write spec files to disk using individual field values, avoiding the
    /// need to clone an entire [`Spec`] when only `spec_md` has changed.
    ///
    /// Only writes REVIEW.md / FEEDBACK.md if the corresponding content is
    /// non-empty, matching [`write_spec`](crate::io::SpecIo::write_spec) semantics.
    pub async fn write_spec_fields(
        specs_root: &Path,
        spec_id: &SpecId,
        spec_md: &str,
        plan_md: &str,
        review_md: &str,
        feedback_md: &str,
    ) -> Result<(), SpecError> {
        let dir = resolve_spec_dir(specs_root, spec_id.dir_name())?;
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        Self::atomic_write(dir.join("SPEC.md"), spec_md).await?;
        Self::write_plan(dir.join("PLAN.md"), plan_md).await?;
        Self::write_or_clear(dir.join("REVIEW.md"), review_md).await?;
        Self::write_or_clear(dir.join("FEEDBACK.md"), feedback_md).await?;
        Ok(())
    }

    /// Get the last modified time of a file as Unix epoch seconds.
    async fn modified_time(path: &Path) -> Result<u64, SpecError> {
        let meta = fs::metadata(path).await?;
        let mtime = meta.modified()?;
        let dur = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(dur.as_secs())
    }

    /// Extract the title from the first H1 line in markdown.
    pub fn extract_title(content: &str) -> String {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("# ") {
                return rest.trim().to_string();
            }
        }
        String::new()
    }

    /// Extract the status from YAML frontmatter if present.
    /// Looks for a line like `status: draft` in the frontmatter block.
    pub fn extract_status(content: &str) -> Option<SpecStatus> {
        let frontmatter = Self::frontmatter(content)?;
        for line in frontmatter.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("status:") {
                return SpecStatus::parse(val.trim());
            }
        }
        None
    }

    /// Slice the YAML frontmatter block (content between the opening `---`
    /// line and the closing `---` line), excluding both markers.
    fn frontmatter(content: &str) -> Option<&str> {
        split_frontmatter(content).map(|(fm, _)| fm)
    }

    /// Extract research artifact names from YAML frontmatter.
    ///
    /// Supports `research: [a, b]` inline lists and the multi-line
    /// `research:\n  - name` form, matching [`extract_reviewers`].
    fn extract_research(content: &str) -> Vec<String> {
        Self::frontmatter(content).map_or_else(Vec::new, |fm| frontmatter_list(fm, "research"))
    }

    /// Extract reviewers from YAML frontmatter.
    /// Looks for `reviewers: [list]` or `reviewers:\n  - name` format.
    fn extract_reviewers(content: &str) -> Vec<String> {
        Self::frontmatter(content).map_or_else(Vec::new, |fm| frontmatter_list(fm, "reviewers"))
    }

    /// Extract the audit trail written by the manager's frontmatter update:
    /// `audit:` followed by `  - { time: N, from: "a", to: "b", actor: "c" }`
    /// lines. Entries that do not parse are skipped.
    fn extract_audit(content: &str) -> Vec<(u64, String, String, String)> {
        let Some(fm) = Self::frontmatter(content) else {
            return vec![];
        };
        let mut in_audit = false;
        let mut entries = Vec::new();
        for line in fm.lines() {
            if is_frontmatter_key_line(line) {
                in_audit = line.trim() == "audit:";
                continue;
            }
            if !in_audit {
                continue;
            }
            let Some(body) = line
                .trim()
                .strip_prefix("- {")
                .and_then(|b| b.strip_suffix('}'))
            else {
                continue;
            };
            let time = inline_map_value(body, "time").and_then(|t| t.parse::<u64>().ok());
            let from = inline_map_value(body, "from");
            let to = inline_map_value(body, "to");
            let actor = inline_map_value(body, "actor");
            if let (Some(time), Some(from), Some(to), Some(actor)) = (time, from, to, actor) {
                entries.push((time, from, to, actor));
            }
        }
        entries
    }

    /// Parse tasks from PLAN.md content.
    ///
    /// Delegates to the validated [`crate::plan_parser::PlanParser`] and maps
    /// its typed tasks onto [`crate::spec::Task`], so SPEC.md task display uses
    /// the same parser as TASKS.md generation and cannot disagree about what a
    /// row means (FUNC-026). The previous hand-rolled duplicate parser is
    /// removed.
    fn parse_tasks(plan_md: &str) -> Vec<crate::spec::Task> {
        let Ok(plan_tasks) = crate::plan_parser::PlanParser::parse(plan_md) else {
            return Vec::new();
        };
        plan_tasks
            .into_iter()
            .map(|task| crate::spec::Task {
                id: task.id,
                title: task.title,
                description: String::new(),
                linked_requirements: if crate::plan_parser::PlanParser::is_placeholder_cell(
                    &task.requirement,
                ) {
                    Vec::new()
                } else {
                    // The parser preserves the raw cell (which may list several
                    // comma-separated requirement ids).
                    task.requirement
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                },
                status: task.status,
                effort: task.effort.as_str().to_string(),
                priority: task.priority.as_str().to_string(),
                dependencies: task.dependencies,
                completed_at: task.completed_at,
            })
            .collect()
    }

    /// Build the [`Requirement`] list for a spec from its SPEC.md content and parsed tasks.
    ///
    /// A requirement is marked as implemented when it has at least one linked task and
    /// every linked task is completed.
    fn build_requirements(spec_md: &str, tasks: &[crate::spec::Task]) -> Vec<Requirement> {
        let parsed = parse_requirements(spec_md);
        let mut req_to_tasks: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for task in tasks {
            for req_id in &task.linked_requirements {
                req_to_tasks
                    .entry(req_id.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }

        parsed
            .into_iter()
            .map(|pr| {
                let linked_ids = req_to_tasks.get(&pr.id).cloned().unwrap_or_default();
                let all_completed = !linked_ids.is_empty()
                    && linked_ids.iter().all(|tid| {
                        tasks
                            .iter()
                            .any(|t| t.id == *tid && t.status == crate::spec::TaskStatus::Completed)
                    });
                let template = detect_ears_template(&pr.ears_text)
                    .unwrap_or(crate::spec::EarsTemplate::Ubiquitous);
                Requirement {
                    id: pr.id,
                    text: pr.ears_text,
                    template,
                    implemented: all_completed,
                }
            })
            .collect()
    }
}
