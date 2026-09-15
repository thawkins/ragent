//! File I/O operations for spec management.
//!
//! Provides atomic writes, spec discovery, and safe directory creation.

use crate::error::SpecError;
use crate::spec::{Requirement, Spec, SpecId, SpecStatus};
use crate::validate::{detect_ears_template, parse_requirements};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;

/// Monotonic counter making concurrent atomic-write temp paths unique
/// (two writers racing on the same target file get distinct temp names).
static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

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
        let dir = specs_root.join(id.dir_name());
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
        // Unique temp name so concurrent writers to the same target never
        // race on the same temp path (a deterministic `.tmp` suffix made two
        // writers rename over each other's temp file).
        let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_name = path
            .file_name()
            .map_or_else(|| "tmp".to_string(), |n| n.to_string_lossy().into_owned());
        let temp_path = path.with_file_name(format!(".{file_name}.{seq}.tmp"));
        fs::write(&temp_path, content).await?;
        // Sync the temp file before the rename so a crash cannot leave a
        // renamed-but-unflushed file behind. The sync failure is propagated:
        // swallowing it would silently void the crash-durability guarantee
        // while still performing the rename (FUNC-025).
        fs::File::open(&temp_path).await?.sync_all().await?;
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
    /// both `SPEC.md` and `PLAN.md`. Returns a list of `Spec` structs
    /// with basic metadata populated.
    pub async fn discover_specs(specs_root: &Path) -> Result<Vec<Spec>, SpecError> {
        let mut specs = Vec::new();
        let mut entries = fs::read_dir(specs_root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let spec_md_path = path.join("SPEC.md");
            let plan_md_path = path.join("PLAN.md");
            if !spec_md_path.is_file() || !plan_md_path.is_file() {
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let Some(id) = SpecId::new(dir_name) else {
                continue;
            };
            specs.push(Self::load_spec_from_dir(&path, id).await?);
        }
        Ok(specs)
    }

    /// Read a spec and plan from disk into a fully-populated `Spec`.
    pub async fn read_spec(specs_root: &Path, id: &SpecId) -> Result<Spec, SpecError> {
        let dir = specs_root.join(id.dir_name());
        let spec_md_path = dir.join("SPEC.md");
        let plan_md_path = dir.join("PLAN.md");
        if !spec_md_path.is_file() {
            return Err(SpecError::NotFound(spec_md_path.display().to_string()));
        }
        if !plan_md_path.is_file() {
            return Err(SpecError::NotFound(plan_md_path.display().to_string()));
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
        let plan_md = Self::read_file(&plan_md_path).await?;
        let review_md = Self::read_optional(&dir.join("REVIEW.md")).await;
        let feedback_md = Self::read_optional(&dir.join("FEEDBACK.md")).await;
        let modified_at = Self::modified_time(&spec_md_path).await?;
        let title = Self::extract_title(&spec_md);
        let status = Self::extract_status(&spec_md).unwrap_or(SpecStatus::Draft);
        let reviewers = Self::extract_reviewers(&spec_md);
        let research = Self::extract_research(&spec_md);
        let mut spec = Spec::new(id, title);
        spec.status = status;
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
        let dir = spec.dir_path(specs_root);
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        Self::atomic_write(dir.join("SPEC.md"), &spec.spec_md).await?;
        Self::atomic_write(dir.join("PLAN.md"), &spec.plan_md).await?;
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

    /// Write spec files to disk using individual field values, avoiding the
    /// need to clone an entire [`Spec`] when only `spec_md` has changed.
    ///
    /// Only writes REVIEW.md / FEEDBACK.md if the corresponding content is
    /// non-empty, matching [`write_spec`] semantics.
    pub async fn write_spec_fields(
        specs_root: &Path,
        spec_id: &SpecId,
        spec_md: &str,
        plan_md: &str,
        review_md: &str,
        feedback_md: &str,
    ) -> Result<(), SpecError> {
        let dir = specs_root.join(spec_id.dir_name());
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        Self::atomic_write(dir.join("SPEC.md"), spec_md).await?;
        Self::atomic_write(dir.join("PLAN.md"), plan_md).await?;
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
    /// line and the closing `---`), excluding both markers.
    fn frontmatter(content: &str) -> Option<&str> {
        let rest = content.strip_prefix("---")?;
        let end = rest.find("---")?;
        Some(&rest[..end])
    }

    /// Extract research artifact names from YAML frontmatter.
    ///
    /// Supports `research: [a, b]` inline lists and the multi-line
    /// `research:\n  - name` form, matching [`extract_reviewers`].
    fn extract_research(content: &str) -> Vec<String> {
        let Some(frontmatter) = Self::frontmatter(content) else {
            return vec![];
        };
        for line in frontmatter.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("research:") {
                let val = val.trim();
                if let Some(rest) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    return rest
                        .split(',')
                        .map(|r| r.trim().trim_matches(&['"', '\''][..]).to_string())
                        .filter(|r| !r.is_empty())
                        .collect();
                }
            }
        }
        vec![]
    }

    /// Extract reviewers from YAML frontmatter.
    /// Looks for `reviewers: [list]` or `reviewers:\n  - name` format.
    fn extract_reviewers(content: &str) -> Vec<String> {
        let Some(frontmatter) = Self::frontmatter(content) else {
            return vec![];
        };
        let mut in_reviewers = false;
        let mut reviewers = Vec::new();
        for line in frontmatter.lines() {
            let trimmed = line.trim();
            // Check for inline list: reviewers: [a, b]
            if let Some(val) = trimmed.strip_prefix("reviewers:") {
                let val = val.trim();
                if let Some(rest) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    for r in rest.split(',') {
                        let name = r.trim().trim_matches(&['"', '\''][..]);
                        if !name.is_empty() {
                            reviewers.push(name.to_string());
                        }
                    }
                }
                // Start of multi-line list
                in_reviewers = true;
            } else if in_reviewers {
                if let Some(name) = trimmed.strip_prefix("- ") {
                    reviewers.push(name.trim().trim_matches(&['"', '\''][..]).to_string());
                } else if !trimmed.is_empty() && !trimmed.starts_with('-') {
                    // No longer in the reviewers list
                    in_reviewers = false;
                }
            }
        }
        reviewers
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
                linked_requirements: if task.requirement.is_empty()
                    || task.requirement == "—"
                    || task.requirement == "-"
                {
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
                completed_at: None, // Round-tripped from file; actual timestamp not preserved
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
