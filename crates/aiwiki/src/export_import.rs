//! Export and import functionality for AIWiki.
//!
//! Provides:
//! - Export wiki as single markdown file
//! - Import external markdown into wiki
//! - Obsidian-compatible vault export

use crate::{Aiwiki, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Export the entire wiki as a single combined markdown file.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `output_path` - Where to write the combined markdown file
///
/// # Returns
/// The number of pages exported.
pub async fn export_single_markdown(wiki: &Aiwiki, output_path: impl AsRef<Path>) -> Result<usize> {
    let wiki_dir = wiki.path("wiki");
    let output_path = output_path.as_ref();

    let mut all_content = String::new();
    let mut exported_count = 0;

    // Add header
    all_content.push_str(&format!("# {}\n\n", wiki.config.name));
    all_content.push_str(&format!(
        "Exported on: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    all_content.push_str("---\n\n");

    // Collect all markdown files
    let mut files = Vec::new();
    collect_markdown_files(&wiki_dir, &wiki_dir, &mut files).await?;

    // Sort by path for consistent ordering
    files.sort();

    // Process each file
    for file_path in &files {
        let content = fs::read_to_string(file_path).await?;
        let relative_path = file_path
            .strip_prefix(&wiki_dir)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Add section separator and file info
        all_content.push_str(&format!("\n\n---\n\n"));
        all_content.push_str(&format!("## {}\n\n", relative_path));

        // Strip YAML frontmatter if present
        let content_without_frontmatter = strip_frontmatter(&content);
        all_content.push_str(content_without_frontmatter);

        exported_count += 1;
    }

    // Add footer
    all_content.push_str("\n\n---\n\n");
    all_content.push_str(&format!(
        "*Exported {} pages from {}*",
        exported_count, wiki.config.name
    ));

    // Write to file
    fs::write(output_path, all_content).await?;

    Ok(exported_count)
}

/// Export the wiki as an Obsidian-compatible vault.
///
/// Creates a directory structure compatible with Obsidian:
/// - All wiki pages as markdown files
/// - An `obsidian/` directory with config files
/// - Proper frontmatter for Obsidian
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `output_dir` - Directory to create the vault in
///
/// # Returns
/// The number of pages exported.
pub async fn export_obsidian_vault(wiki: &Aiwiki, output_dir: impl AsRef<Path>) -> Result<usize> {
    let wiki_dir = wiki.path("wiki");
    let output_dir = output_dir.as_ref();

    // Create output directory
    fs::create_dir_all(output_dir).await?;

    // Create .obsidian directory with config
    let obsidian_dir = output_dir.join(".obsidian");
    fs::create_dir_all(&obsidian_dir).await?;

    // Create app.json
    let app_config = serde_json::json!({
        "alwaysUpdateLinks": true
    });
    fs::write(
        obsidian_dir.join("app.json"),
        serde_json::to_string_pretty(&app_config)?,
    )
    .await?;

    // Create appearance.json (use dark mode by default)
    let appearance_config = serde_json::json!({
        "baseFontSize": 16,
        "theme": "obsidian",
        "cssTheme": ""
    });
    fs::write(
        obsidian_dir.join("appearance.json"),
        serde_json::to_string_pretty(&appearance_config)?,
    )
    .await?;

    // Create core-plugins.json
    let core_plugins = serde_json::json!([
        "graph",
        "backlink",
        "page-preview",
        "note-composer",
        "command-palette",
        "editor-status",
        "starred",
        "outline",
        "word-count",
        "file-recovery"
    ]);
    fs::write(
        obsidian_dir.join("core-plugins.json"),
        serde_json::to_string_pretty(&core_plugins)?,
    )
    .await?;

    // Copy all wiki pages
    let mut exported_count = 0;
    let mut files = Vec::new();
    collect_markdown_files(&wiki_dir, &wiki_dir, &mut files).await?;

    for file_path in &files {
        let content = fs::read_to_string(file_path).await?;
        let relative_path = file_path.strip_prefix(&wiki_dir).unwrap_or(file_path);

        // Create parent directories in output
        let output_file_path = output_dir.join(relative_path);
        if let Some(parent) = output_file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Convert frontmatter to Obsidian format
        let converted_content = convert_to_obsidian_format(&content, &wiki.config.name);
        fs::write(output_file_path, converted_content).await?;

        exported_count += 1;
    }

    // Create README
    let readme = format!(
        "# {}\n\n\
         This is an Obsidian-compatible vault exported from AIWiki on {}.\n\n\
         ## Usage\n\n\
         1. Open this folder as a vault in Obsidian\n\
         2. All your wiki pages are ready to use\n\
         3. Links between pages should work automatically\n\n\
         ## Features\n\n\
         - **Graph view**: See connections between pages\n\
         - **Backlinks**: Find pages that link to the current page\n\
         - **Tags**: Frontmatter tags are preserved\n",
        wiki.config.name,
        chrono::Utc::now().format("%Y-%m-%d")
    );
    fs::write(output_dir.join("README.md"), readme).await?;

    Ok(exported_count)
}

/// Import external markdown files into the wiki.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `source_path` - Path to markdown file or directory
/// * `target_subdir` - Optional subdirectory in wiki/ (e.g., "imports")
///
/// # Returns
/// The number of files imported.
pub async fn import_markdown(
    wiki: &Aiwiki,
    source_path: impl AsRef<Path>,
    target_subdir: Option<&str>,
) -> Result<usize> {
    let source_path = source_path.as_ref();
    let wiki_dir = wiki.path("wiki");

    let target_dir = if let Some(subdir) = target_subdir {
        wiki_dir.join(subdir)
    } else {
        wiki_dir.clone()
    };

    fs::create_dir_all(&target_dir).await?;

    let mut imported_count = 0;

    if source_path.is_file() {
        // Import single file
        import_single_markdown_file(source_path, &target_dir).await?;
        imported_count = 1;
    } else if source_path.is_dir() {
        // Import directory recursively
        imported_count = import_markdown_directory(source_path, &target_dir).await?;
    }

    Ok(imported_count)
}

/// Collect all markdown files recursively.
async fn collect_markdown_files(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut entries = fs::read_dir(current_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            Box::pin(collect_markdown_files(base_dir, &path, files)).await?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            files.push(path);
        }
    }

    Ok(())
}

/// Strip YAML frontmatter from content.
fn strip_frontmatter(content: &str) -> &str {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        if let Some(end_pos) = content[4..].find("---") {
            return &content[4 + end_pos + 3..];
        }
    }
    content
}

/// Convert wiki content to Obsidian format.
fn convert_to_obsidian_format(content: &str, vault_name: &str) -> String {
    // For now, just pass through the content
    // In the future, this could:
    // - Convert wiki-style links to Obsidian format
    // - Adjust frontmatter
    // - Handle special syntax
    let _ = vault_name; // Unused for now
    content.to_string()
}

/// Import a single markdown file.
async fn import_single_markdown_file(source: &Path, target_dir: &Path) -> Result<()> {
    let file_name = source
        .file_name()
        .ok_or_else(|| crate::AiwikiError::Config("Invalid source file".to_string()))?;

    let content = fs::read_to_string(source).await?;
    let target_path = target_dir.join(file_name);

    fs::write(target_path, content).await?;

    Ok(())
}

/// Import a directory of markdown files recursively.
async fn import_markdown_directory(source: &Path, target_dir: &Path) -> Result<usize> {
    let mut count = 0;
    let mut entries = fs::read_dir(source).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default();

        if path.is_dir() {
            // Create subdirectory and recurse
            let sub_target = target_dir.join(file_name);
            fs::create_dir_all(&sub_target).await?;
            count += Box::pin(import_markdown_directory(&path, &sub_target)).await?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            // Copy markdown file
            let target_path = target_dir.join(file_name);
            let content = fs::read_to_string(&path).await?;
            fs::write(target_path, content).await?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_frontmatter() {
        let with_frontmatter = "---\ntitle: Test\n---\n# Hello\n\nContent";
        let without_frontmatter = "# Hello\n\nContent";
        assert_eq!(strip_frontmatter(with_frontmatter), without_frontmatter);

        // No frontmatter
        assert_eq!(strip_frontmatter("# Hello"), "# Hello");
    }
}
