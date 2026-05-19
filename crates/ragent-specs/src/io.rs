//! File I/O operations for spec management.
//!
//! Provides atomic writes, spec discovery, and safe directory creation.

use crate::error::SpecError;
use crate::spec::{Spec, SpecId, SpecStatus};
use std::path::{Path, PathBuf};
use tokio::fs;

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
        if dir.exists() {
            return Err(SpecError::AlreadyExists(id.to_string()));
        }
        fs::create_dir_all(&dir).await?;
        Self::atomic_write(dir.join("SPEC.md"), spec_md).await?;
        Self::atomic_write(dir.join("PLAN.md"), plan_md).await?;
        Ok(dir)
    }

    /// Write content to a file atomically using a temporary file and rename.
    ///
    /// This ensures readers never see a partially-written file.
    pub async fn atomic_write(path: impl AsRef<Path>, content: &str) -> Result<(), SpecError> {
        let path = path.as_ref();
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, content).await?;
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
            let spec_md = Self::read_file(&spec_md_path).await?;
            let plan_md = Self::read_file(&plan_md_path).await?;
            let review_md_path = path.join("REVIEW.md");
            let review_md = if review_md_path.is_file() {
                Self::read_file(&review_md_path).await.unwrap_or_default()
            } else {
                String::new()
            };
            let modified_at = Self::modified_time(&spec_md_path).await?;
            // Parse title from first H1 in SPEC.md
            let title = Self::extract_title(&spec_md);
            // Parse status from frontmatter or default to Draft
            let status = Self::extract_status(&spec_md).unwrap_or(SpecStatus::Draft);
            // Parse reviewers from frontmatter
            let reviewers = Self::extract_reviewers(&spec_md);
            let mut spec = Spec::new(id, title);
            spec.status = status;
            spec.spec_md = spec_md;
            spec.tasks = Self::parse_tasks(&plan_md);
            spec.plan_md = plan_md;
            spec.review_md = review_md;
            spec.reviewers = reviewers;
            spec.modified_at = modified_at;
            spec.path = Some(path);
            specs.push(spec);
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
        let spec_md = Self::read_file(&spec_md_path).await?;
        let plan_md = Self::read_file(&plan_md_path).await?;
        let review_md_path = dir.join("REVIEW.md");
        let review_md = if review_md_path.is_file() {
            Self::read_file(&review_md_path).await.unwrap_or_default()
        } else {
            String::new()
        };
        let modified_at = Self::modified_time(&spec_md_path).await?;
        let title = Self::extract_title(&spec_md);
        let status = Self::extract_status(&spec_md).unwrap_or(SpecStatus::Draft);
        let reviewers = Self::extract_reviewers(&spec_md);
        let mut spec = Spec::new(id.clone(), title);
        spec.status = status;
        spec.spec_md = spec_md;
        spec.tasks = Self::parse_tasks(&plan_md);
        spec.plan_md = plan_md;
        spec.review_md = review_md;
        spec.reviewers = reviewers;
        spec.modified_at = modified_at;
        spec.path = Some(dir);
        Ok(spec)
    }

    /// Write a `Spec` back to disk (SPEC.md, PLAN.md, and optionally REVIEW.md).
    pub async fn write_spec(specs_root: &Path, spec: &Spec) -> Result<(), SpecError> {
        let dir = spec.dir_path(specs_root);
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        Self::atomic_write(dir.join("SPEC.md"), &spec.spec_md).await?;
        Self::atomic_write(dir.join("PLAN.md"), &spec.plan_md).await?;
        if !spec.review_md.is_empty() {
            Self::atomic_write(dir.join("REVIEW.md"), &spec.review_md).await?;
        }
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
    fn extract_title(content: &str) -> String {
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
    fn extract_status(content: &str) -> Option<SpecStatus> {
        if !content.starts_with("---") {
            return None;
        }
        let end = content[3..].find("---")?;
        let frontmatter = &content[3..3 + end];
        for line in frontmatter.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("status:") {
                return SpecStatus::parse(val.trim());
            }
        }
        None
    }

    /// Extract reviewers from YAML frontmatter.
    /// Looks for `reviewers: [list]` or `reviewers:\n  - name` format.
    fn extract_reviewers(content: &str) -> Vec<String> {
        if !content.starts_with("---") {
            return vec![];
        }
        let Some(end) = content[3..].find("---") else {
            return vec![];
        };
        let frontmatter = &content[3..3 + end];
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
    /// Extracts task table rows with columns:
    /// ID, Title, Requirement, Effort, Priority, Dependencies.
    fn parse_tasks(plan_md: &str) -> Vec<crate::spec::Task> {
        let mut tasks = Vec::new();
        let mut in_task_section = false;
        for line in plan_md.lines() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("## Tasks") || trimmed.eq_ignore_ascii_case("### Tasks")
            {
                in_task_section = true;
                continue;
            }
            if in_task_section && trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
                break;
            }
            if !in_task_section || (trimmed.starts_with("|") && trimmed.contains("ID")) {
                continue;
            }
            // Parse table rows: | ID | Title | Req | Effort | Priority | Status | Dependencies |
            let cells: Vec<&str> = trimmed
                .split('|')
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .collect();
            if cells.len() >= 6 {
                let id = cells[0].to_string();
                if id.starts_with("T-") {
                    let title = cells.get(1).copied().unwrap_or("").to_string();
                    let req = cells.get(2).copied().unwrap_or("").to_string();
                    let effort = cells.get(3).copied().unwrap_or("").to_string();
                    let priority = cells.get(4).copied().unwrap_or("").to_string();
                    // Status is column 5 if 7+ columns, otherwise fallback to Pending
                    let status_str = cells.get(5).copied().unwrap_or("");
                    let status = if cells.len() >= 7 {
                        crate::spec::TaskStatus::parse(status_str)
                            .unwrap_or(crate::spec::TaskStatus::Pending)
                    } else {
                        crate::spec::TaskStatus::Pending
                    };
                    let deps = cells
                        .get(if cells.len() >= 7 { 6 } else { 5 })
                        .map(|d| {
                            if *d == "—" || *d == "-" || d.is_empty() {
                                Vec::new()
                            } else {
                                d.split(',').map(|s| s.trim().to_string()).collect()
                            }
                        })
                        .unwrap_or_default();
                    tasks.push(crate::spec::Task {
                        id: id.clone(),
                        title,
                        description: String::new(),
                        linked_requirements: if req.is_empty() || req == "—" || req == "-" {
                            Vec::new()
                        } else {
                            vec![req]
                        },
                        status,
                        effort,
                        priority,
                        dependencies: deps,
                        completed_at: if status == crate::spec::TaskStatus::Completed {
                            Some(1) // Round-tripped from file; actual timestamp not preserved
                        } else {
                            None
                        },
                    });
                }
            }
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_spec_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let id = SpecId::new("my-spec").unwrap();
        let spec_md = "# Test Spec\n";
        let plan_md = "# Plan\n";
        let dir = SpecIo::create_spec_dir(tmp.path(), &id, spec_md, plan_md)
            .await
            .unwrap();
        assert!(dir.is_dir());
        assert!(dir.join("SPEC.md").is_file());
        assert!(dir.join("PLAN.md").is_file());
        assert_eq!(
            fs::read_to_string(dir.join("SPEC.md")).await.unwrap(),
            spec_md
        );
    }

    #[tokio::test]
    async fn test_create_spec_dir_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let id = SpecId::new("my-spec").unwrap();
        fs::create_dir(tmp.path().join("my-spec")).await.unwrap();
        let result = SpecIo::create_spec_dir(tmp.path(), &id, "x", "y").await;
        assert!(matches!(result, Err(SpecError::AlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_atomic_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        SpecIo::atomic_write(&path, "hello world").await.unwrap();
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "hello world");
        // No temp file should remain
        assert!(!tmp.path().join("test.txt.tmp").exists());
    }

    #[tokio::test]
    async fn test_read_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("data.md");
        fs::write(&path, "content").await.unwrap();
        let data = SpecIo::read_file(&path).await.unwrap();
        assert_eq!(data, "content");
    }

    #[tokio::test]
    async fn test_spec_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let id = SpecId::new("exists").unwrap();
        assert!(!SpecIo::spec_exists(tmp.path(), &id).await);
        fs::create_dir(tmp.path().join("exists")).await.unwrap();
        assert!(SpecIo::spec_exists(tmp.path(), &id).await);
    }

    #[tokio::test]
    async fn test_discover_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // Create two valid specs and one invalid directory
        let id1 = SpecId::new("spec-one").unwrap();
        let id2 = SpecId::new("spec-two").unwrap();
        SpecIo::create_spec_dir(root, &id1, "# Spec One\n", "# Plan One\n")
            .await
            .unwrap();
        SpecIo::create_spec_dir(root, &id2, "# Spec Two\n", "# Plan Two\n")
            .await
            .unwrap();
        fs::create_dir(root.join("no-files")).await.unwrap();

        let specs = SpecIo::discover_specs(root).await.unwrap();
        assert_eq!(specs.len(), 2);
        let titles: Vec<_> = specs.iter().map(|s| s.title.clone()).collect();
        assert!(titles.contains(&"Spec One".to_string()));
        assert!(titles.contains(&"Spec Two".to_string()));
    }

    #[tokio::test]
    async fn test_read_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let id = SpecId::new("read-test").unwrap();
        SpecIo::create_spec_dir(root, &id, "# Read Test\n", "# Plan\n")
            .await
            .unwrap();
        let spec = SpecIo::read_spec(root, &id).await.unwrap();
        assert_eq!(spec.id.as_str(), "read-test");
        assert_eq!(spec.title, "Read Test");
        assert!(spec.path.is_some());
    }

    #[tokio::test]
    async fn test_read_spec_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let id = SpecId::new("missing").unwrap();
        let result = SpecIo::read_spec(tmp.path(), &id).await;
        assert!(matches!(result, Err(SpecError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_write_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let id = SpecId::new("write-test").unwrap();
        let mut spec = Spec::new(id.clone(), "Write Test");
        spec.spec_md = "# Updated Spec\n".to_string();
        spec.plan_md = "# Updated Plan\n".to_string();
        SpecIo::write_spec(root, &spec).await.unwrap();

        let spec_path = root.join("write-test/SPEC.md");
        let plan_path = root.join("write-test/PLAN.md");
        assert!(spec_path.is_file());
        assert_eq!(
            fs::read_to_string(spec_path).await.unwrap(),
            "# Updated Spec\n"
        );
        assert_eq!(
            fs::read_to_string(plan_path).await.unwrap(),
            "# Updated Plan\n"
        );
    }

    #[tokio::test]
    async fn test_extract_title() {
        assert_eq!(SpecIo::extract_title("# My Title\n"), "My Title");
        assert_eq!(SpecIo::extract_title("\n# Title Here\n"), "Title Here");
        assert_eq!(SpecIo::extract_title("no header"), "");
    }

    #[tokio::test]
    async fn test_extract_status() {
        let fm = "---\nstatus: approved\n---\n# Title\n";
        assert_eq!(SpecIo::extract_status(fm), Some(SpecStatus::Approved));

        let no_fm = "# Title\n";
        assert_eq!(SpecIo::extract_status(no_fm), None);

        let draft_fm = "---\nstatus: draft\n---\n";
        assert_eq!(SpecIo::extract_status(draft_fm), Some(SpecStatus::Draft));
    }
}
