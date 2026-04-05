//! Resolve `@` references to actual file, directory, or URL content.
//!
//! Takes parsed [`ParsedRef`] items and fetches their content, returning
//! [`ResolvedRef`] structs ready for injection into the prompt.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::fuzzy::{collect_project_files, fuzzy_match};
use super::parse::{FileRef, ParsedRef, parse_refs};
use crate::tool::office_read;
use crate::tool::pdf_read;

/// Maximum content size per resolved reference (50 KB).
const MAX_CONTENT_SIZE: usize = 50 * 1024;

/// A resolved `@` reference with its fetched content.
#[derive(Debug, Clone)]
pub struct ResolvedRef {
    /// The original raw text (without `@` prefix).
    pub original: String,
    /// The classification of this reference.
    pub kind: FileRef,
    /// The resolved file/directory content or URL text.
    pub content: String,
    /// Whether the content was truncated due to size limits.
    pub truncated: bool,
    /// The resolved absolute path (for files/directories), if applicable.
    pub resolved_path: Option<PathBuf>,
}

/// Resolve a single parsed reference to its content.
///
/// # Errors
///
/// Returns an error if the file/directory cannot be read or the URL cannot
/// be fetched.
pub async fn resolve_ref(parsed: &ParsedRef, working_dir: &Path) -> Result<ResolvedRef> {
    match &parsed.kind {
        FileRef::File(path) => resolve_file(path, &parsed.raw, working_dir).await,
        FileRef::Directory(path) => resolve_directory(path, &parsed.raw, working_dir),
        FileRef::Url(url) => resolve_url(url, &parsed.raw).await,
        FileRef::Fuzzy(name) => resolve_fuzzy(name, &parsed.raw, working_dir).await,
    }
}

/// Resolve a file reference by reading its contents.
///
/// For binary formats (.docx, .xlsx, .pptx, .pdf), delegates to the
/// appropriate reader tool. Text files are read with `read_to_string`.
async fn resolve_file(path: &Path, raw: &str, working_dir: &Path) -> Result<ResolvedRef> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };

    if let Some(content) = try_read_binary(&abs_path, raw).await? {
        let (content, truncated) = truncate_content(content);
        return Ok(ResolvedRef {
            original: raw.to_string(),
            kind: FileRef::File(path.to_path_buf()),
            content,
            truncated,
            resolved_path: Some(abs_path),
        });
    }

    let content = tokio::fs::read_to_string(&abs_path)
        .await
        .with_context(|| format!("Cannot read file '@{raw}' ({})", abs_path.display()))?;

    let (content, truncated) = truncate_content(content);

    // Add line numbers
    let numbered = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4}  {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ResolvedRef {
        original: raw.to_string(),
        kind: FileRef::File(path.to_path_buf()),
        content: numbered,
        truncated,
        resolved_path: Some(abs_path),
    })
}

/// Resolve a directory reference by listing its contents.
fn resolve_directory(path: &Path, raw: &str, working_dir: &Path) -> Result<ResolvedRef> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };

    if !abs_path.is_dir() {
        anyhow::bail!("'@{raw}' is not a directory or does not exist");
    }

    let mut lines = Vec::new();
    lines.push(format!("{}/", abs_path.display()));
    list_recursive(&abs_path, "", 0, 2, &mut lines)?;

    let content = lines.join("\n");
    let (content, truncated) = truncate_content(content);

    Ok(ResolvedRef {
        original: raw.to_string(),
        kind: FileRef::Directory(path.to_path_buf()),
        content,
        truncated,
        resolved_path: Some(abs_path),
    })
}

/// Resolve a URL reference by fetching its content.
async fn resolve_url(url: &str, raw: &str) -> Result<ResolvedRef> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("ragent/0.1")
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch '@{raw}'"))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} fetching '@{raw}'", response.status().as_u16());
    }

    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read response from '@{raw}'"))?;

    // Simple HTML detection and conversion
    let processed = if body.trim_start().starts_with("<!") || body.trim_start().starts_with("<html")
    {
        html2text::from_read(body.as_bytes(), 120).unwrap_or(body)
    } else {
        body
    };

    let (content, truncated) = truncate_content(processed);

    Ok(ResolvedRef {
        original: raw.to_string(),
        kind: FileRef::Url(url.to_string()),
        content,
        truncated,
        resolved_path: None,
    })
}

/// Resolve a fuzzy name by finding the best-matching project file.
async fn resolve_fuzzy(name: &str, raw: &str, working_dir: &Path) -> Result<ResolvedRef> {
    let candidates = collect_project_files(working_dir, 10_000);
    let matches = fuzzy_match(name, &candidates);

    let best = matches
        .first()
        .with_context(|| format!("No file matching '@{raw}' found in project"))?;

    let abs_path = working_dir.join(&best.path);

    if let Some(content) = try_read_binary(&abs_path, raw).await? {
        let (content, truncated) = truncate_content(content);
        return Ok(ResolvedRef {
            original: raw.to_string(),
            kind: FileRef::File(best.path.clone()),
            content,
            truncated,
            resolved_path: Some(abs_path),
        });
    }

    let content = tokio::fs::read_to_string(&abs_path)
        .await
        .with_context(|| format!("Cannot read file '@{raw}' ({})", abs_path.display()))?;

    let (content, truncated) = truncate_content(content);

    let numbered = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4}  {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ResolvedRef {
        original: raw.to_string(),
        kind: FileRef::File(best.path.clone()),
        content: numbered,
        truncated,
        resolved_path: Some(abs_path),
    })
}

/// Attempt to read a file as a binary document (Office or PDF).
///
/// Returns `Some(content)` if the file has a recognised binary extension
/// (.docx, .xlsx, .pptx, .pdf) and was read successfully, or `None` if the
/// extension is not a binary document type (caller should fall back to
/// text reading).
async fn try_read_binary(abs_path: &Path, raw: &str) -> Result<Option<String>> {
    let ext = abs_path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    match ext.as_deref() {
        Some("docx" | "xlsx" | "pptx") => {
            let path = abs_path.to_path_buf();
            let ext_owned = ext.unwrap();
            let raw_owned = raw.to_string();
            let content = tokio::task::spawn_blocking(move || -> Result<String> {
                match ext_owned.as_str() {
                    "docx" => office_read::read_docx(&path, "markdown"),
                    "xlsx" => office_read::read_xlsx(&path, None, None, "markdown"),
                    "pptx" => office_read::read_pptx(&path, None, "markdown"),
                    _ => unreachable!(),
                }
            })
            .await
            .with_context(|| format!("Failed to read '@{raw_owned}'"))??;

            Ok(Some(content))
        }
        Some("pdf") => {
            let path = abs_path.to_path_buf();
            let raw_owned = raw.to_string();
            let content = tokio::task::spawn_blocking(move || -> Result<String> {
                pdf_read::read_pdf(&path, None, None, "text")
            })
            .await
            .with_context(|| format!("Failed to read '@{raw_owned}'"))??;

            Ok(Some(content))
        }
        _ => Ok(None),
    }
}

/// Truncate content to `MAX_CONTENT_SIZE` at a char boundary.
fn truncate_content(content: String) -> (String, bool) {
    if content.len() <= MAX_CONTENT_SIZE {
        return (content, false);
    }

    let end = content
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= MAX_CONTENT_SIZE)
        .last()
        .unwrap_or(0);

    let mut truncated = content[..end].to_string();
    truncated.push_str("\n\n[Content truncated — exceeded 50KB limit]");
    (truncated, true)
}

/// Parse all `@` references in input and resolve each, returning
/// the original text with a `<referenced_files>` block appended.
///
/// If no `@` references are found, returns the input unchanged and
/// an empty vector.
pub async fn resolve_all_refs(
    input: &str,
    working_dir: &Path,
) -> Result<(String, Vec<ResolvedRef>)> {
    let parsed = parse_refs(input);
    if parsed.is_empty() {
        return Ok((input.to_string(), Vec::new()));
    }

    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    for p in &parsed {
        match resolve_ref(p, working_dir).await {
            Ok(r) => resolved.push(r),
            Err(e) => errors.push(format!("@{}: {e}", p.raw)),
        }
    }

    if resolved.is_empty() && !errors.is_empty() {
        anyhow::bail!("Failed to resolve references:\n{}", errors.join("\n"));
    }

    // Build the augmented prompt
    let mut output = input.to_string();

    if !resolved.is_empty() {
        output.push_str("\n\n<referenced_files>\n");
        for r in &resolved {
            let tag = match &r.kind {
                FileRef::File(p) | FileRef::Directory(p) => {
                    format!("<file path=\"{}\">", p.display())
                }
                FileRef::Url(u) => format!("<file url=\"{u}\">"),
                FileRef::Fuzzy(n) => {
                    if let Some(rp) = &r.resolved_path {
                        format!("<file path=\"{}\">", rp.display())
                    } else {
                        format!("<file name=\"{n}\">")
                    }
                }
            };
            output.push_str(&tag);
            output.push('\n');
            output.push_str(&r.content);
            output.push_str("\n</file>\n");
        }
        output.push_str("</referenced_files>");
    }

    if !errors.is_empty() {
        output.push_str("\n\n[Warning: some references could not be resolved:\n");
        for e in &errors {
            output.push_str(&format!("  - {e}\n"));
        }
        output.push(']');
    }

    Ok((output, resolved))
}

/// Recursively list a directory tree (reused from list tool logic).
fn list_recursive(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) -> Result<()> {
    if depth >= max_depth {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory: {}", dir.display()))?
        .filter_map(std::result::Result::ok)
        .collect();

    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    entries.retain(|e| e.file_name().to_str().is_none_or(|n| !n.starts_with('.')));

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();

        if path.is_dir() {
            if matches!(
                name_str.as_ref(),
                "node_modules" | "target" | "__pycache__" | "dist" | "build"
            ) {
                lines.push(format!("{prefix}{connector}{name_str}/  (skipped)"));
                continue;
            }
            lines.push(format!("{prefix}{connector}{name_str}/"));
            let new_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
            list_recursive(&path, &new_prefix, depth + 1, max_depth, lines)?;
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            lines.push(format!(
                "{prefix}{connector}{name_str}  ({})",
                format_size(size)
            ));
        }
    }

    Ok(())
}

/// Format a byte count as a human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_file() {
        let tmp = std::env::temp_dir().join("ragent_test_resolve_file");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        std::fs::write(tmp.join("hello.txt"), "Hello\nWorld\n").expect("write");

        let parsed = ParsedRef {
            raw: "hello.txt".to_string(),
            kind: FileRef::File(PathBuf::from("hello.txt")),
            span: 0..10,
        };

        let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
        assert!(resolved.content.contains("Hello"));
        assert!(resolved.content.contains("World"));
        assert!(!resolved.truncated);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_resolve_directory() {
        let tmp = std::env::temp_dir().join("ragent_test_resolve_dir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("subdir")).expect("mkdir");
        std::fs::write(tmp.join("subdir/file.txt"), "content").expect("write");

        let parsed = ParsedRef {
            raw: "subdir/".to_string(),
            kind: FileRef::Directory(PathBuf::from("subdir")),
            span: 0..8,
        };

        let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
        assert!(resolved.content.contains("file.txt"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_resolve_fuzzy() {
        let tmp = std::env::temp_dir().join("ragent_test_resolve_fuzzy");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).expect("mkdir");
        std::fs::write(tmp.join("src/main.rs"), "fn main() {}").expect("write");

        let parsed = ParsedRef {
            raw: "main".to_string(),
            kind: FileRef::Fuzzy("main".to_string()),
            span: 0..5,
        };

        let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
        assert!(resolved.content.contains("fn main()"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_file() {
        let tmp = std::env::temp_dir().join("ragent_test_resolve_nofile");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");

        let parsed = ParsedRef {
            raw: "nope.txt".to_string(),
            kind: FileRef::File(PathBuf::from("nope.txt")),
            span: 0..9,
        };

        assert!(resolve_ref(&parsed, &tmp).await.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_resolve_all_refs_no_refs() {
        let tmp = std::env::temp_dir();
        let (text, resolved) = resolve_all_refs("plain text", &tmp).await.expect("resolve");
        assert_eq!(text, "plain text");
        assert!(resolved.is_empty());
    }

    #[tokio::test]
    async fn test_resolve_all_refs_with_file() {
        let tmp = std::env::temp_dir().join("ragent_test_resolve_all");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        std::fs::write(tmp.join("data.txt"), "line1\nline2\n").expect("write");

        let (text, resolved) = resolve_all_refs("Check @data.txt please", &tmp)
            .await
            .expect("resolve");

        assert_eq!(resolved.len(), 1);
        assert!(text.contains("<referenced_files>"));
        assert!(text.contains("line1"));
        assert!(text.contains("</file>"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_truncate_content_small() {
        let (content, truncated) = truncate_content("hello".to_string());
        assert_eq!(content, "hello");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_content_large() {
        let large = "x".repeat(MAX_CONTENT_SIZE + 100);
        let (content, truncated) = truncate_content(large);
        assert!(truncated);
        assert!(content.contains("[Content truncated"));
        assert!(content.len() <= MAX_CONTENT_SIZE + 100);
    }
}
