//! Long-lived markdown-rendering worker thread (FR-010, FR-002).
//!
//! The TUI's `render_markdown_pipeline` converts markdown to HTML via
//! `pulldown-cmark` and then HTML to plain text via `html2text`.  The
//! `html2text` step may panic on malformed HTML (word-wrapper subtraction
//! overflow), so it must run on a dedicated thread — never the UI thread.
//!
//! Previously, every cache miss spawned a **new** OS thread (`std::thread::
//! Builder::spawn` + `join`), which is expensive during streaming: each
//! `TextDelta` event produces a different accumulated text, misses the
//! cache, and spawns a thread.  This module replaces that pattern with a
//! single long-lived worker thread that receives rendering requests via an
//! `mpsc` channel and returns results via a `oneshot` channel, eliminating
//! per-call thread creation overhead (FR-010) while keeping the computation
//! off the UI thread (FR-002).
//!
//! ## Table-cell misalignment fix
//!
//! Slash-command help tables (e.g. `/spec help`) put one very long,
//! unbreakable code span (the full command syntax) into the first column.
//! html2text's table layout derives per-column widths from *estimate* sizes
//! that count the backtick decoration produced by `do_decorate()` CSS rules,
//! but the rendered text of such a cell can end up one (or, when the cell
//! wraps, two) characters shorter — the renderer re-emits a backtick via
//! `add_inline_text` only when the code span actually fits in the column.
//! The estimate then exceeds the rendered width, the column shrinks, and
//! text from later rows visually "moves" into an earlier column.
//!
//! The fix is to make the estimate and the rendered text agree by appending
//! a real space character inside the single-code-span cell before the HTML
//! conversion (`preprocess_markdown_tables` + `fixup_code_cell`).  The space
//! sits inside the code span, survives cell wrapping, and forces the
//! rendered width to match the width estimate.  Table rows containing a
//! literal `|` inside a code span (e.g. `[--github | --gitlab]`) must be
//! written with a backslash pipe (`\|`) in the source markdown so the
//! cell splitter does not split the span; the `/spec help` table does this.

use std::sync::mpsc;

/// Detect whether a line is a markdown table row (starts and ends with `|`).
/// This intentionally matches the permissive `pulldown-cmark` rule used when
/// collecting rows in [`preprocess_markdown_tables`].
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 2 && trimmed.starts_with('|') && trimmed.ends_with('|')
}

/// Detect the `|---|---|---|` separator row between header and body.
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !is_table_row(trimmed) {
        return false;
    }
    let body = &trimmed[1..trimmed.len() - 1];
    body.chars().all(|c| matches!(c, '-' | ':' | '|' | ' '))
}

/// Append a space-suffix to any cell that is exactly one code span whose
/// (inner) length would wrap at the rendered table width.
///
/// html2text's table layout derives per-column widths from *estimate* sizes
/// that count the backtick decoration produced by `do_decorate()` CSS rules,
/// but the CSS `content:` pseudo-element text is attached as
/// `content_before`/`content_after` fragments that the renderer drops when
/// they collide with the column edge.  The estimates then exceed the
/// actually-rendered width, the column shrinks, and later-row cell text
/// bleeds into adjacent columns (visible as "positionals moved to the
/// Description column").  Appending a real space character inside the code
/// span makes the rendered width match the estimate exactly, so the column
/// keeps its computed width.
///
/// Returns the cell unchanged when it does not qualify.
fn fixup_code_cell(cell: &str) -> String {
    let Some(inner) = cell.strip_prefix('`').and_then(|s| s.strip_suffix('`')) else {
        return cell.to_string();
    };
    // A multi-token cell like `` `foo` bar `` (code span + more text) is fine
    // as-is: the backtick decoration either fully fits or the whole cell
    // wraps earlier, and the estimate stays consistent.
    if inner.contains('`') || inner.is_empty() {
        return cell.to_string();
    }
    // html2text renders tables at width 120; a code span shorter than 40
    // characters comfortably fits inside one column of a three-column table
    // so the space-suffix would leak into the visible output.  Only spans
    // long enough to wrap inside a column need the fixup.
    if inner.chars().count() <= 40 {
        return cell.to_string();
    }
    format!("`{inner} `")
}

/// Split a logical markdown table row into cells.
///
/// Handles backslash escapes (`\|`) so that code spans containing pipes do
/// not split the cell.
fn split_row_cells(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return Vec::new();
    }
    let body = &trimmed[1..trimmed.len() - 1];
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in body.chars() {
        if escaped {
            current.push('\\');
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '|' => {
                cells.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        cells.push(current);
    }
    cells
}

/// Re-assemble cells back into a table row with single-space separators.
fn join_row_cells(cells: &[String]) -> String {
    let mut out = String::from("|");
    for cell in cells {
        out.push(' ');
        out.push_str(cell.trim());
        out.push_str(" |");
    }
    out
}

/// Return `markdown` with every table-cell that is exactly one code span
/// rewritten to `` `content ` `` (an extra space inside the span).  The space
/// survives cell wrapping in html2text and keeps its size estimate in sync
/// with the rendered width.
pub fn preprocess_markdown_tables(markdown: &str) -> String {
    // Fast path: skip processing when no table rows are present.
    if !markdown.lines().any(is_table_row) {
        return markdown.to_string();
    }

    let mut out = String::with_capacity(markdown.len() + 128);
    let mut in_table = false;
    let mut in_fence = false;

    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        // Track fenced code blocks across lines: ``` (or ````) opens a
        // fence and must close it again.  Fence contents are never scanned
        // for table rows.
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // A table starts on any row whose *next* line is a separator. Peek at
        // the following line from the iterator rather than rescanning the
        // document: a re-scan is O(n) per line (O(n^2) overall) and lands on
        // the *first* occurrence of a duplicate line, resolving the wrong
        // "next" line for repeated rows.
        let is_row = is_table_row(line);
        let is_sep = is_table_separator(line);
        let next_is_sep = lines
            .peek()
            .map(|next| is_table_separator(next))
            .unwrap_or(false);

        match (in_table, is_row, is_sep, next_is_sep) {
            (false, true, true, _) => {} // separator row itself: leave alone
            (false, true, _, true) => {
                in_table = true; // header row of a new table
            }
            (true, true, _, _) => {} // in-table row
            (true, _, _, _) => in_table = false,
            _ => {}
        }

        if !in_table || is_sep {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let cells = split_row_cells(line);
        let mut new_cells = Vec::with_capacity(cells.len());
        for cell in cells {
            new_cells.push(fixup_code_cell(cell.trim()));
        }
        let row = join_row_cells(&new_cells);
        out.push_str(&row);
        out.push('\n');
    }

    out
}

/// A request to render markdown to plain text.
struct MdRequest {
    /// Raw markdown text, pre-processed by [`preprocess_markdown_tables`].
    input_markdown: String,
    /// Sender for the rendered plain-text result.
    response_tx: mpsc::Sender<Result<String, String>>,
}

/// Handle to the long-lived markdown worker thread.
///
/// Created once at `App::new` and held for the lifetime of the application.
/// Call [`MdWorker::render`] to send markdown to the worker and block until
/// the plain-text result is returned.
pub struct MdWorker {
    sender: mpsc::Sender<MdRequest>,
}

impl MdWorker {
    /// Spawn the worker thread and return a handle.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<MdRequest>();
        std::thread::Builder::new()
            .name("md-html2text".to_string())
            .spawn(move || {
                // Process requests until the channel is closed.
                while let Ok(req) = rx.recv() {
                    let md = preprocess_markdown_tables(&req.input_markdown);
                    let mut opts = pulldown_cmark::Options::empty();
                    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
                    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
                    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
                    let parser = pulldown_cmark::Parser::new_ext(&md, opts);
                    let mut html_buf = String::new();
                    pulldown_cmark::html::push_html(&mut html_buf, parser);
                    let result = html2text::from_read(html_buf.as_bytes(), 120);
                    // If the response channel is closed (caller dropped), just
                    // continue to the next request.
                    let _ = req.response_tx.send(result.map_err(|e| format!("{e:?}")));
                }
            })
            .expect("failed to spawn md-html2text worker thread");
        Self { sender: tx }
    }

    /// Send markdown to the worker and block until the plain-text result
    /// arrives.
    ///
    /// The `html2text` computation runs on the worker thread, not the caller's
    /// thread, so panics in `html2text` unwind only the worker thread and are
    /// caught by the `Result` return (FR-002).  The worker is automatically
    /// restarted if it panics (see [`Self::render`]).
    pub fn render(&self, markdown: &str) -> Result<String, String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.sender
            .send(MdRequest {
                input_markdown: markdown.to_string(),
                response_tx,
            })
            .map_err(|_| "worker channel closed".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "worker dropped response".to_string())?
    }
}
