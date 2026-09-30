//! Spec manager: lifecycle transitions, listing, filtering, and search.
//!
//! Provides a high-level async API over the filesystem-based spec store.

use crate::error::SpecError;
use crate::io::SpecIo;
use crate::plan_parser::PlanParser;
use crate::spec::{Spec, SpecId, SpecStatus};
use crate::validate::{SddFlags, detect_clarification_markers};
use std::path::{Path, PathBuf};

// -- Transition graph --

/// Returns the list of statuses that `from` is allowed to transition to.
const fn allowed_transitions(from: SpecStatus) -> &'static [SpecStatus] {
    match from {
        SpecStatus::Draft => &[SpecStatus::InReview],
        SpecStatus::InReview => &[SpecStatus::Draft, SpecStatus::Approved],
        SpecStatus::Approved => &[SpecStatus::InProgress],
        SpecStatus::InProgress => &[SpecStatus::Implemented],
        SpecStatus::Implemented => &[SpecStatus::Verified],
        SpecStatus::Verified => &[SpecStatus::Archived],
        SpecStatus::Archived => &[SpecStatus::Draft],
    }
}

/// Returns `true` if `from` -> `to` is a valid transition.
#[must_use]
pub fn is_valid_transition(from: SpecStatus, to: SpecStatus) -> bool {
    if from == to {
        return false;
    }
    allowed_transitions(from).contains(&to)
}

/// Returns the list of allowed next statuses for a given status.
#[must_use]
pub fn next_statuses(from: SpecStatus) -> Vec<SpecStatus> {
    allowed_transitions(from).to_vec()
}

// -- SpecManager --

/// High-level manager for the spec directory.
#[derive(Debug, Clone)]
pub struct SpecManager {
    specs_root: PathBuf,
}

impl SpecManager {
    /// Create a new manager rooted at `specs_root`.
    pub fn new(specs_root: impl Into<PathBuf>) -> Self {
        Self {
            specs_root: specs_root.into(),
        }
    }

    /// Get the root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.specs_root
    }

    // -- Discovery --

    /// Discover all specs under the root directory.
    pub async fn discover_specs(&self) -> Result<Vec<Spec>, SpecError> {
        SpecIo::discover_specs(&self.specs_root).await
    }

    // -- Read / Write --

    /// Read a single spec by ID.
    pub async fn read_spec(&self, id: &SpecId) -> Result<Spec, SpecError> {
        SpecIo::read_spec(&self.specs_root, id).await
    }

    /// Write a spec back to disk, updating frontmatter status and audit trail.
    pub async fn write_spec(&self, spec: &Spec) -> Result<(), SpecError> {
        let updated_spec_md = update_frontmatter(
            &spec.spec_md,
            spec.status,
            &spec.audit_trail,
            &spec.reviewers,
        )?;
        SpecIo::write_spec_fields(
            &self.specs_root,
            &spec.id,
            &updated_spec_md,
            &spec.plan_md,
            &spec.review_md,
            &spec.feedback_md,
        )
        .await
    }

    /// Create a new spec directory with SPEC.md and PLAN.md.
    pub async fn create_spec(
        &self,
        id: &SpecId,
        spec_md: &str,
        plan_md: &str,
    ) -> Result<(), SpecError> {
        SpecIo::create_spec_dir(&self.specs_root, id, spec_md, plan_md).await?;
        Ok(())
    }

    /// Delete a spec directory from the workspace.
    pub async fn delete_spec(&self, id: &SpecId) -> Result<(), SpecError> {
        let dir = self.specs_root.join(id.dir_name());
        if !dir.is_dir() {
            return Err(SpecError::NotFound(id.as_str().to_string()));
        }
        tokio::fs::remove_dir_all(&dir).await?;
        Ok(())
    }

    // -- Transitions --

    /// Transition a spec to a new status.
    ///
    /// Validates the transition, updates the in-memory spec, updates frontmatter,
    /// and writes back to disk. All SDD checks run unconditionally (backward
    /// compatible). Use [`SpecManager::transition_with_flags`] to gate SDD
    /// checks via configuration (FR-019).
    pub async fn transition(
        &self,
        spec: &mut Spec,
        new_status: SpecStatus,
        actor: impl Into<String>,
    ) -> Result<(), SpecError> {
        self.transition_with_flags(spec, new_status, actor, &SddFlags::all_enabled())
            .await
    }

    /// Transition a spec to a new status with SDD capability flags gating
    /// pre-transition checks (FR-019).
    ///
    /// When `flags.clarification_markers` is `false`, the FR-003 clarification
    /// gate is skipped, allowing approval even when `[NEEDS CLARIFICATION]`
    /// markers are present. This preserves existing workflows for users who
    /// have not opted in to the clarification-marker capability.
    pub async fn transition_with_flags(
        &self,
        spec: &mut Spec,
        new_status: SpecStatus,
        actor: impl Into<String>,
        flags: &SddFlags,
    ) -> Result<(), SpecError> {
        if !is_valid_transition(spec.status, new_status) {
            return Err(SpecError::InvalidStatusTransition {
                from: spec.status.as_str().to_string(),
                to: new_status.as_str().to_string(),
            });
        }

        // FR-003: Block `approved` transition when unresolved clarification
        // markers remain in SPEC.md (gated by FR-019 flag).
        if new_status == SpecStatus::Approved && flags.clarification_markers {
            let markers = detect_clarification_markers(&spec.spec_md);
            if !markers.is_empty() {
                return Err(SpecError::UnresolvedClarifications {
                    count: markers.len(),
                });
            }
        }

        // FR-008: Block `in_progress` transition when Phase -1 gates are
        // unchecked or missing in PLAN.md (gated by FR-019 flag).
        if new_status == SpecStatus::InProgress && flags.phase_minus_one_gates {
            let gates = PlanParser::parse_phase_minus_one_gates(&spec.plan_md);
            let unchecked: Vec<String> = gates
                .unchecked_required_gates()
                .into_iter()
                .map(String::from)
                .collect();
            if !unchecked.is_empty() {
                return Err(SpecError::UncheckedPhaseGates { gates: unchecked });
            }
        }

        spec.transition(new_status, actor);
        self.write_spec(spec).await
    }

    // -- Task management --

    /// Update a task's status within a spec.
    ///
    /// Finds the task by ID, updates its status, and rewrites the PLAN.md
    /// to reflect the change (replacing the task table row).
    pub async fn update_task_status(
        &self,
        spec: &mut Spec,
        task_id: &str,
        new_status: crate::spec::TaskStatus,
    ) -> Result<(), SpecError> {
        let task = spec
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| SpecError::UnknownId(task_id.to_string()))?;
        task.status = new_status;
        if new_status == crate::spec::TaskStatus::Completed {
            task.completed_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
        } else {
            task.completed_at = None;
        }
        // Rewrite PLAN.md with updated task table
        spec.plan_md = Self::rewrite_plan_tasks(&spec.plan_md, &spec.tasks)?;
        self.write_spec(spec).await
    }

    /// Rewrite the task table in PLAN.md with updated task statuses.
    fn rewrite_plan_tasks(plan_md: &str, tasks: &[crate::spec::Task]) -> Result<String, SpecError> {
        let lines: Vec<&str> = plan_md.lines().collect();
        let mut in_task_section = false;
        let mut table_start = None;
        let mut table_end = None;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("## Tasks") || trimmed.eq_ignore_ascii_case("### Tasks")
            {
                in_task_section = true;
                continue;
            }
            if in_task_section && trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
                table_end = Some(i);
                break;
            }
            if in_task_section && table_start.is_none() && trimmed.starts_with('|') {
                table_start = Some(i);
            }
        }
        let table_start = table_start.unwrap_or(lines.len());
        let table_end = table_end.unwrap_or(lines.len());
        let mut new_lines: Vec<String> = lines[..table_start]
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        new_lines.push(
            "| ID | Title | Requirement | Effort | Priority | Status | Dependencies |".to_string(),
        );
        new_lines.push(
            "|----|-------|-------------|--------|----------|--------|--------------|".to_string(),
        );
        for task in tasks {
            new_lines.push(task.table_row());
        }
        new_lines.extend(
            lines[table_end..]
                .iter()
                .map(std::string::ToString::to_string),
        );
        Ok(new_lines.join("\n"))
    }

    // -- Listing --

    /// List specs with optional filtering and sorting.
    pub async fn list_specs(&self, filter: &SpecFilter) -> Result<Vec<Spec>, SpecError> {
        let mut specs = self.discover_specs().await?;

        // Filter by status
        if let Some(status) = filter.status {
            specs.retain(|s| s.status == status);
        }

        // Filter by ID prefix (case-insensitive)
        if let Some(ref prefix) = filter.id_prefix {
            let lower = prefix.to_lowercase();
            specs.retain(|s| s.id.as_str().to_lowercase().starts_with(&lower));
        }

        // Filter by modified-since
        if let Some(since) = filter.modified_since {
            specs.retain(|s| s.modified_at >= since);
        }

        // Exclude archived unless explicitly requested
        if !filter.include_archived {
            specs.retain(|s| s.status != SpecStatus::Archived);
        }

        // Sort
        match filter.sort_by {
            SortBy::ModifiedAt => {
                specs.sort_by_key(|b| std::cmp::Reverse(b.modified_at));
            }
            SortBy::Status => {
                let order = |s: SpecStatus| match s {
                    SpecStatus::Draft => 0,
                    SpecStatus::InReview => 1,
                    SpecStatus::Approved => 2,
                    SpecStatus::InProgress => 3,
                    SpecStatus::Implemented => 4,
                    SpecStatus::Verified => 5,
                    SpecStatus::Archived => 6,
                };
                specs.sort_by_key(|s| order(s.status));
            }
            SortBy::Id => {
                specs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            }
            SortBy::Title => {
                specs.sort_by(|a, b| a.title.cmp(&b.title));
            }
        }

        Ok(specs)
    }

    // -- Search --

    /// Full-text search across all specs.
    ///
    /// Returns matching specs ordered by relevance (title match > content match).
    /// Archived specs are excluded by default; pass `include_archived: true` to include them.
    pub async fn search_specs(&self, query: &str) -> Result<Vec<SpecSearchResult>, SpecError> {
        self.search_specs_filtered(query, false).await
    }

    /// Full-text search with explicit archive inclusion.
    pub async fn search_specs_with_archived(
        &self,
        query: &str,
    ) -> Result<Vec<SpecSearchResult>, SpecError> {
        self.search_specs_filtered(query, true).await
    }

    /// Common search implementation with archive filtering.
    async fn search_specs_filtered(
        &self,
        query: &str,
        include_archived: bool,
    ) -> Result<Vec<SpecSearchResult>, SpecError> {
        // An empty query matches every byte offset and yields junk snippets;
        // treat it as no results.
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut specs = self.discover_specs().await?;
        let query_lower = query.to_lowercase();

        // Exclude archived unless explicitly requested
        if !include_archived {
            specs.retain(|s| s.status != SpecStatus::Archived);
        }

        let mut results = Vec::new();

        for spec in specs {
            let title_lower = spec.title.to_lowercase();
            let spec_lower = spec.spec_md.to_lowercase();
            let plan_lower = spec.plan_md.to_lowercase();
            let review_lower = spec.review_md.to_lowercase();
            // (The lowered copies are reused by snippet extraction below,
            // avoiding a second full-text lowercasing per matched document.)

            let title_match = title_lower.contains(&query_lower);
            let spec_match = spec_lower.contains(&query_lower);
            let plan_match = plan_lower.contains(&query_lower);
            let review_match = review_lower.contains(&query_lower);

            if title_match || spec_match || plan_match || review_match {
                let mut snippets = Vec::new();
                if spec_match {
                    snippets.extend(extract_snippets_lowered(
                        &spec.spec_md,
                        &spec_lower,
                        &query_lower,
                        3,
                    ));
                }
                if plan_match {
                    snippets.extend(extract_snippets_lowered(
                        &spec.plan_md,
                        &plan_lower,
                        &query_lower,
                        3,
                    ));
                }
                if review_match {
                    snippets.extend(extract_snippets_lowered(
                        &spec.review_md,
                        &review_lower,
                        &query_lower,
                        3,
                    ));
                }

                let score = if title_match { 3 } else { 0 }
                    + if spec_match { 2 } else { 0 }
                    + i32::from(plan_match)
                    + i32::from(review_match);

                results.push(SpecSearchResult {
                    spec,
                    score,
                    snippets,
                });
            }
        }

        results.sort_by_key(|b| std::cmp::Reverse(b.score));
        Ok(results)
    }
}

/// Update the YAML frontmatter of a SPEC.md with the current status and audit trail.
fn update_frontmatter(
    content: &str,
    status: SpecStatus,
    audit_trail: &[(u64, String, String, String)],
    reviewers: &[String],
) -> Result<String, SpecError> {
    let body_start = if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("---") {
            3 + end + 3
        } else {
            0
        }
    } else {
        0
    };

    let body = if body_start > 0 {
        &content[body_start..]
    } else {
        content
    };

    let mut fm_lines = vec!["---".to_string(), format!("status: {}", status.as_str())];

    if !audit_trail.is_empty() {
        fm_lines.push("audit:".to_string());
        for (ts, old, new, actor) in audit_trail {
            fm_lines.push(format!(
                "  - {{ time: {ts}, from: \"{old}\", to: \"{new}\", actor: \"{actor}\" }}"
            ));
        }
    }

    if !reviewers.is_empty() {
        let names: Vec<String> = reviewers.iter().map(|r| format!("\"{r}\"")).collect();
        fm_lines.push(format!("reviewers: [{}]", names.join(", ")));
    }

    fm_lines.push("---".to_string());

    Ok(format!("{}\n{}", fm_lines.join("\n"), body.trim_start()))
}

// -- Search helpers --

/// Extract context snippets around query matches.
///
/// Takes both the original and an already-lowercased copy of the text so
/// callers that have already lowercased (search) do not pay for a second
/// full-text lowercasing per snippet batch.
fn extract_snippets_lowered(
    text: &str,
    text_lower: &str,
    query: &str,
    max_snippets: usize,
) -> Vec<String> {
    let mut snippets = Vec::new();
    let window = 40usize;

    for (idx, _) in text_lower.match_indices(query) {
        if snippets.len() >= max_snippets {
            break;
        }
        let start = idx.saturating_sub(window);
        let end = (idx + query.len() + window).min(text.len());
        // Align byte indices to char boundaries in the original UTF-8 text so slicing
        // does not panic on multi-byte characters (e.g. em-dashes).
        let start_char = text.floor_char_boundary(start);
        let end_char = text.ceil_char_boundary(end);
        let snippet = &text[start_char..end_char];
        let prefix = if start_char > 0 { "..." } else { "" };
        let suffix = if end_char < text.len() { "..." } else { "" };
        snippets.push(format!("{}{}{}", prefix, snippet.trim(), suffix));
    }

    snippets
}

// -- Filter / Sort types --

/// Controls how spec lists are filtered.
#[derive(Debug, Clone, Default)]
pub struct SpecFilter {
    /// Filter by exact status match.
    pub status: Option<SpecStatus>,
    /// Filter by ID prefix (case-insensitive).
    pub id_prefix: Option<String>,
    /// Only include specs modified at or after this Unix timestamp.
    pub modified_since: Option<u64>,
    /// Include archived specs in results.
    pub include_archived: bool,
    /// Sort order.
    pub sort_by: SortBy,
}

impl SpecFilter {
    /// Create a filter with default settings (no filters, sort by modified desc).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Only return specs with this status.
    #[must_use]
    pub const fn with_status(mut self, status: SpecStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Only return specs whose ID starts with this prefix.
    #[must_use]
    pub fn with_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.id_prefix = Some(prefix.into());
        self
    }

    /// Only return specs modified at or after this timestamp.
    #[must_use]
    pub const fn with_modified_since(mut self, since: u64) -> Self {
        self.modified_since = Some(since);
        self
    }

    /// Include archived specs.
    #[must_use]
    pub const fn with_archived(mut self) -> Self {
        self.include_archived = true;
        self
    }

    /// Set sort order.
    #[must_use]
    pub const fn with_sort(mut self, sort: SortBy) -> Self {
        self.sort_by = sort;
        self
    }
}

/// Sort order for spec listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    /// Most recently modified first.
    #[default]
    ModifiedAt,
    /// Lifecycle order (Draft -> Archived).
    Status,
    /// Alphanumeric by spec ID.
    Id,
    /// Alphanumeric by title.
    Title,
}

/// A single search result with relevance score and snippets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSearchResult {
    /// The matching spec.
    pub spec: Spec,
    /// Relevance score (higher = better match).
    pub score: i32,
    /// Context snippets around matches.
    pub snippets: Vec<String>,
}

// -- Tests --

#[cfg(test)]
#[path = "../tests/inline/manager_tests.rs"]
mod tests;
