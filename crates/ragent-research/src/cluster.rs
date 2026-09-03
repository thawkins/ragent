//! Concept-clustering helpers for `/research cluster` (T-003).
//!
//! This module reads the captured source documents under a research folder's
//! `sources/` directory and assembles them into a single payload that respects
//! the active LLM provider's context window (FR-003, FR-004, NFR-002).

use crate::io::{ResearchIo, ResearchIoError};
use crate::research_name::ResearchName;
use std::path::{Path, PathBuf};

/// Default context-window size in tokens, used when the active model's
/// context window cannot be resolved.
pub const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 128_000;

/// Tokens reserved for the concept-extraction prompt, model response, and
/// safety margin when computing the source-payload budget.
pub const PROMPT_RESERVE_TOKENS: usize = 4_096;

/// Conservative bytes-per-token estimate (FR-004). UTF-8 text averages about
/// 4 bytes per token for English prose, so this gives a safe upper bound on the
/// number of source bytes we can stuff into the prompt.
pub const BYTES_PER_TOKEN_GUESS: usize = 4;

/// Fixed concept-extraction prompt template used by `/research cluster`.
///
/// The placeholder `[INSERT_DOCUMENTS_HERE]` is replaced with the assembled
/// source payload by [`build_concept_extraction_prompt`] before the prompt is
/// dispatched to the active LLM (FR-005, FR-006, FR-014).
///
/// The prompt explicitly asks for a predictable markdown structure so the
/// resulting `CONCEPTS.md` can be lightly normalized by
/// [`format_concepts_md_with_sources`]. Concept headings are numbered
/// sequentially and evidence bullets cite sources with `[#N]` markers so each
/// concept traces back to the References Index in `RESEARCH.md`.
pub const CONCEPT_EXTRACTION_PROMPT_TEMPLATE: &str = "You are an expert data analyst and researcher. Analyze the provided documents and extract the most important concepts, themes, and ideas across them.\n\
    \n\
    Instructions:\n\
    \n\
    1. Read all documents to understand the overall context.\n\
    2. Identify up to 20 core concepts that appear frequently, carry significant weight, or tie the documents together. Avoid overlapping or repetitive concepts.\n\
    3. For each concept, produce a markdown section with:\n\
    \n\
       - A level-2 heading (`## N. Concept Name`) with a concise label (2-4 words), where N is the concept's sequential number starting at 1.\n\
       - A **Definition** paragraph (1-2 sentences).\n\
       - A **Key Evidence** bullet list with 1-2 brief examples from the text, citing the contributing documents with `[#N]` citation markers (e.g. `[#1]`, `[#7]`), where N is the number in each document header (`--- web-NN.md ---`).\n\
    \n\
    Output format requirements:\n\
    \n\
    - Begin the response with a single `# Concepts` level-1 heading.\n\
    - Use level-2 headings (`##`) for each concept name, numbered sequentially (`## 1. First Concept`, `## 2. Second Concept`, ...).\n\
    - Use bold labels (`**Definition:**` and `**Key Evidence:**`) inside each section.\n\
    - Keep evidence bullets short and specific, and cite sources with `[#N]` markers (N matches the number in each document's `web-NN.md` header) for each claim.\n\
    - Focus on depth and relevance over quantity.\n\
    \n\
    Here are the documents:\n\
    \n\
    [INSERT_DOCUMENTS_HERE]\n";

/// Lightweight post-processor that normalizes an LLM-generated concept-extraction
/// response into a consistent `CONCEPTS.md` layout (FR-014).
///
/// Normalizations applied:
/// * Trim leading/trailing whitespace.
/// * Collapse runs of more than two consecutive blank lines to a single blank line.
/// * Ensure the content starts with `# Concepts` if no top-level heading is present.
/// * Ensure a single trailing newline.
#[must_use]
pub fn format_concepts_md(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "# Concepts\n\nNo concepts were extracted.\n".to_string();
    }

    // Remove leading blank lines with iterator-based stripping (avoids O(n²)
    // Vec::remove(0) shifts).
    let mut lines: Vec<&str> = trimmed
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .collect();

    // Drop trailing blank lines.
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    // Collapse runs of blank lines to one and build the output in a single
    // pass, avoiding per-line String allocations when the line is unchanged.
    let mut out = String::with_capacity(trimmed.len() + 64);
    let mut blank_run = false;
    let mut first = true;
    for line in lines {
        if line.trim().is_empty() {
            if !blank_run && !first {
                out.push('\n');
            }
            blank_run = true;
        } else {
            if !first {
                out.push('\n');
            }
            out.push_str(line);
            blank_run = false;
        }
        first = false;
    }
    // Ensure a top-level `# Concepts` heading exists and normalize whatever
    // level-1 heading the model used.
    if let Some(pos) = out.find('\n') {
        let first_line = &out[..pos];
        if first_line.trim_start().starts_with("# ") {
            out.replace_range(..pos, "# Concepts");
        } else {
            out.insert_str(0, "# Concepts\n");
        }
    } else if out.trim_start().starts_with("# ") {
        out.clear();
        out.push_str("# Concepts");
    } else {
        out.insert_str(0, "# Concepts\n");
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Metadata for one captured web source, parsed from the header block of its
/// `sources/web-NN.md` supporting file.
///
/// The header is the `- Key: value` list that
/// `crate::document::render_supporting_file` writes at the top of every web
/// source file (`URL`, `Title`, `Author(s)`, `Published (UTC)`, ...). Values
/// the file does not expose stay empty and are omitted from the rendered
/// `**WebSources:**` block by [`format_concepts_md_with_sources`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebSourceMeta {
    /// 1-based source index parsed from the `web-NN.md` filename. This is the
    /// same number used by `[#N]` citations in `RESEARCH.md`.
    pub index: usize,
    /// Supporting-file basename (e.g. `web-01.md`).
    pub file: String,
    /// Source URL from the `- URL:` header line.
    pub url: String,
    /// Source title from the `- Title:` header line.
    pub title: String,
    /// Author name from the `- Author(s):` header line (empty when unknown).
    pub author: String,
    /// Publication date as `YYYY-MM-DD` parsed from the `- Published (UTC):`
    /// header line (empty when the page exposed no date).
    pub published: String,
}

/// Read every `web-NN.md` file under the item's `sources/` directory and
/// parse its header block into [`WebSourceMeta`] entries, sorted by index.
///
/// Files without a parseable `web-NN` index are skipped. Missing metadata
/// fields remain empty strings.
///
/// # Errors
///
/// Returns [`ResearchIoError::Io`] when the `sources/` directory or one of
/// its files cannot be read.
pub fn load_web_source_metadata(
    research_root: &Path,
    name: &ResearchName,
) -> Result<Vec<WebSourceMeta>, ResearchIoError> {
    static FILE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let file_re =
        FILE_RE.get_or_init(|| regex::Regex::new(r"^web-(\d+)\.md$").expect("valid regex"));

    let sources_dir = ResearchIo::sources_dir(research_root, name);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&sources_dir)? {
        let path = entry?.path();
        let Some(filename) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let Some(caps) = file_re.captures(filename) else {
            continue;
        };
        let index: usize = caps[1].parse().unwrap_or(0);
        if index == 0 {
            continue;
        }
        let body = std::fs::read_to_string(&path)?;
        out.push(parse_web_source_header(&body, index, filename));
    }
    out.sort_by_key(|m| m.index);
    Ok(out)
}

/// Parse the `- Key: value` header lines of a web source supporting file.
///
/// Only the lines before the first ``` fence are considered; the fenced body
/// below the header is the captured page text, not metadata.
fn parse_web_source_header(body: &str, index: usize, filename: &str) -> WebSourceMeta {
    let mut meta = WebSourceMeta {
        index,
        file: filename.to_string(),
        ..WebSourceMeta::default()
    };
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            break;
        }
        let line = line.trim();
        if let Some(v) = line.strip_prefix("- URL: ") {
            meta.url = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("- Title: ") {
            meta.title = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("- Author(s): ") {
            // The supporting-file header uses an em-dash placeholder for
            // unknown authors; treat it as empty so the bullet omits it.
            let v = v.trim();
            meta.author = if v == "\u{2014}" || v == "--" {
                String::new()
            } else {
                v.to_string()
            };
        } else if let Some(v) = line.strip_prefix("- Published (UTC): ") {
            // Normalize the RFC-3339 timestamp to the YYYY-MM-DD form used in
            // `RESEARCH.md` source bullets.
            let v = v.trim();
            meta.published = if v == "\u{2014}" || v == "--" {
                String::new()
            } else {
                v.chars().take(10).collect()
            };
        }
    }
    meta
}

/// Render one `**WebSources:**` bullet in the exact `RESEARCH.md` Findings
/// style: `- [N] Title [Author] — URL (published YYYY-MM-DD)`, with the
/// author and published segments omitted when unknown. Mirrors
/// `crate::document::render_finding_sources`.
fn render_web_source_bullet(meta: &WebSourceMeta) -> String {
    let title = if meta.title.is_empty() {
        meta.url.as_str()
    } else {
        meta.title.as_str()
    };
    let mut out = format!("- [{}] {}", meta.index, title);
    if !meta.author.is_empty() {
        out.push_str(&format!(" [{}]", meta.author));
    }
    out.push_str(" \u{2014} ");
    out.push_str(&meta.url);
    if !meta.published.is_empty() {
        out.push_str(&format!(" (published {})", meta.published));
    }
    out
}

/// Normalize an LLM concept-extraction response and append a
/// `**WebSources:**` block to every concept that cites captured sources.
///
/// Extends [`format_concepts_md`] with two `RESEARCH.md`-aligned passes:
///
/// * **Numbering** — every `## ` heading is renumbered sequentially
///   (`## 1. ...`, `## 2. ...`, ...). Any existing leading number
///   (`## 3. ...`, `## 3 - ...`, `## 3: ...`) is stripped first so a model
///   that miscounts still yields a clean sequential run.
/// * **Source mapping** — each concept section is scanned for `web-NN`
///   filename citations (still accepted from models trained on the older
///   prompt); matching entries from `sources` are rendered as a trailing
///   `**WebSources:**` bullet list in the same format as the `**Sources:**`
///   block under `## Findings` in `RESEARCH.md`. Sections with no recognized
///   citations are left untouched.
/// * **Citation style** — every inline `web-NN` mention in the section body is
///   rewritten to the `[#N]` citation form used by `RESEARCH.md` so both
///   documents share one reference style.
///
/// URLs are linkified to `[url](url)` Markdown form so the companion matches
/// the clickable references in `RESEARCH.md`.
#[must_use]
pub fn format_concepts_md_with_sources(raw: &str, sources: &[WebSourceMeta]) -> String {
    let formatted = format_concepts_md(raw);
    if formatted.lines().count() <= 1 {
        // The placeholder/empty render has no concept sections.
        return formatted;
    }

    static NUM_PREFIX_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static WEB_REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static HASH_REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let num_prefix_re = NUM_PREFIX_RE.get_or_init(|| {
        regex::Regex::new(r"^\d+\s*[.):\-\u{2013}\u{2014}]\s*").expect("valid regex")
    });
    let web_ref_re =
        WEB_REF_RE.get_or_init(|| regex::Regex::new(r"\bweb-(\d+)\b").expect("valid regex"));
    let hash_ref_re =
        HASH_REF_RE.get_or_init(|| regex::Regex::new(r"\[#(\d+)\]").expect("valid regex"));

    let by_index: std::collections::HashMap<usize, &WebSourceMeta> =
        sources.iter().map(|m| (m.index, m)).collect();

    // Pass 1: renumber every `## ` heading sequentially.
    let mut numbered = String::with_capacity(formatted.len() + 64);
    let mut concept_no = 0usize;
    for line in formatted.lines() {
        if line.starts_with("## ") {
            concept_no += 1;
            let label = num_prefix_re.replace(&line[3..], "").trim().to_string();
            numbered.push_str(&format!("## {concept_no}. {label}\n"));
        } else {
            numbered.push_str(line);
            numbered.push('\n');
        }
    }

    // Pass 2: append a `**WebSources:**` block to sections that cite sources.
    let mut sections: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in numbered.lines() {
        if line.starts_with("## ") && !current.is_empty() {
            sections.push(std::mem::take(&mut current));
        }
        current.push(line);
    }
    if !current.is_empty() {
        sections.push(current);
    }

    let mut out = String::with_capacity(numbered.len() + 256);
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for line in section {
            // Rewrite inline `web-NN` citations to the unpadded `[#N]` style
            // used by `RESEARCH.md` so both documents share one format.
            let rewritten = web_ref_re.replace_all(line, |caps: &regex::Captures| {
                let n = caps[1].trim_start_matches('0');
                format!("[#{}]", if n.is_empty() { "0" } else { n })
            });
            out.push_str(&rewritten);
            out.push('\n');
        }
        while out.ends_with("\n\n") {
            out.pop();
        }
        let body = section
            .iter()
            .filter(|l| !l.starts_with("## "))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        let mut indices: Vec<usize> = web_ref_re
            .find_iter(&body)
            .filter_map(|m| m.as_str()[4..].parse().ok())
            .chain(
                hash_ref_re
                    .find_iter(&body)
                    .filter_map(|m| m.as_str()[2..m.as_str().len() - 1].parse().ok()),
            )
            .collect();
        indices.sort_unstable();
        indices.dedup();
        let cited: Vec<&WebSourceMeta> = indices
            .iter()
            .filter_map(|n| by_index.get(n).copied())
            .collect();
        if !cited.is_empty() {
            out.push_str("\n**WebSources:**\n");
            for meta in &cited {
                out.push_str(&render_web_source_bullet(meta));
                out.push('\n');
            }
        }
    }
    if !out.ends_with("\n\n") {
        // Guarantee exactly one blank line between sections.
        while out.ends_with('\n') {
            out.pop();
        }
        out.push_str("\n\n");
    }
    let mut linked = crate::document::linkify_urls(out.trim_end());
    linked.push('\n');
    linked
}

/// Assembled source payload returned by [`build_cluster_payload_sync`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterPayload {
    /// Concatenated markdown text, ready to be inserted into the prompt.
    pub text: String,
    /// Basenames of the source files that contributed to `text`.
    pub files: Vec<String>,
    /// Total bytes in `text` after optional truncation.
    pub total_bytes: usize,
    /// Maximum bytes the payload was allowed to consume.
    pub max_bytes: usize,
    /// `true` when one or more source bodies were truncated to fit the budget.
    pub truncated: bool,
}

/// Convert a token context window into a byte budget for the source payload.
#[must_use]
pub fn estimate_max_payload_bytes(context_window_tokens: usize) -> usize {
    let available = context_window_tokens.saturating_sub(PROMPT_RESERVE_TOKENS);
    available.saturating_mul(BYTES_PER_TOKEN_GUESS)
}

/// Resolve the active model's context window from the provider registry.
///
/// Returns `None` when the model or provider is unknown so callers can fall
/// back to [`DEFAULT_CONTEXT_WINDOW_TOKENS`].
#[must_use]
pub fn resolve_context_window_tokens(
    provider_id: Option<&str>,
    model_id: Option<&str>,
    registry: Option<&ragent_llm::ProviderRegistry>,
) -> Option<usize> {
    let provider_id = provider_id?;
    let model_id = model_id?;
    let registry = registry?;
    registry
        .resolve_model(provider_id, model_id)
        .map(|m| m.context_window)
        .filter(|w| *w > 0)
}

fn find_source_files(sources_dir: &Path) -> Result<Vec<std::path::PathBuf>, ResearchIoError> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(sources_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> String {
    let bytes = s.as_bytes();
    if bytes.len() <= max_bytes {
        return s.to_string();
    }
    let mut cut = max_bytes;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 64);
    out.push_str(&s[..cut]);
    out.push_str("\n\n… _(truncated — source exceeded per-file context budget)_\n");
    out
}

/// Synchronously read all markdown source files and assemble them into a
/// context-window-bounded payload.
///
/// # Errors
///
/// Returns [`ResearchIoError::Io`] when the `sources/` directory cannot be
/// read or a source file cannot be read.
pub fn build_cluster_payload_sync(
    research_root: &Path,
    name: &ResearchName,
    context_window_tokens: Option<usize>,
) -> Result<ClusterPayload, ResearchIoError> {
    let sources_dir = ResearchIo::sources_dir(research_root, name);
    let max_bytes =
        estimate_max_payload_bytes(context_window_tokens.unwrap_or(DEFAULT_CONTEXT_WINDOW_TOKENS));
    let files = find_source_files(&sources_dir)?;

    let mut text = String::new();
    let mut total_bytes = 0;
    let mut truncated = false;
    let mut remaining = max_bytes;

    for path in &files {
        let filename = path_filename(path);
        let header = format!("\n\n--- {} ---\n\n", filename);
        let header_len = header.len();

        let body_budget = remaining.saturating_sub(header_len);
        if body_budget == 0 {
            truncated = true;
            break;
        }

        let body = std::fs::read_to_string(path)?;
        let was_truncated = body.len() > body_budget;
        let body = truncate_to_char_boundary(&body, body_budget);
        truncated |= was_truncated;

        text.push_str(&header);
        text.push_str(&body);
        total_bytes += header_len + body.len();
        remaining = remaining.saturating_sub(header_len + body.len());
    }

    let files = files.into_iter().map(|p| path_filename(&p)).collect();

    Ok(ClusterPayload {
        text,
        files,
        total_bytes,
        max_bytes,
        truncated,
    })
}

fn path_filename(path: &std::path::Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Async wrapper around [`build_cluster_payload_sync`] that offloads the file
/// reads to the blocking task pool.
pub async fn build_cluster_payload(
    research_root: &Path,
    name: &ResearchName,
    context_window_tokens: Option<usize>,
) -> Result<ClusterPayload, ResearchIoError> {
    let root = research_root.to_path_buf();
    let name = name.clone();
    let tokens = context_window_tokens;
    tokio::task::spawn_blocking(move || build_cluster_payload_sync(&root, &name, tokens))
        .await
        .map_err(|e| {
            ResearchIoError::Io(std::io::Error::other(format!(
                "cluster payload task failed: {e}"
            )))
        })?
}

/// Build the full concept-extraction prompt by inserting the assembled source
/// payload into the fixed [`CONCEPT_EXTRACTION_PROMPT_TEMPLATE`].
///
/// The returned string is ready to be dispatched to the active LLM for T-005.
#[must_use]
pub fn build_concept_extraction_prompt(payload: &ClusterPayload) -> String {
    CONCEPT_EXTRACTION_PROMPT_TEMPLATE.replace("[INSERT_DOCUMENTS_HERE]", &payload.text)
}

/// Assemble an in-memory concept-extraction payload from the gathered source
/// bodies of a `/research create` run.
///
/// Unlike [`build_cluster_payload_sync`] (which reads the on-disk
/// `sources/web-NN.md` supporting files), this variant takes the source bodies
/// the synthesis step already loaded, so it works mid-session before any
/// supporting files are written. Each document block is headed
/// `--- [#N] Title ---` where `N` is the 1-based position in the gathered
/// source list — the same number the References Index in `RESEARCH.md` uses —
/// so the model's `[#N]` citations resolve against that index directly.
///
/// Documents are appended in order until `max_bytes` is reached; a document
/// that does not fit is truncated to the remaining budget (matching
/// [`truncate_to_char_boundary`]).
#[must_use]
pub fn build_concepts_payload_from_bodies(
    bodies: &[crate::analysis::SourceBody],
    max_bytes: usize,
) -> String {
    let mut text = String::new();
    let mut remaining = max_bytes;
    for body in bodies {
        let title = if body.title.trim().is_empty() {
            body.path_or_url.as_str()
        } else {
            body.title.as_str()
        };
        let header = format!("\n\n--- [#{}] {} ---\n\n", body.index, title);
        let header_len = header.len();
        let body_budget = remaining.saturating_sub(header_len);
        if body_budget == 0 {
            break;
        }
        let doc = truncate_to_char_boundary(&body.body, body_budget);
        text.push_str(&header);
        text.push_str(&doc);
        remaining = remaining.saturating_sub(header_len + doc.len());
    }
    text
}

/// Normalize a concept-extraction LLM response for embedding as the
/// `## Concepts` section inside `RESEARCH.md`.
///
/// Transformations applied:
///
/// * The `# Concepts` level-1 heading is dropped (the section provides its own
///   `## Concepts` heading in the parent document).
/// * Every `## N. Name` concept heading is demoted to `### N. Name` so it
///   nests correctly under the parent section.
/// * Inline `web-NN` filename citations are rewritten to `[#M]` where `M` is
///   the combined References Index position supplied by `web_index_map`
///   (supporting-file number to 1-based combined index). `[#N]` citations are
///   kept verbatim.
///
/// Returns `None` when the response contains no concept sections, so callers
/// can omit the parent section entirely.
#[must_use]
pub fn concepts_section_for_research<S: std::hash::BuildHasher>(
    raw: &str,
    web_index_map: &std::collections::HashMap<usize, usize, S>,
) -> Option<String> {
    static WEB_REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let web_ref_re =
        WEB_REF_RE.get_or_init(|| regex::Regex::new(r"\bweb-(\d+)\b").expect("valid regex"));

    let mut out = String::with_capacity(raw.len() + 64);
    let mut sections = 0usize;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !out.ends_with("\n\n") && !out.is_empty() {
                out.push('\n');
            }
            continue;
        }
        // Drop the level-1 heading the prompt asks for.
        if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
            continue;
        }
        let line = if let Some(rest) = trimmed.strip_prefix("## ") {
            // Demote the concept heading one level.
            format!("### {rest}")
        } else {
            trimmed.to_string()
        };
        let line = web_ref_re
            .replace_all(&line, |caps: &regex::Captures| {
                let file_no: usize = caps[1].parse().unwrap_or(0);
                match web_index_map.get(&file_no) {
                    Some(combined) => format!("[#{combined}]"),
                    // Unknown web-NN (no combined index): keep the citation
                    // number as-is so it at least matches the cluster-style
                    // numbering rather than dangling as a filename.
                    None => format!("[#{}]", caps[1].trim_start_matches('0')),
                }
            })
            .into_owned();
        if line.starts_with("### ") {
            if sections > 0 {
                out.push('\n');
            }
            sections += 1;
        }
        out.push_str(&line);
        out.push('\n');
    }
    let out = out.trim().to_string();
    if sections == 0 || out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Write the LLM-generated concept extraction response as `CONCEPTS.md` in the
/// research item folder (FR-007).
///
/// The captured `sources/web-NN.md` headers are parsed so the output gains the
/// sequential concept numbering and `**WebSources:**` reference blocks applied
/// by [`format_concepts_md_with_sources`]. If the source headers cannot be
/// read, formatting degrades to plain [`format_concepts_md`] normalization.
///
/// The caller is responsible for the overwrite guard (FR-008); this function
/// will overwrite an existing `CONCEPTS.md` if one is present. Writes are
/// performed atomically via [`ResearchIo::atomic_write`] so readers never see
/// a partially written file (NFR-002).
///
/// # Errors
///
/// Returns [`ResearchIoError::Io`] when the directory or file cannot be
/// written.
pub async fn write_concepts_md(
    research_root: &Path,
    name: &ResearchName,
    content: &str,
) -> Result<PathBuf, ResearchIoError> {
    let path = ResearchIo::concepts_md_path(research_root, name);
    let sources = load_web_source_metadata(research_root, name).unwrap_or_default();
    let formatted = format_concepts_md_with_sources(content, &sources);
    ResearchIo::atomic_write(&path, &formatted).await?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;

    fn tmp_root() -> (tempfile::TempDir, ResearchName) {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let name = ResearchName::new("cluster-test").expect("valid name");
        (dir, name)
    }

    #[test]
    fn estimate_max_payload_bytes_respects_reserve_and_bytes_per_token() {
        assert_eq!(
            estimate_max_payload_bytes(DEFAULT_CONTEXT_WINDOW_TOKENS),
            (DEFAULT_CONTEXT_WINDOW_TOKENS - PROMPT_RESERVE_TOKENS) * BYTES_PER_TOKEN_GUESS
        );
        assert_eq!(estimate_max_payload_bytes(0), 0);
        assert_eq!(estimate_max_payload_bytes(PROMPT_RESERVE_TOKENS / 2), 0);
    }

    #[test]
    fn resolve_context_window_tokens_uses_registry() {
        let registry = ragent_llm::provider::create_default_registry();
        let ctx = resolve_context_window_tokens(
            Some("gemini"),
            Some("gemini-2.0-flash"),
            Some(&registry),
        );
        assert_eq!(ctx, Some(1_048_576));
    }

    #[test]
    fn resolve_context_window_tokens_returns_none_for_missing_model() {
        let registry = ragent_llm::provider::create_default_registry();
        assert!(
            resolve_context_window_tokens(Some("openai"), Some("missing"), Some(&registry))
                .is_none()
        );
        assert!(resolve_context_window_tokens(None, Some("gpt-4o"), Some(&registry)).is_none());
        assert!(resolve_context_window_tokens(Some("openai"), Some("gpt-4o"), None).is_none());
    }

    #[test]
    fn build_payload_reads_and_concatenates_sources() {
        let (dir, name) = tmp_root();
        let sources = dir.path().join("cluster-test/sources");
        std::fs::create_dir_all(&sources).expect("create sources");
        std::fs::write(sources.join("web-01.md"), "First source body.").expect("write");
        std::fs::write(sources.join("web-02.md"), "Second source body.").expect("write");

        let payload = build_cluster_payload_sync(dir.path(), &name, Some(10_000)).expect("build");
        assert_eq!(payload.files, vec!["web-01.md", "web-02.md"]);
        assert!(payload.text.contains("--- web-01.md ---"));
        assert!(payload.text.contains("--- web-02.md ---"));
        assert!(payload.text.contains("First source body."));
        assert!(payload.text.contains("Second source body."));
        assert!(!payload.truncated);
    }

    #[test]
    fn build_payload_truncates_when_total_exceeds_budget() {
        let (dir, name) = tmp_root();
        let sources = dir.path().join("cluster-test/sources");
        std::fs::create_dir_all(&sources).expect("create sources");
        // Two 3 kB files, each larger than the per-file cap derived from a
        // small context window.
        let big = "x".repeat(3_000);
        std::fs::write(sources.join("web-01.md"), &big).expect("write");
        std::fs::write(sources.join("web-02.md"), &big).expect("write");

        // 5k tokens -> available 904 tokens * 4 bytes/token = 3_616 bytes total,
        // so per-file cap is 1_808 bytes and both 3_000-byte files must be truncated.
        let payload = build_cluster_payload_sync(dir.path(), &name, Some(5_000)).expect("build");
        assert!(payload.truncated);
        assert!(payload.total_bytes <= payload.max_bytes + 200);
        assert!(payload.text.contains("truncated"));
    }

    #[test]
    fn build_payload_is_empty_for_no_sources() {
        let (dir, name) = tmp_root();
        std::fs::create_dir_all(dir.path().join("cluster-test/sources")).expect("create sources");
        let payload = build_cluster_payload_sync(dir.path(), &name, None).expect("build");
        assert!(payload.text.is_empty());
        assert!(payload.files.is_empty());
        assert_eq!(
            payload.max_bytes,
            estimate_max_payload_bytes(DEFAULT_CONTEXT_WINDOW_TOKENS)
        );
    }

    #[test]
    fn build_concept_extraction_prompt_includes_fixed_instructions_and_documents() {
        let payload = ClusterPayload {
            text: "\n\n--- web-01.md ---\n\nAlpha is important.\n\n--- web-02.md ---\n\nBeta ties the sources together.".to_string(),
            files: vec!["web-01.md".to_string(), "web-02.md".to_string()],
            total_bytes: 1,
            max_bytes: 2,
            truncated: false,
        };

        let prompt = build_concept_extraction_prompt(&payload);

        assert!(
            prompt.contains("You are an expert data analyst and researcher."),
            "prompt should contain the fixed persona/instructions"
        );
        assert!(
            prompt.contains("## N. Concept Name"),
            "prompt should request numbered level-2 concept headings"
        );
        assert!(
            prompt.contains("**Definition**"),
            "prompt should request bold definition label"
        );
        assert!(
            prompt.contains("**Key Evidence**"),
            "prompt should request bold evidence label"
        );
        assert!(
            prompt.contains("web-01"),
            "prompt should reference the document-header filename form"
        );
        assert!(
            prompt.contains("[#1]"),
            "prompt should ask the model to cite sources with [#N] markers"
        );
        assert!(
            prompt.contains("# Concepts"),
            "prompt should ask for a top-level Concepts heading"
        );
        assert!(
            prompt.contains("up to 20 core concepts"),
            "prompt should cap the output at 20 concepts"
        );
        assert!(
            !prompt.contains("[INSERT_DOCUMENTS_HERE]"),
            "placeholder should be replaced"
        );
        assert!(
            prompt.contains("--- web-01.md ---"),
            "prompt should contain the inserted document separators"
        );
        assert!(prompt.contains("Alpha is important."));
        assert!(prompt.contains("Beta ties the sources together."));
    }

    #[test]
    fn format_concepts_md_normalizes_output() {
        let raw = "\n\n  \n## Alpha\n\n**Definition:** first concept.\n\n**Key Evidence:**\n- appears here\n\n\n\n## Beta\n\n**Definition:** second concept.\n\n  ";
        let out = format_concepts_md(raw);
        assert!(
            out.starts_with("# Concepts\n"),
            "output should begin with # Concepts: {out:?}"
        );
        assert!(out.contains("## Alpha\n"));
        assert!(out.contains("## Beta\n"));
        assert!(out.contains("**Definition:**"));
        assert!(out.contains("- appears here"));
        assert!(
            !out.contains("\n\n\n"),
            "excessive blank lines should be collapsed: {out:?}"
        );
        assert!(out.ends_with('\n'));
        assert!(!out.ends_with("\n\n"));
    }

    #[test]
    fn format_concepts_md_adds_heading_when_missing() {
        let raw = "## Alpha\n\nDefinition of alpha.\n";
        let out = format_concepts_md(raw);
        assert_eq!(out.lines().next().unwrap(), "# Concepts");
        assert!(out.contains("## Alpha"));
    }

    #[test]
    fn format_concepts_md_handles_empty_input() {
        assert_eq!(
            format_concepts_md(""),
            "# Concepts\n\nNo concepts were extracted.\n"
        );
        assert_eq!(
            format_concepts_md("   \n  \n  "),
            "# Concepts\n\nNo concepts were extracted.\n"
        );
    }

    #[test]
    fn build_concept_extraction_prompt_repeats_payload_independently() {
        let payload = ClusterPayload {
            text: "first".to_string(),
            files: vec![],
            total_bytes: 0,
            max_bytes: 0,
            truncated: false,
        };
        let p1 = build_concept_extraction_prompt(&payload);
        let p2 = build_concept_extraction_prompt(&payload);
        assert_eq!(p1, p2, "same payload must produce deterministic prompt");
        assert_eq!(
            p1.matches("first").count(),
            1,
            "payload inserted exactly once"
        );
    }

    #[tokio::test]
    async fn write_concepts_md_creates_file() {
        let (dir, name) = tmp_root();
        std::fs::create_dir_all(dir.path().join("cluster-test")).expect("create item dir");
        let path = write_concepts_md(dir.path(), &name, "# Concepts\n\n- Alpha\n")
            .await
            .expect("write");
        assert_eq!(path, dir.path().join("cluster-test/CONCEPTS.md"));
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content, "# Concepts\n\n- Alpha\n");
    }

    #[tokio::test]
    async fn write_concepts_md_overwrites_existing_file() {
        let (dir, name) = tmp_root();
        let item_dir = dir.path().join("cluster-test");
        std::fs::create_dir_all(&item_dir).expect("create item dir");
        let path = item_dir.join("CONCEPTS.md");
        std::fs::write(&path, "old content").expect("seed old file");
        let returned = write_concepts_md(dir.path(), &name, "new content")
            .await
            .expect("overwrite");
        assert_eq!(returned, path);
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content, "# Concepts\nnew content\n");
    }

    #[tokio::test]
    async fn write_concepts_md_normalizes_content() {
        let (dir, name) = tmp_root();
        std::fs::create_dir_all(dir.path().join("cluster-test")).expect("create item dir");
        let path = write_concepts_md(
            dir.path(),
            &name,
            "\n\n  \n## Gamma\n\n**Definition:** third concept.\n\n\n\n",
        )
        .await
        .expect("write");
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.starts_with("# Concepts\n"));
        // Headings are renumbered sequentially by format_concepts_md_with_sources.
        assert!(content.contains("## 1. Gamma"));
        assert!(!content.contains("\n\n\n"));
        assert!(content.ends_with('\n'));
    }
}
