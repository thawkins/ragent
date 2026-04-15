//! Memory import/export system for portable data exchange.
//!
//! Supports exporting all memories and journal entries to a portable JSON format,
//! and importing from JSON or external formats (Cline Memory Bank, Claude Code).
//!
//! # Export format
//!
//! ```json
//! {
//!   "version": "1.0",
//!   "exported_at": "2025-07-15T10:30:00Z",
//!   "source": "ragent",
//!   "memories": [...],
//!   "journal": [...],
//!   "blocks": {...}
//! }
//! ```
//!
//! # CLI usage
//!
//! ```bash
//! ragent memory export > memories.json
//! ragent memory import < memories.json
//! ragent memory import --dry-run < memories.json
//! ragent memory import --format cline < cline_memory/
//! ragent memory import --format claude-code < .claude/memory.md
//! ```

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::memory::block::{BlockScope, MemoryBlock};
use crate::memory::journal::JournalEntry;
use crate::memory::storage::BlockStorage;
use crate::memory::store::StructuredMemory;
use crate::storage::Storage;

// ─�� Export format ─────────────────────────────────────────────────────────────

/// Top-level export container for all memory data.
///
/// Contains structured memories, journal entries, and file-based blocks
/// in a portable JSON format that can be imported by another ragent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryExport {
    /// Format version (semver-compatible).
    pub version: String,
    /// ISO 8601 timestamp when the export was created.
    pub exported_at: String,
    /// Source application that created this export.
    pub source: String,
    /// Structured memories from the SQLite store.
    #[serde(default)]
    pub memories: Vec<StructuredMemory>,
    /// Journal entries from the SQLite store.
    #[serde(default)]
    pub journal: Vec<JournalEntry>,
    /// File-based memory blocks, keyed by scope then label.
    #[serde(default)]
    pub blocks: MemoryBlocksExport,
}

/// Exported memory blocks organised by scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBlocksExport {
    /// Project-scoped blocks, keyed by label.
    #[serde(default)]
    pub project: std::collections::HashMap<String, String>,
    /// Global-scoped blocks, keyed by label.
    #[serde(default)]
    pub global: std::collections::HashMap<String, String>,
}

impl Default for MemoryBlocksExport {
    fn default() -> Self {
        Self {
            project: std::collections::HashMap::new(),
            global: std::collections::HashMap::new(),
        }
    }
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// Number of structured memories exported.
    pub memory_count: usize,
    /// Number of journal entries exported.
    pub journal_count: usize,
    /// Number of project blocks exported.
    pub project_block_count: usize,
    /// Number of global blocks exported.
    pub global_block_count: usize,
}

/// Result of an import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// Number of structured memories imported.
    pub memory_count: usize,
    /// Number of journal entries imported.
    pub journal_count: usize,
    /// Number of project blocks imported.
    pub project_block_count: usize,
    /// Number of global blocks imported.
    pub global_block_count: usize,
    /// Warnings encountered during import.
    #[serde(default)]
    pub warnings: Vec<String>,
}

// ── Export functions ─────���────────────────────────────────────────────────────

/// Export all memories, journal entries, and blocks to a portable JSON format.
///
/// # Arguments
///
/// * `storage` - SQLite storage backend.
/// * `block_storage` - File-based block storage backend.
/// * `working_dir` - Current project working directory.
///
/// # Returns
///
/// A `MemoryExport` struct containing all memory data and a result summary.
pub fn export_all(
    storage: &Storage,
    block_storage: &dyn BlockStorage,
    working_dir: &PathBuf,
) -> Result<(MemoryExport, ExportResult)> {
    let mut export = MemoryExport {
        version: "1.0".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        source: "ragent".to_string(),
        memories: Vec::new(),
        journal: Vec::new(),
        blocks: MemoryBlocksExport::default(),
    };

    // Export structured memories.
    let memories = storage.list_memories("", 100_000)?;
    for mem in &memories {
        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
        let mut structured = StructuredMemory::new(&mem.content, &mem.category)
            .with_confidence(mem.confidence)
            .with_source(&mem.source)
            .with_project(&mem.project)
            .with_session_id(&mem.session_id)
            .with_tags(tags);
        structured.id = mem.id;
        structured.created_at = mem.created_at.clone();
        structured.updated_at = mem.updated_at.clone();
        structured.access_count = mem.access_count;
        structured.last_accessed = mem.last_accessed.clone();
        export.memories.push(structured);
    }

    // Export journal entries.
    let journal = storage.list_journal_entries(100_000)?;
    for entry in &journal {
        let tags = storage.get_journal_tags(&entry.id).unwrap_or_default();
        let je = JournalEntry::new(&entry.title, &entry.content)
            .with_tags(tags)
            .with_project(&entry.project)
            .with_session_id(&entry.session_id);
        export.journal.push(je);
    }

    // Export project blocks.
    let project_labels = block_storage
        .list(&BlockScope::Project, working_dir)
        .unwrap_or_default();
    for label in &project_labels {
        if let Ok(Some(block)) = block_storage.load(label, &BlockScope::Project, working_dir) {
            export
                .blocks
                .project
                .insert(label.clone(), block.content.clone());
        }
    }

    // Export global blocks.
    let global_labels = block_storage
        .list(&BlockScope::Global, working_dir)
        .unwrap_or_default();
    for label in &global_labels {
        if let Ok(Some(block)) = block_storage.load(label, &BlockScope::Global, working_dir) {
            export
                .blocks
                .global
                .insert(label.clone(), block.content.clone());
        }
    }

    let result = ExportResult {
        memory_count: export.memories.len(),
        journal_count: export.journal.len(),
        project_block_count: export.blocks.project.len(),
        global_block_count: export.blocks.global.len(),
    };

    Ok((export, result))
}

// ── Import functions ──────────────────────────────────────────────────────────

/// Import memories, journal entries, and blocks from a ragent export JSON.
///
/// When `dry_run` is `true`, validates the import data without writing anything.
///
/// # Arguments
///
/// * `json_data` - JSON string in ragent export format.
/// * `storage` - SQLite storage backend.
/// * `block_storage` - File-based block storage backend.
/// * `working_dir` - Current project working directory.
/// * `dry_run` - If true, validate without writing.
///
/// # Returns
///
/// An `ImportResult` with counts and any warnings.
pub fn import_ragent(
    json_data: &str,
    storage: &Storage,
    block_storage: &dyn BlockStorage,
    working_dir: &PathBuf,
    dry_run: bool,
) -> Result<ImportResult> {
    let export: MemoryExport =
        serde_json::from_str(json_data).context("Failed to parse ragent export JSON")?;

    let mut result = ImportResult {
        memory_count: 0,
        journal_count: 0,
        project_block_count: 0,
        global_block_count: 0,
        warnings: Vec::new(),
    };

    // Import structured memories.
    for mem in &export.memories {
        if let Err(e) = StructuredMemory::validate_category(&mem.category) {
            result.warnings.push(format!(
                "Skipping memory with invalid category '{}': {e}",
                mem.category
            ));
            continue;
        }
        if let Err(e) = StructuredMemory::validate_confidence(mem.confidence) {
            result.warnings.push(format!(
                "Skipping memory with invalid confidence {}: {e}",
                mem.confidence
            ));
            continue;
        }
        if !dry_run {
            let id = storage.create_memory(
                &mem.content,
                &mem.category,
                &mem.source,
                mem.confidence,
                &mem.project,
                &mem.session_id,
                &mem.tags,
            )?;
        }
        result.memory_count += 1;
    }

    // Import journal entries.
    for entry in &export.journal {
        if let Err(e) = JournalEntry::validate_tags(&entry.tags) {
            result
                .warnings
                .push(format!("Skipping journal entry with invalid tags: {e}"));
            continue;
        }
        if !dry_run {
            storage.create_journal_entry(
                &entry.id,
                &entry.title,
                &entry.content,
                &entry.project,
                &entry.session_id,
                &entry.tags,
            )?;
        }
        result.journal_count += 1;
    }

    // Import project blocks.
    for (label, content) in &export.blocks.project {
        if let Err(e) = MemoryBlock::validate_label(label) {
            result.warnings.push(format!(
                "Skipping project block with invalid label '{label}': {e}"
            ));
            continue;
        }
        if !dry_run {
            let existing = block_storage
                .load(label, &BlockScope::Project, working_dir)
                .ok()
                .flatten();
            let block = if let Some(mut existing) = existing {
                existing.content = content.clone();
                existing.updated_at = Utc::now();
                existing
            } else {
                MemoryBlock::new(label, BlockScope::Project).with_content(content.clone())
            };
            block_storage.save(&block, working_dir)?;
        }
        result.project_block_count += 1;
    }

    // Import global blocks.
    for (label, content) in &export.blocks.global {
        if let Err(e) = MemoryBlock::validate_label(label) {
            result.warnings.push(format!(
                "Skipping global block with invalid label '{label}': {e}"
            ));
            continue;
        }
        if !dry_run {
            let existing = block_storage
                .load(label, &BlockScope::Global, working_dir)
                .ok()
                .flatten();
            let block = if let Some(mut existing) = existing {
                existing.content = content.clone();
                existing.updated_at = Utc::now();
                existing
            } else {
                MemoryBlock::new(label, BlockScope::Global).with_content(content.clone())
            };
            block_storage.save(&block, working_dir)?;
        }
        result.global_block_count += 1;
    }

    Ok(result)
}

// ── Cline Memory Bank adapter ────────────────────────────────────────────────

/// Import from Cline Memory Bank format.
///
/// Cline stores memories as separate `.md` files in a `.clinerules` or
/// `cline_memory` directory. Each file is treated as a memory block.
/// The filename (minus `.md`) becomes the block label.
///
/// Best-effort mapping:
/// - `activeContext.md` → project block `active-context`
/// - `techStack.md` → project block `tech-stack`
/// - `projectProgress.md` → project block `project-progress`
/// - Other `.md` files → project blocks with slugified names
///
/// # Arguments
///
/// * `dir_path` - Path to the Cline Memory Bank directory.
/// * `block_storage` - File-based block storage backend.
/// * `working_dir` - Current project working directory.
/// * `dry_run` - If true, validate without writing.
///
/// # Returns
///
/// An `ImportResult` with counts and any warnings.
pub fn import_cline(
    dir_path: &PathBuf,
    block_storage: &dyn BlockStorage,
    working_dir: &PathBuf,
    dry_run: bool,
) -> Result<ImportResult> {
    let mut result = ImportResult {
        memory_count: 0,
        journal_count: 0,
        project_block_count: 0,
        global_block_count: 0,
        warnings: Vec::new(),
    };

    if !dir_path.exists() || !dir_path.is_dir() {
        anyhow::bail!(
            "Cline Memory Bank directory not found: {}",
            dir_path.display()
        );
    }

    for entry in std::fs::read_dir(dir_path)
        .with_context(|| format!("Failed to read Cline directory: {}", dir_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !filename.ends_with(".md") {
            continue;
        }

        let label = slugify_cline_filename(filename);
        if let Err(e) = MemoryBlock::validate_label(&label) {
            result.warnings.push(format!(
                "Skipping Cline file '{filename}' — invalid label '{label}': {e}"
            ));
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read Cline file: {}", path.display()))?;

        if !dry_run {
            let block = MemoryBlock::new(&label, BlockScope::Project).with_content(content.clone());
            block_storage.save(&block, working_dir)?;
        }
        result.project_block_count += 1;
    }

    Ok(result)
}

/// Convert a Cline Memory Bank filename to a valid block label.
///
/// Strips the `.md` extension and converts PascalCase/camelCase to kebab-case.
fn slugify_cline_filename(filename: &str) -> String {
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    let mut result = String::new();
    for (i, ch) in stem.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            result.push('-');
            result.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if ch == '_' || ch == ' ' || ch == '-' {
            if !result.ends_with('-') {
                result.push('-');
            }
        }
    }
    // Remove trailing hyphens.
    let trimmed = result.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "cline-memory".to_string()
    } else {
        trimmed
    }
}

// ── Claude Code auto-memory adapter ──────────────────────────────────────────

/// Import from Claude Code auto-memory format.
///
/// Claude Code stores memories in a single `.claude/memory.md` file with
/// sections marked by markdown headings. This adapter splits the file into
/// separate blocks based on headings, similar to the `memory_migrate` tool.
///
/// Best-effort mapping:
/// - Top-level headings → separate blocks
/// - Content before any heading → `general` block
/// - If the file has no structure, imported as a single `claude-memory` block
///
/// # Arguments
///
/// * `file_path` - Path to the Claude Code memory file.
/// * `block_storage` - File-based block storage backend.
/// * `working_dir` - Current project working directory.
/// * `dry_run` - If true, validate without writing.
///
/// # Returns
///
/// An `ImportResult` with counts and any warnings.
pub fn import_claude_code(
    file_path: &PathBuf,
    block_storage: &dyn BlockStorage,
    working_dir: &PathBuf,
    dry_run: bool,
) -> Result<ImportResult> {
    let mut result = ImportResult {
        memory_count: 0,
        journal_count: 0,
        project_block_count: 0,
        global_block_count: 0,
        warnings: Vec::new(),
    };

    if !file_path.exists() {
        anyhow::bail!("Claude Code memory file not found: {}", file_path.display());
    }

    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read Claude Code file: {}", file_path.display()))?;

    // Use the migration module to split by headings.
    let sections = crate::memory::migrate::analyse_memory_md(&content);

    if sections.is_empty() {
        result
            .warnings
            .push("Claude Code memory file is empty — nothing to import".to_string());
        return Ok(result);
    }

    for (label, section_content) in &sections {
        if let Err(e) = MemoryBlock::validate_label(label) {
            result.warnings.push(format!(
                "Skipping section with invalid label '{label}': {e}"
            ));
            continue;
        }

        if !dry_run {
            let block =
                MemoryBlock::new(label, BlockScope::Project).with_content(section_content.clone());
            block_storage.save(&block, working_dir)?;
        }
        result.project_block_count += 1;
    }

    Ok(result)
}

// ── Tests ──────────────────────────────��─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::storage::FileBlockStorage;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::Builder::new()
            .prefix("ragent-import-export-")
            .tempdir()
            .expect("create temp dir")
    }

    #[test]
    fn test_export_format_serialisation() {
        let export = MemoryExport {
            version: "1.0".to_string(),
            exported_at: "2025-07-15T10:30:00Z".to_string(),
            source: "ragent".to_string(),
            memories: vec![
                StructuredMemory::new("Test fact", "fact").with_tags(vec!["test".to_string()]),
            ],
            journal: vec![JournalEntry::new("Test entry", "Test content")],
            blocks: MemoryBlocksExport {
                project: std::collections::HashMap::from([(
                    "patterns".to_string(),
                    "Some patterns".to_string(),
                )]),
                global: std::collections::HashMap::new(),
            },
        };

        let json = serde_json::to_string_pretty(&export).unwrap();
        assert!(json.contains("\"version\": \"1.0\""));
        assert!(json.contains("\"source\": \"ragent\""));
        assert!(json.contains("Test fact"));
        assert!(json.contains("patterns"));
    }

    #[test]
    fn test_export_roundtrip() {
        let export = MemoryExport {
            version: "1.0".to_string(),
            exported_at: "2025-07-15T10:30:00Z".to_string(),
            source: "ragent".to_string(),
            memories: vec![StructuredMemory::new("Fact 1", "fact")],
            journal: Vec::new(),
            blocks: MemoryBlocksExport::default(),
        };

        let json = serde_json::to_string(&export).unwrap();
        let parsed: MemoryExport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, "1.0");
        assert_eq!(parsed.memories.len(), 1);
        assert_eq!(parsed.memories[0].content, "Fact 1");
    }

    #[test]
    fn test_slugify_cline_filename() {
        assert_eq!(slugify_cline_filename("activeContext.md"), "active-context");
        assert_eq!(slugify_cline_filename("techStack.md"), "tech-stack");
        assert_eq!(
            slugify_cline_filename("projectProgress.md"),
            "project-progress"
        );
        assert_eq!(slugify_cline_filename("simple.md"), "simple");
        // API_Conventions slugifies with uppercase→lowercase+hyphen
        let result = slugify_cline_filename("API_Conventions.md");
        assert!(!result.is_empty());
        assert!(result.starts_with('a')); // lowercase start
    }

    #[test]
    fn test_import_cline_directory() {
        let dir = setup();
        let working_dir = PathBuf::from(dir.path());
        let storage = FileBlockStorage::new();

        // Create Cline-style files.
        let cline_dir = dir.path().join("cline_memory");
        std::fs::create_dir_all(&cline_dir).unwrap();
        std::fs::write(cline_dir.join("activeContext.md"), "Current context").unwrap();
        std::fs::write(cline_dir.join("techStack.md"), "Rust, SQLite").unwrap();

        let result =
            import_cline(&PathBuf::from(&cline_dir), &storage, &working_dir, false).unwrap();
        assert_eq!(result.project_block_count, 2);
        assert!(result.warnings.is_empty());

        // Verify blocks were created.
        let block1 = storage
            .load("active-context", &BlockScope::Project, &working_dir)
            .unwrap()
            .unwrap();
        assert_eq!(block1.content, "Current context");

        let block2 = storage
            .load("tech-stack", &BlockScope::Project, &working_dir)
            .unwrap()
            .unwrap();
        assert_eq!(block2.content, "Rust, SQLite");
    }

    #[test]
    fn test_import_cline_dry_run() {
        let dir = setup();
        let working_dir = PathBuf::from(dir.path());
        let storage = FileBlockStorage::new();

        let cline_dir = dir.path().join("cline_memory");
        std::fs::create_dir_all(&cline_dir).unwrap();
        std::fs::write(cline_dir.join("notes.md"), "Some notes").unwrap();

        let result =
            import_cline(&PathBuf::from(&cline_dir), &storage, &working_dir, true).unwrap();
        assert_eq!(result.project_block_count, 1);

        // Verify block was NOT created (dry run).
        let block = storage
            .load("notes", &BlockScope::Project, &working_dir)
            .unwrap();
        assert!(block.is_none());
    }

    #[test]
    fn test_import_claude_code_file() {
        let dir = setup();
        let working_dir = PathBuf::from(dir.path());
        let storage = FileBlockStorage::new();

        // Create a Claude Code-style memory file.
        let claude_file = dir.path().join("memory.md");
        std::fs::write(
            &claude_file,
            "# Claude Memory\n\n## Patterns\n\nRust patterns here\n\n## Conventions\n\nFollow AGENTS.md\n",
        )
        .unwrap();

        let result =
            import_claude_code(&PathBuf::from(&claude_file), &storage, &working_dir, false)
                .unwrap();
        assert_eq!(result.project_block_count, 2);

        let patterns = storage
            .load("patterns", &BlockScope::Project, &working_dir)
            .unwrap()
            .unwrap();
        assert!(patterns.content.contains("Rust patterns"));
    }

    #[test]
    fn test_memory_blocks_export_default() {
        let blocks = MemoryBlocksExport::default();
        assert!(blocks.project.is_empty());
        assert!(blocks.global.is_empty());
    }
}
