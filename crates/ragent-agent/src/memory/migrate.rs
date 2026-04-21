//! Migration utilities for splitting a flat MEMORY.md into structured blocks.
//!
//! When a project has an existing `MEMORY.md` with structured headings
//! (e.g., `## Patterns`, `## Conventions`), this module can split it into
//! separate named blocks. The migration is opt-in and requires user
//! confirmation before modifying any files.

use super::block::{BlockScope, MemoryBlock};
use super::storage::BlockStorage;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Analyse a flat MEMORY.md and propose a split into named blocks.
///
/// Returns a list of `(proposed_label, content)` pairs derived from the
/// top-level headings in the file. The heading text is slugified to create
/// a valid label.
pub fn analyse_memory_md(content: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_label = String::new();
    let mut current_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            // Flush the previous section (only if it has non-whitespace content).
            if !current_label.is_empty() {
                let body = current_lines.join("\n");
                if !body.trim().is_empty() {
                    sections.push((current_label.clone(), body));
                }
            }
            current_label = slugify_heading(heading);
            current_lines.clear();
        } else if line.starts_with("# ") {
            // Top-level heading — use as the first section if no ## yet.
            if current_label.is_empty() {
                current_label = slugify_heading(line.strip_prefix("# ").unwrap_or(line));
            }
        } else {
            current_lines.push(line.to_string());
        }
    }

    // Flush the last section.
    if !current_label.is_empty() {
        let body = current_lines.join("\n");
        if !body.trim().is_empty() {
            sections.push((current_label.clone(), body));
        }
    }

    // If no headings were found, return the entire content as one block.
    if sections.is_empty() && !content.trim().is_empty() {
        sections.push(("general".to_string(), content.to_string()));
    }

    sections
}

/// Slugify a heading into a valid block label.
///
/// Converts to lowercase, replaces non-alphanumeric characters with hyphens,
/// collapses multiple hyphens, strips leading/trailing hyphens, and ensures
/// the result starts with a letter.
fn slugify_heading(heading: &str) -> String {
    let lower = heading.to_lowercase();
    let slug: String = lower
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse multiple hyphens.
    let mut result = String::new();
    let mut prev_hyphen = false;
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_hyphen {
                result.push(ch);
            }
            prev_hyphen = true;
        } else {
            result.push(ch);
            prev_hyphen = false;
        }
    }

    // Strip leading/trailing hyphens.
    let trimmed = result.trim_matches('-').to_string();

    // Ensure starts with a letter.
    if trimmed.is_empty() {
        return "section".to_string();
    }
    if !trimmed.chars().next().unwrap().is_ascii_lowercase() {
        format!("s-{trimmed}")
    } else {
        trimmed
    }
}

/// Execute the migration: split MEMORY.md into named blocks.
///
/// This is a dry-run by default. Set `execute: true` to actually write files.
/// When `execute` is false, returns a description of what would be done.
///
/// The original MEMORY.md is never deleted — it remains as a backup.
pub fn migrate_memory_md(
    content: &str,
    scope: &BlockScope,
    working_dir: &PathBuf,
    storage: &dyn BlockStorage,
    execute: bool,
) -> Result<MigrationPlan> {
    let sections = analyse_memory_md(content);

    let mut plan = MigrationPlan {
        source_scope: scope.clone(),
        sections: Vec::new(),
        would_create: Vec::new(),
        would_skip: Vec::new(),
    };

    for (label, section_content) in sections {
        // Check if block already exists.
        let existing = storage.load(&label, scope, working_dir)?;
        let section_info = SectionInfo {
            label: label.clone(),
            line_count: section_content.lines().count(),
            byte_count: section_content.len(),
        };

        if existing.is_some() {
            plan.would_skip.push(label.clone());
        } else {
            plan.would_create.push(label.clone());
        }

        plan.sections
            .push((section_info, section_content, existing.is_some()));
    }

    if execute {
        for (info, section_content, skip) in &plan.sections {
            if *skip {
                continue;
            }
            let block =
                MemoryBlock::new(&info.label, scope.clone()).with_content(section_content.clone());
            storage.save(&block, working_dir).with_context(|| {
                format!("Failed to create block '{}' during migration", info.label)
            })?;
        }
    }

    Ok(plan)
}

/// Result of a migration analysis.
#[derive(Debug)]
pub struct MigrationPlan {
    /// Scope of the source MEMORY.md.
    pub source_scope: BlockScope,
    /// Sections found in the source file.
    pub sections: Vec<(SectionInfo, String, bool)>,
    /// Labels that would be created (no existing block).
    pub would_create: Vec<String>,
    /// Labels that would be skipped (existing block found).
    pub would_skip: Vec<String>,
}

impl MigrationPlan {
    /// Returns a human-readable summary of the migration plan.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Migration plan for {} scope:\n",
            self.source_scope
        ));
        if self.would_create.is_empty() && self.would_skip.is_empty() {
            out.push_str("  No sections found to migrate.\n");
            return out;
        }
        if !self.would_create.is_empty() {
            out.push_str(&format!(
                "  Would create: {}\n",
                self.would_create.join(", ")
            ));
        }
        if !self.would_skip.is_empty() {
            out.push_str(&format!(
                "  Would skip (already exist): {}\n",
                self.would_skip.join(", ")
            ));
        }
        out
    }
}

/// Metadata about a section extracted from MEMORY.md.
#[derive(Debug)]
pub struct SectionInfo {
    /// Proposed block label (slugified from heading).
    pub label: String,
    /// Number of content lines in this section.
    pub line_count: usize,
    /// Byte count of the section content.
    pub byte_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::storage::FileBlockStorage;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::Builder::new()
            .prefix("ragent-mem-migrate-")
            .tempdir()
            .expect("create temp dir")
    }

    #[test]
    fn test_slugify_heading() {
        assert_eq!(slugify_heading("Patterns"), "patterns");
        assert_eq!(slugify_heading("Error Handling"), "error-handling");
        assert_eq!(
            slugify_heading("Code Style & Conventions"),
            "code-style-conventions"
        );
        assert_eq!(slugify_heading("1. Getting Started"), "s-1-getting-started");
    }

    #[test]
    fn test_analyse_memory_md_with_headings() {
        let content = "# Project Memory\n\n## Patterns\n\nUse Result<T, E>.\n\n## Conventions\n\n4 spaces indent.\n";
        let sections = analyse_memory_md(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "patterns");
        assert!(sections[0].1.contains("Result"));
        assert_eq!(sections[1].0, "conventions");
        assert!(sections[1].1.contains("4 spaces"));
    }

    #[test]
    fn test_analyse_memory_md_no_headings() {
        let content = "Just some plain text notes.\nNo headings here.";
        let sections = analyse_memory_md(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "general");
    }

    #[test]
    fn test_analyse_memory_md_empty() {
        let sections = analyse_memory_md("");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_migrate_dry_run() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let content = "# Memory\n\n## Patterns\n\nUse Result<T, E>.\n\n## Notes\n\nSome notes.\n";
        let plan = migrate_memory_md(content, &BlockScope::Project, &wd, &storage, false).unwrap();

        assert_eq!(plan.would_create.len(), 2);
        assert!(plan.would_create.contains(&"patterns".to_string()));
        assert!(plan.would_create.contains(&"notes".to_string()));

        // Dry run should not create files.
        let labels = storage.list(&BlockScope::Project, &wd).unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn test_migrate_execute() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let content = "# Memory\n\n## Patterns\n\nUse Result<T, E>.\n\n## Notes\n\nSome notes.\n";
        let plan = migrate_memory_md(content, &BlockScope::Project, &wd, &storage, true).unwrap();

        assert_eq!(plan.would_create.len(), 2);

        // Blocks should now exist.
        let patterns = storage
            .load("patterns", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(patterns.label, "patterns");
        assert!(patterns.content.contains("Result"));

        let notes = storage
            .load("notes", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(notes.label, "notes");
    }

    #[test]
    fn test_migrate_skips_existing() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        // Pre-create the "patterns" block.
        let existing = MemoryBlock::new("patterns", BlockScope::Project)
            .with_content("Existing patterns content".to_string());
        storage.save(&existing, &wd).unwrap();

        let content =
            "# Memory\n\n## Patterns\n\nNew patterns content.\n\n## Notes\n\nSome notes.\n";
        let plan = migrate_memory_md(content, &BlockScope::Project, &wd, &storage, true).unwrap();

        assert!(plan.would_skip.contains(&"patterns".to_string()));

        // Existing block should be unchanged.
        let loaded = storage
            .load("patterns", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.content, "Existing patterns content");
    }

    #[test]
    fn test_migration_plan_summary() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let content = "# Memory\n\n## Alpha\n\nA\n\n## Beta\n\nB\n";
        let plan = migrate_memory_md(content, &BlockScope::Project, &wd, &storage, false).unwrap();
        let summary = plan.summary();
        assert!(summary.contains("Would create: alpha, beta"));
    }
}
