//! File-based storage backend for memory blocks.
//!
//! [`FileBlockStorage`] reads and writes memory blocks as `.md` files in the
//! `.ragent/memory/` directory (project scope) or `~/.ragent/memory/`
//! (global scope). Each block file contains YAML frontmatter with metadata
//! and a Markdown body with the block content.
//!
//! Writes are atomic (write to a temp file, then rename) to prevent data
//! corruption on crash.

use anyhow::{Context, Result};
use std::path::PathBuf;

use super::block::{BlockScope, MemoryBlock, resolve_block_dir};

/// Abstract storage interface for memory blocks.
///
/// All operations are synchronous because they involve small, local file I/O.
/// The trait is object-safe so it can be mocked in tests.
pub trait BlockStorage: Send + Sync {
    /// Load a memory block by label and scope.
    ///
    /// Returns `Ok(Some(block))` if the file exists, `Ok(None)` if not found.
    fn load(
        &self,
        label: &str,
        scope: &BlockScope,
        working_dir: &PathBuf,
    ) -> Result<Option<MemoryBlock>>;

    /// Save a memory block, creating or overwriting the file.
    ///
    /// Enforces the block's content limit. Writes are atomic.
    fn save(&self, block: &MemoryBlock, working_dir: &PathBuf) -> Result<()>;

    /// List all memory block labels in the given scope.
    fn list(&self, scope: &BlockScope, working_dir: &PathBuf) -> Result<Vec<String>>;

    /// Delete a memory block by label and scope.
    fn delete(&self, label: &str, scope: &BlockScope, working_dir: &PathBuf) -> Result<()>;
}

/// File-based implementation of [`BlockStorage`].
///
/// Stores each block as `<memory_dir>/<label>.md` with YAML frontmatter.
pub struct FileBlockStorage;

impl FileBlockStorage {
    /// Create a new file-based block storage instance.
    pub fn new() -> Self {
        Self
    }

    /// Resolve the full file path for a block.
    fn block_path(label: &str, scope: &BlockScope, working_dir: &PathBuf) -> Result<PathBuf> {
        let dir = resolve_block_dir(scope, working_dir)?;
        Ok(dir.join(format!("{label}.md")))
    }
}

impl Default for FileBlockStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockStorage for FileBlockStorage {
    fn load(
        &self,
        label: &str,
        scope: &BlockScope,
        working_dir: &PathBuf,
    ) -> Result<Option<MemoryBlock>> {
        let path = Self::block_path(label, scope, working_dir)?;
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read memory block: {}", path.display()))?;
        let block = MemoryBlock::from_markdown(&text, scope.clone());
        Ok(Some(block))
    }

    fn save(&self, block: &MemoryBlock, working_dir: &PathBuf) -> Result<()> {
        // Enforce content limit before writing.
        if let Err(e) = block.check_content_limit() {
            anyhow::bail!("{e}");
        }

        let dir = resolve_block_dir(&block.scope, working_dir)?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create memory directory: {}", dir.display()))?;

        let path = dir.join(block.filename());
        let content = block.to_markdown();

        // Atomic write: write to temp file, then rename.
        let temp_path = path.with_extension("md.tmp");
        std::fs::write(&temp_path, &content)
            .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;
        std::fs::rename(&temp_path, &path)
            .with_context(|| format!("Failed to rename temp file to {}", path.display()))?;

        Ok(())
    }

    fn list(&self, scope: &BlockScope, working_dir: &PathBuf) -> Result<Vec<String>> {
        let dir = resolve_block_dir(scope, working_dir)?;
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut labels = Vec::new();
        let entries = std::fs::read_dir(&dir)
            .with_context(|| format!("Failed to read memory directory: {}", dir.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Only include files with valid labels (lowercase, starts with letter).
                if MemoryBlock::validate_label(stem).is_ok() {
                    labels.push(stem.to_string());
                }
            }
        }

        labels.sort();
        Ok(labels)
    }

    fn delete(&self, label: &str, scope: &BlockScope, working_dir: &PathBuf) -> Result<()> {
        let path = Self::block_path(label, scope, working_dir)?;
        if !path.exists() {
            anyhow::bail!("Memory block '{label}' not found at {}", path.display());
        }
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to delete memory block: {}", path.display()))?;
        Ok(())
    }
}

/// Load all memory blocks from both scopes for a working directory.
///
/// Returns a vector of (scope, block) pairs. Files that cannot be parsed
/// are silently skipped (logged via tracing).
pub fn load_all_blocks(
    storage: &dyn BlockStorage,
    working_dir: &PathBuf,
) -> Vec<(BlockScope, MemoryBlock)> {
    let mut blocks = Vec::new();

    for scope in [BlockScope::Project, BlockScope::Global] {
        if let Ok(labels) = storage.list(&scope, working_dir) {
            for label in labels {
                match storage.load(&label, &scope, working_dir) {
                    Ok(Some(block)) => blocks.push((scope.clone(), block)),
                    Ok(None) => {
                        tracing::debug!("Block '{label}' listed but not found on load");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load memory block '{label}': {e}");
                    }
                }
            }
        }
    }

    blocks
}

/// Load a legacy MEMORY.md file as a block, if it exists.
///
/// This handles backward compatibility: existing MEMORY.md files that don't
/// have YAML frontmatter are loaded as blocks with label "MEMORY" and the
/// given default scope.
pub fn load_legacy_memory(scope: &BlockScope, working_dir: &PathBuf) -> Option<MemoryBlock> {
    let dir = resolve_block_dir(scope, working_dir).ok()?;
    let path = dir.join("MEMORY.md");
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    Some(MemoryBlock::from_markdown(&text, scope.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::Builder::new()
            .prefix("ragent-mem-block-")
            .tempdir()
            .expect("create temp dir")
    }

    #[test]
    fn test_save_and_load() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let block = MemoryBlock::new("patterns", BlockScope::Project)
            .with_description("Observed patterns".to_string())
            .with_content("Use `Result<T, E>` for error handling.".to_string());

        storage.save(&block, &wd).unwrap();
        let loaded = storage
            .load("patterns", &BlockScope::Project, &wd)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.label, "patterns");
        assert_eq!(loaded.description, "Observed patterns");
        assert_eq!(loaded.scope, BlockScope::Project);
        assert_eq!(loaded.content, "Use `Result<T, E>` for error handling.");
    }

    #[test]
    fn test_load_nonexistent() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let result = storage
            .load("nonexistent", &BlockScope::Project, &wd)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_empty() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let labels = storage.list(&BlockScope::Project, &wd).unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn test_list_multiple() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        for label in &["alpha", "beta", "gamma"] {
            let block = MemoryBlock::new(*label, BlockScope::Project)
                .with_content(format!("Content for {label}"));
            storage.save(&block, &wd).unwrap();
        }

        let labels = storage.list(&BlockScope::Project, &wd).unwrap();
        assert_eq!(labels, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_delete() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let block = MemoryBlock::new("to-delete", BlockScope::Project)
            .with_content("temporary".to_string());
        storage.save(&block, &wd).unwrap();
        storage
            .delete("to-delete", &BlockScope::Project, &wd)
            .unwrap();
        let result = storage
            .load("to-delete", &BlockScope::Project, &wd)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let result = storage.delete("ghost", &BlockScope::Project, &wd);
        assert!(result.is_err());
    }

    #[test]
    fn test_content_limit_enforced() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let block = MemoryBlock::new("limited", BlockScope::Project)
            .with_limit(10)
            .with_content("This is way more than ten bytes".to_string());

        let result = storage.save(&block, &wd);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("exceeds block limit")
        );
    }

    #[test]
    fn test_atomic_write() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        let block = MemoryBlock::new("atomic", BlockScope::Project)
            .with_content("safe content".to_string());
        storage.save(&block, &wd).unwrap();

        // No temp file should be left behind.
        let dir = wd.join(".ragent/memory");
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.ends_with(".tmp"), "Temp file left behind: {name}");
        }
    }

    #[test]
    fn test_load_all_blocks() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        // Create blocks in both scopes.
        let project_block = MemoryBlock::new("project-notes", BlockScope::Project)
            .with_content("Project stuff".to_string());
        storage.save(&project_block, &wd).unwrap();

        // Global blocks need a real home dir — skip if unavailable in CI.
        if dirs::home_dir().is_some() {
            let global_block = MemoryBlock::new("global-notes", BlockScope::Global)
                .with_content("Global stuff".to_string());
            storage.save(&global_block, &wd).unwrap();
        }

        let all = load_all_blocks(&storage, &wd);
        assert!(!all.is_empty());
    }

    #[test]
    fn test_load_legacy_memory() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let mem_dir = wd.join(".ragent/memory");
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(mem_dir.join("MEMORY.md"), "Legacy content here.").unwrap();

        let block = load_legacy_memory(&BlockScope::Project, &wd).unwrap();
        assert_eq!(block.label, "MEMORY");
        assert_eq!(block.content, "Legacy content here.");
    }

    #[test]
    fn test_load_legacy_memory_empty() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        // No MEMORY.md exists.
        let result = load_legacy_memory(&BlockScope::Project, &wd);
        assert!(result.is_none());
    }

    #[test]
    fn test_global_scope_save_and_load() {
        let tmp = setup();
        let wd = PathBuf::from(tmp.path());
        let storage = FileBlockStorage::new();

        // Only run if home dir is available.
        if dirs::home_dir().is_none() {
            return;
        }

        let block = MemoryBlock::new("global-test", BlockScope::Global)
            .with_content("Shared across projects".to_string());
        storage.save(&block, &wd).unwrap();

        let loaded = storage
            .load("global-test", &BlockScope::Global, &wd)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.label, "global-test");
        assert_eq!(loaded.scope, BlockScope::Global);
        assert_eq!(loaded.content, "Shared across projects");

        // Cleanup.
        let home = dirs::home_dir().unwrap();
        let _ = std::fs::remove_file(home.join(".ragent/memory/global-test.md"));
    }
}
