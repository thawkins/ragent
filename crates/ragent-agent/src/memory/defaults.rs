//! Default memory block seeding.
//!
//! On first use, ragent creates a set of default memory blocks if they don't
//! already exist. This gives agents a useful starting structure without
//! requiring manual setup. The defaults are:
//!
//! - `persona.md` — Agent personality and communication preferences.
//! - `human.md` — User preferences and communication style.
//! - `project.md` — Project-specific conventions and notes.
//!
//! Each block is only created when the file does not already exist — this
//! module never overwrites user content.

use super::block::{BlockScope, MemoryBlock};
use super::storage::BlockStorage;
use std::path::PathBuf;

/// Seed default memory blocks for a given working directory.
///
/// Creates `persona.md`, `human.md`, and `project.md` in the project memory
/// directory, and `persona.md` + `human.md` in the global memory directory.
/// Existing files are never overwritten.
///
/// Returns the number of new blocks created.
pub fn seed_defaults(storage: &dyn BlockStorage, working_dir: &PathBuf) -> usize {
    let mut created = 0;

    // Project-scoped defaults.
    for (label, description, content) in project_defaults() {
        if storage
            .load(&label, &BlockScope::Project, working_dir)
            .ok()
            .flatten()
            .is_none()
        {
            let block = MemoryBlock::new(label, BlockScope::Project)
                .with_description(description)
                .with_content(content.to_string());
            if storage.save(&block, working_dir).is_ok() {
                created += 1;
            }
        }
    }

    // Global-scoped defaults.
    for (label, description, content) in global_defaults() {
        if storage
            .load(&label, &BlockScope::Global, working_dir)
            .ok()
            .flatten()
            .is_none()
        {
            let block = MemoryBlock::new(label, BlockScope::Global)
                .with_description(description)
                .with_content(content.to_string());
            if storage.save(&block, working_dir).is_ok() {
                created += 1;
            }
        }
    }

    created
}

/// Project-scoped default block definitions: (label, description, content).
fn project_defaults() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![(
        "project",
        "Project-specific conventions, architecture, and notes",
        "# Project Memory\n\nKey conventions and architecture notes for this project.\n\n## Conventions\n\n- (add project-specific conventions here)\n\n## Architecture\n\n- (add architecture notes here)\n",
    )]
}

/// Global-scoped default block definitions: (label, description, content).
fn global_defaults() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "persona",
            "Agent personality, communication style, and role preferences",
            "# Persona\n\n## Role\n\nI am a helpful AI coding assistant.\n\n## Communication Style\n\n- Clear and concise\n- Code-first when appropriate\n- Explain reasoning when asked\n\n## Preferences\n\n- (add your preferences here)\n",
        ),
        (
            "human",
            "User preferences, communication style, and working patterns",
            "# Human Preferences\n\n## Communication\n\n- (add how you prefer the agent to communicate)\n\n## Working Style\n\n- (add your working preferences)\n\n## Context\n\n- (add any context about yourself or your workflow)\n",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::storage::FileBlockStorage;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::Builder::new()
            .prefix("ragent-mem-defaults-")
            .tempdir()
            .expect("create temp dir")
    }

    #[test]
    fn test_seed_creates_defaults() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let created = seed_defaults(&storage, &wd);
        assert!(created >= 1, "Should create at least project defaults");

        // Verify project block exists.
        let project = storage
            .load("project", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(project.label, "project");
    }

    #[test]
    fn test_seed_idempotent() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let first = seed_defaults(&storage, &wd);
        let second = seed_defaults(&storage, &wd);

        assert!(first >= 1, "First call should create blocks");
        assert_eq!(second, 0, "Second call should not create duplicates");
    }

    #[test]
    fn test_seed_does_not_overwrite() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        // Manually create a project block with custom content.
        let custom = MemoryBlock::new("project", BlockScope::Project)
            .with_content("My custom content".to_string());
        storage.save(&custom, &wd).unwrap();

        // Seeding should not overwrite it.
        seed_defaults(&storage, &wd);
        let loaded = storage
            .load("project", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.content, "My custom content");
    }
}
