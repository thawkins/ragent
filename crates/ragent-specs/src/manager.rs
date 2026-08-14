//! Spec manager: lifecycle transitions, listing, filtering, and search.
//!
//! Provides a high-level async API over the filesystem-based spec store.

use crate::error::SpecError;
use crate::io::SpecIo;
use crate::plan_parser::PlanParser;
use crate::spec::{Spec, SpecId, SpecStatus};
use crate::validate::{SddFlags, detect_clarification_markers};
use std::path::{Path, PathBuf};

// ── Transition graph ──────────────────────────────────────────────────────

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

/// Returns `true` if `from` → `to` is a valid transition.
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

// ── SpecManager ───────────────────────────────────────────────────────────

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

    // ── Discovery ───────────────────────────────────────────────────────────

    /// Discover all specs under the root directory.
    pub async fn discover_specs(&self) -> Result<Vec<Spec>, SpecError> {
        SpecIo::discover_specs(&self.specs_root).await
    }

    // ── Read / Write ────────────────────────────────────────────────────────

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
        let mut spec = spec.clone();
        spec.spec_md = updated_spec_md;
        SpecIo::write_spec(&self.specs_root, &spec).await
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

    // ── Transitions ─────────────────────────────────────────────────────────

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

    // ── Task management ─────────────────────────────────────────────────────

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
            let deps = if task.dependencies.is_empty() {
                "—".to_string()
            } else {
                task.dependencies.join(", ")
            };
            let req = if task.linked_requirements.is_empty() {
                "—".to_string()
            } else {
                task.linked_requirements.join(", ")
            };
            new_lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                task.id,
                task.title,
                req,
                task.effort,
                task.priority,
                task.status.as_str(),
                deps
            ));
        }
        new_lines.extend(
            lines[table_end..]
                .iter()
                .map(std::string::ToString::to_string),
        );
        Ok(new_lines.join("\n"))
    }

    // ── Listing ─────────────────────────────────────────────────────────────

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

    // ── Search ──────────────────────────────────────────────────────────────

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

            let title_match = title_lower.contains(&query_lower);
            let spec_match = spec_lower.contains(&query_lower);
            let plan_match = plan_lower.contains(&query_lower);
            let review_match = review_lower.contains(&query_lower);

            if title_match || spec_match || plan_match || review_match {
                let mut snippets = Vec::new();
                if spec_match {
                    snippets.extend(extract_snippets(&spec.spec_md, &query_lower, 3));
                }
                if plan_match {
                    snippets.extend(extract_snippets(&spec.plan_md, &query_lower, 3));
                }
                if review_match {
                    snippets.extend(extract_snippets(&spec.review_md, &query_lower, 3));
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

// ── Search helpers ──────────────────────────────────────────────���─────────

/// Extract context snippets around query matches.
fn extract_snippets(text: &str, query: &str, max_snippets: usize) -> Vec<String> {
    let text_lower = text.to_lowercase();
    let mut snippets = Vec::new();
    let window = 40usize;

    for (idx, _) in text_lower.match_indices(query) {
        if snippets.len() >= max_snippets {
            break;
        }
        let start = idx.saturating_sub(window);
        let end = (idx + query.len() + window).min(text.len());
        let snippet = &text[start..end];
        let prefix = if start > 0 { "…" } else { "" };
        let suffix = if end < text.len() { "…" } else { "" };
        snippets.push(format!("{}{}{}", prefix, snippet.trim(), suffix));
    }

    snippets
}

// ── Filter / Sort types ───────────────────────────────────────────────────

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
    /// Lifecycle order (Draft → Archived).
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_transitions() {
        assert!(is_valid_transition(SpecStatus::Draft, SpecStatus::InReview));
        assert!(is_valid_transition(
            SpecStatus::InReview,
            SpecStatus::Approved
        ));
        assert!(is_valid_transition(SpecStatus::InReview, SpecStatus::Draft));
        assert!(is_valid_transition(
            SpecStatus::Approved,
            SpecStatus::InProgress
        ));
        assert!(is_valid_transition(
            SpecStatus::InProgress,
            SpecStatus::Implemented
        ));
        assert!(is_valid_transition(
            SpecStatus::Implemented,
            SpecStatus::Verified
        ));
        assert!(is_valid_transition(
            SpecStatus::Verified,
            SpecStatus::Archived
        ));
        assert!(is_valid_transition(SpecStatus::Archived, SpecStatus::Draft));
    }

    #[test]
    fn test_invalid_transitions() {
        // Same status
        assert!(!is_valid_transition(SpecStatus::Draft, SpecStatus::Draft));
        // Skip ahead
        assert!(!is_valid_transition(
            SpecStatus::Draft,
            SpecStatus::Approved
        ));
        assert!(!is_valid_transition(
            SpecStatus::Draft,
            SpecStatus::Implemented
        ));
        // Backwards
        assert!(!is_valid_transition(
            SpecStatus::Approved,
            SpecStatus::Draft
        ));
        assert!(!is_valid_transition(
            SpecStatus::Implemented,
            SpecStatus::InProgress
        ));
    }

    #[test]
    fn test_next_statuses() {
        let next = next_statuses(SpecStatus::Draft);
        assert_eq!(next, vec![SpecStatus::InReview]);

        let next = next_statuses(SpecStatus::InReview);
        assert_eq!(next, vec![SpecStatus::Draft, SpecStatus::Approved]);
    }

    #[test]
    fn test_update_frontmatter() {
        let content = "# Title\n\nBody.\n";
        let audit = vec![(
            1_700_000_000,
            "draft".to_string(),
            "in_review".to_string(),
            "alice".to_string(),
        )];
        let updated = update_frontmatter(content, SpecStatus::InReview, &audit, &[]).unwrap();
        assert!(updated.starts_with("---\n"));
        assert!(updated.contains("status: in_review"));
        assert!(updated.contains("audit:"));
        assert!(updated.contains("alice"));
        assert!(updated.contains("# Title"));
    }

    #[test]
    fn test_update_frontmatter_preserves_existing_body() {
        let content = "# Spec\n\n## Section\n\nText.\n";
        let updated = update_frontmatter(content, SpecStatus::Approved, &[], &[]).unwrap();
        assert!(updated.contains("## Section"));
        assert!(updated.contains("Text."));
    }

    #[test]
    fn test_update_frontmatter_replaces_old_frontmatter() {
        let content = "---\nstatus: draft\n---\n\n# Title\n";
        let updated = update_frontmatter(content, SpecStatus::Approved, &[], &[]).unwrap();
        // Should only have one frontmatter block
        let count = updated.matches("---").count();
        assert_eq!(count, 2, "should have exactly 2 --- markers");
        assert!(updated.contains("status: approved"));
    }

    #[test]
    fn test_extract_snippets() {
        let text = "The quick brown fox jumps over the lazy dog. The quick brown fox.";
        let snippets = extract_snippets(text, "fox", 2);
        assert_eq!(snippets.len(), 2);
        assert!(snippets[0].contains("fox"));
        assert!(snippets[1].contains("fox"));
    }

    #[test]
    fn test_spec_filter_builder() {
        let f = SpecFilter::new()
            .with_status(SpecStatus::Draft)
            .with_id_prefix("test")
            .with_archived()
            .with_sort(SortBy::Id);
        assert_eq!(f.status, Some(SpecStatus::Draft));
        assert_eq!(f.id_prefix, Some("test".to_string()));
        assert!(f.include_archived);
        assert_eq!(f.sort_by, SortBy::Id);
    }

    #[test]
    fn test_spec_filter_defaults() {
        let f = SpecFilter::new();
        assert!(f.status.is_none());
        assert!(f.id_prefix.is_none());
        assert!(!f.include_archived);
        assert_eq!(f.sort_by, SortBy::ModifiedAt);
    }

    #[test]
    fn test_manager_new() {
        let mgr = SpecManager::new("/tmp/specs");
        assert_eq!(mgr.root(), Path::new("/tmp/specs"));
    }

    #[tokio::test]
    async fn test_manager_discover_and_list() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create two specs
        let id1 = SpecId::new("alpha").unwrap();
        let id2 = SpecId::new("beta").unwrap();
        SpecIo::create_spec_dir(root, &id1, "# Alpha\n", "# Plan Alpha\n")
            .await
            .unwrap();
        SpecIo::create_spec_dir(root, &id2, "# Beta\n", "# Plan Beta\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let specs = mgr.discover_specs().await.unwrap();
        assert_eq!(specs.len(), 2);

        // List all (no filter)
        let list = mgr.list_specs(&SpecFilter::new()).await.unwrap();
        assert_eq!(list.len(), 2);

        // Filter by prefix
        let filtered = mgr
            .list_specs(&SpecFilter::new().with_id_prefix("alp"))
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id.as_str(), "alpha");
    }

    #[tokio::test]
    async fn test_manager_search() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("search-test").unwrap();
        let spec_md = "# Search Test\n\nThis spec is about **frogs**.\n";
        let plan_md = "# Plan\n\nWe will study **frogs** in detail.\n";
        SpecIo::create_spec_dir(root, &id, spec_md, plan_md)
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let results = mgr.search_specs("frogs").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].spec.id.as_str(), "search-test");
        assert_eq!(results[0].score, 3); // spec(2) + plan(1)
        assert!(!results[0].snippets.is_empty());
    }

    #[tokio::test]
    async fn test_manager_transition() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("transition-test").unwrap();
        let spec_md = "---\nstatus: draft\n---\n\n# Transition Test\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        assert_eq!(spec.status, SpecStatus::Draft);

        mgr.transition(&mut spec, SpecStatus::InReview, "alice")
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::InReview);
        assert_eq!(spec.audit_trail.len(), 2);

        // Re-read from disk and verify frontmatter updated
        let spec2 = mgr.read_spec(&id).await.unwrap();
        assert_eq!(spec2.status, SpecStatus::InReview);
        assert!(spec2.spec_md.contains("status: in_review"));
        assert!(spec2.spec_md.contains("audit:"));
    }

    #[tokio::test]
    async fn test_manager_invalid_transition() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("invalid-trans").unwrap();
        SpecIo::create_spec_dir(root, &id, "# Invalid\n", "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        let result = mgr
            .transition(&mut spec, SpecStatus::Implemented, "bob")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transition_approved_blocked_by_clarifications() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-block").unwrap();
        let spec_md =
            "---\nstatus: in_review\n---\n\n# Clar Block\n\n[NEEDS CLARIFICATION: what scale?]\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        // Force in_review so transition to approved is graph-valid
        spec.status = SpecStatus::InReview;

        let result = mgr
            .transition(&mut spec, SpecStatus::Approved, "alice")
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, SpecError::UnresolvedClarifications { count } if count == 1),
            "expected UnresolvedClarifications error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_transition_approved_allowed_without_clarifications() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-ok").unwrap();
        let spec_md = "---\nstatus: in_review\n---\n\n# Clar OK\n\nNo markers here.\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        spec.status = SpecStatus::InReview;

        mgr.transition(&mut spec, SpecStatus::Approved, "alice")
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::Approved);
    }

    #[tokio::test]
    async fn test_transition_non_approved_ignores_clarifications() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-non-approved").unwrap();
        let spec_md =
            "---\nstatus: draft\n---\n\n# Non Approved\n\n[NEEDS CLARIFICATION: something?]\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        // Draft -> InReview should succeed even with clarification markers
        mgr.transition(&mut spec, SpecStatus::InReview, "alice")
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::InReview);
    }

    #[tokio::test]
    async fn test_manager_list_sorting() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id1 = SpecId::new("zeta").unwrap();
        let id2 = SpecId::new("alpha").unwrap();
        SpecIo::create_spec_dir(root, &id1, "# Zeta\n", "# Plan\n")
            .await
            .unwrap();
        SpecIo::create_spec_dir(root, &id2, "# Alpha\n", "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let by_id = mgr
            .list_specs(&SpecFilter::new().with_sort(SortBy::Id))
            .await
            .unwrap();
        assert_eq!(by_id[0].id.as_str(), "alpha");
        assert_eq!(by_id[1].id.as_str(), "zeta");

        let by_title = mgr
            .list_specs(&SpecFilter::new().with_sort(SortBy::Title))
            .await
            .unwrap();
        assert_eq!(by_title[0].title, "Alpha");
        assert_eq!(by_title[1].title, "Zeta");
    }

    #[tokio::test]
    async fn test_manager_list_exclude_archived() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id1 = SpecId::new("active").unwrap();
        let id2 = SpecId::new("archived").unwrap();
        SpecIo::create_spec_dir(root, &id1, "# Active\n", "# Plan\n")
            .await
            .unwrap();
        SpecIo::create_spec_dir(
            root,
            &id2,
            "---\nstatus: archived\n---\n\n# Archived\n",
            "# Plan\n",
        )
        .await
        .unwrap();

        let mgr = SpecManager::new(root);
        let default_list = mgr.list_specs(&SpecFilter::new()).await.unwrap();
        assert_eq!(default_list.len(), 1);
        assert_eq!(default_list[0].id.as_str(), "active");

        let with_archived = mgr
            .list_specs(&SpecFilter::new().with_archived())
            .await
            .unwrap();
        assert_eq!(with_archived.len(), 2);
    }

    // ── transition_with_flags tests (T-036, FR-019) ─────────────────────────

    #[tokio::test]
    async fn test_transition_with_flags_disabled_allows_clarifications() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-flag-off").unwrap();
        let spec_md = "---\nstatus: in_review\n---\n\n# Clar Flag Off\n\n[NEEDS CLARIFICATION: what scale?]\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        spec.status = SpecStatus::InReview;

        // With clarification_markers disabled, transition should succeed
        // even though markers are present.
        let flags = crate::validate::SddFlags::all_disabled();
        mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::Approved);
    }

    #[tokio::test]
    async fn test_transition_with_flags_enabled_blocks_clarifications() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-flag-on").unwrap();
        let spec_md =
            "---\nstatus: in_review\n---\n\n# Clar Flag On\n\n[NEEDS CLARIFICATION: what scale?]\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        spec.status = SpecStatus::InReview;

        // With clarification_markers enabled, transition should be blocked.
        let flags = crate::validate::SddFlags {
            clarification_markers: true,
            ..crate::validate::SddFlags::all_disabled()
        };
        let result = mgr
            .transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, SpecError::UnresolvedClarifications { count } if count == 1),
            "expected UnresolvedClarifications error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_transition_backward_compat_blocks_clarifications() {
        // The legacy transition() should still block (all_enabled flags).
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let id = SpecId::new("clar-compat").unwrap();
        let spec_md =
            "---\nstatus: in_review\n---\n\n# Clar Compat\n\n[NEEDS CLARIFICATION: what?]\n";
        SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
            .await
            .unwrap();

        let mgr = SpecManager::new(root);
        let mut spec = mgr.read_spec(&id).await.unwrap();
        spec.status = SpecStatus::InReview;

        let result = mgr
            .transition(&mut spec, SpecStatus::Approved, "alice")
            .await;
        assert!(result.is_err());
    }

    // ── Phase -1 gate transition tests (T-017, FR-008) ──────────────────────

    /// Helper: create a spec dir with a PLAN.md containing the given gate
    /// markdown.
    async fn make_gate_spec(root: &Path, id: &str, plan_md: &str) {
        let spec_id = SpecId::new(id).unwrap();
        let spec_md = "---\nstatus: draft\n---\n\n# Gate Test\n\nSome requirement.\n";
        SpecIo::create_spec_dir(root, &spec_id, spec_md, plan_md)
            .await
            .unwrap();
    }

    const GATES_ALL_CHECKED: &str = "\
# Plan

## Phase -1 Gates

- [x] **Simplicity**: The design is minimal.
- [x] **Anti-Abstraction**: No speculative abstractions.
- [x] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

    const GATES_ONE_UNCHECKED: &str = "\
# Plan

## Phase -1 Gates

- [x] **Simplicity**: The design is minimal.
- [ ] **Anti-Abstraction**: No speculative abstractions.
- [x] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

    const GATES_ALL_UNCHECKED: &str = "\
# Plan

## Phase -1 Gates

- [ ] **Simplicity**: The design is minimal.
- [ ] **Anti-Abstraction**: No speculative abstractions.
- [ ] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

    const GATES_SECTION_MISSING: &str = "\
# Plan

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

    #[tokio::test]
    async fn test_transition_in_progress_blocked_when_gates_unchecked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-unchecked", GATES_ONE_UNCHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-unchecked").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        let flags = SddFlags {
            phase_minus_one_gates: true,
            ..SddFlags::all_disabled()
        };
        let result = mgr
            .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 1 && gates[0] == "Anti-Abstraction"),
            "expected UncheckedPhaseGates with Anti-Abstraction, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_transition_in_progress_blocked_when_all_gates_unchecked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-all-unchecked", GATES_ALL_UNCHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-all-unchecked").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        let flags = SddFlags {
            phase_minus_one_gates: true,
            ..SddFlags::all_disabled()
        };
        let result = mgr
            .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 3),
            "expected 3 unchecked gates, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_transition_in_progress_blocked_when_gate_section_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-missing", GATES_SECTION_MISSING).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-missing").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        let flags = SddFlags {
            phase_minus_one_gates: true,
            ..SddFlags::all_disabled()
        };
        let result = mgr
            .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 3),
            "expected 3 missing gates, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_transition_in_progress_allowed_when_gates_checked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-checked", GATES_ALL_CHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-checked").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        let flags = SddFlags {
            phase_minus_one_gates: true,
            ..SddFlags::all_disabled()
        };
        mgr.transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::InProgress);
    }

    #[tokio::test]
    async fn test_transition_in_progress_allowed_when_gate_flag_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-flag-off", GATES_ALL_UNCHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-flag-off").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        // With phase_minus_one_gates disabled, transition should succeed
        // even though all gates are unchecked.
        let flags = SddFlags::all_disabled();
        mgr.transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::InProgress);
    }

    #[tokio::test]
    async fn test_transition_backward_compat_blocks_unchecked_gates() {
        // The legacy transition() uses all_enabled flags, so it should block.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-compat", GATES_ONE_UNCHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-compat").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::Approved;

        let result = mgr
            .transition(&mut spec, SpecStatus::InProgress, "alice")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transition_non_in_progress_not_blocked_by_gates() {
        // Transitions to statuses other than in_progress should not be
        // affected by Phase -1 gate checks.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_gate_spec(root, "gate-other", GATES_ALL_UNCHECKED).await;

        let mgr = SpecManager::new(root);
        let mut spec = mgr
            .read_spec(&SpecId::new("gate-other").unwrap())
            .await
            .unwrap();
        spec.status = SpecStatus::InReview;

        // Transition to Approved should not be blocked by gates (only by
        // clarification markers, which are disabled here).
        let flags = SddFlags {
            phase_minus_one_gates: true,
            ..SddFlags::all_disabled()
        };
        mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
            .await
            .unwrap();
        assert_eq!(spec.status, SpecStatus::Approved);
    }
}
