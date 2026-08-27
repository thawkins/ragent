//! Free-standing helper functions for the TUI app module.
// Clippy complains about `pub(crate)` inside this private module, but CI's
// dead-code lint runs with `-D unreachable_pub`, so `pub` items here fail CI.
// Keep `pub(crate)` and suppress the clippy nursery lint.
#![allow(clippy::redundant_pub_crate)]

#[derive(Debug, Clone, Copy)]
pub(crate) struct MentionSpan {
    pub(crate) at_start: usize,
    pub(crate) token_start: usize,
    pub(crate) token_end: usize,
}

impl MentionSpan {
    pub(crate) fn query<'a>(&self, input: &'a str) -> &'a str {
        &input[self.token_start..self.token_end]
    }
}

pub(crate) fn try_extract_research_code_block(text: &str) -> Option<String> {
    if !text.starts_with("From: /") {
        return None;
    }
    // Opening fence must be a bare triple-backtick line: "\n```\n" preceded by
    // a blank line so we don't accidentally match the closing fence of a
    // language-tagged block earlier in the text.
    let body_start = text.find("\n\n```\n")? + 5;
    // Closing fence may be `\n```\n` (followed by more text) or `\n```\n` at
    // end-of-string.  `\n```` would terminate early on a four-backtick fence,
    // but we only emit triple-backtick fences from this codebase.
    let body_end_off = text[body_start..].find("\n```")?;
    let body_end = body_start + body_end_off;
    let body = &text[body_start..body_end];
    let prefix = &text[..body_start - 5];
    Some(format!("{}\n\n{}", prefix.trim_end(), body))
}

pub(crate) fn parse_swarm_args(args: &str) -> (String, Option<String>) {
    let mut agent_type: Option<String> = None;
    let mut parts = Vec::new();
    let mut tokens = args.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        if token == "--agent" {
            if let Some(next) = tokens.next() {
                agent_type = Some(next.to_string());
            }
        } else {
            parts.push(token);
        }
    }
    let prompt = parts.join(" ");
    (prompt, agent_type)
}

pub(crate) fn short_session_id(session_id: &str) -> String {
    tail8(session_id)
}

/// Truncate a [`RunId`] to a stable short prefix for compact display.
///
/// Returns the last 8 characters of the run id, mirroring
/// [`short_session_id`]. If the run id is shorter than 8 characters the
/// whole value is returned unchanged.
pub(crate) fn short_run_id(run_id: &ragent_types::id::RunId) -> String {
    tail8(run_id.as_str())
}

/// Return the last 8 characters of a string, or the whole string if shorter.
fn tail8(s: &str) -> String {
    let start = s.len().saturating_sub(8);
    s[start..].to_string()
}

/// Resolve the activity-log database path from the main storage database path.
///
/// Thin wrapper around [`ragent_storage::ActivityLog::default_path`] so the
/// TUI and the binary share a single source of truth for the path convention.
pub(crate) fn activity_log_db_path(db_path: &std::path::Path) -> std::path::PathBuf {
    ragent_storage::ActivityLog::default_path(db_path)
}

/// Parse the `<run-id> [--yes]` arguments shared by the destructive `/alog`
/// subcommands (`delete`, `export`).
///
/// On a validation failure the formatted usage / confirmation warning is
/// appended to the UI through `append` and the status bar is set, then
/// `None` is returned so the caller can simply abort.
pub(crate) fn parse_alog_run_id_yes(
    args: &str,
    subcmd: &str,
    mut append: impl FnMut(&str),
) -> Option<String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let has_yes = parts.contains(&"--yes");
    let run_id = parts.iter().find(|p| **p != "--yes").copied();

    let Some(run_id) = run_id else {
        append(&format!(
            "From: /alog {subcmd}\n\n\
             \u{26a0} Missing <run-id> argument.\n\n\
             Usage: `/alog {subcmd} <run-id> --yes`"
        ));
        return None;
    };

    if !has_yes {
        append(&format!(
            "From: /alog {subcmd}\n\n\
             \u{26a0} The `--yes` flag is required to confirm this operation.\n\n\
             To proceed, re-run:\n\
             `/alog {subcmd} <run-id> --yes`"
        ));
        return None;
    }

    Some(run_id.to_string())
}

/// Open the activity log and verify `run_id` exists (via `list_runs`),
/// returning the log handle and the run's event count.
///
/// On failure an error message prefixed `From: /alog {subcmd}` is returned
/// so the caller can surface it directly. Used by the `/alog delete` and
/// `/alog export` handlers, which share this open → verify → count scaffold.
pub(crate) fn open_verified_alog(
    alog_path: &std::path::Path,
    run_id: &ragent_types::id::RunId,
    subcmd: &str,
) -> Result<(ragent_storage::activity_log::ActivityLog, u64), String> {
    use ragent_storage::activity_log::ActivityLog;

    let log = ActivityLog::open(alog_path).map_err(|e| {
        format!(
            "From: /alog {subcmd}\n\n\
             \u{26a0} Failed to open activity log at `{}`: {e}",
            alog_path.display(),
        )
    })?;

    let runs = log.list_runs().map_err(|e| {
        format!(
            "From: /alog {subcmd}\n\n\
             \u{26a0} Failed to list activity-log runs: {e}",
        )
    })?;

    if !runs.iter().any(|r| r == run_id) {
        return Err(format!(
            "From: /alog {subcmd}\n\n\
             \u{26a0} No run with id `{}` was found in the activity log.",
            run_id.as_str(),
        ));
    }

    let count = log.count(run_id).unwrap_or(0);
    Ok((log, count))
}

pub(crate) fn summarise_error(raw: &str) -> String {
    // Try to extract just the human-readable message from common patterns
    // e.g. "LLM call failed: Unknown model: claude-haiku-4.5"
    let cleaned = raw.trim().strip_prefix("LLM call failed: ").unwrap_or(raw);

    if cleaned.contains("not accessible via the /chat/completions endpoint")
        || cleaned.contains("unsupported_api_for_model")
    {
        return "Selected model is not available for chat/completions; use /model and pick a non-Codex chat model".to_string();
    }

    // Truncate to a reasonable length for the status bar
    if cleaned.len() > 120 {
        let mut end = 120;
        while end > 0 && !cleaned.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &cleaned[..end])
    } else {
        cleaned.to_string()
    }
}

/// Remove control characters (except newlines and tabs) and ANSI escape
/// sequences from strings before they are displayed in the TUI.
///
/// Fetched page titles, error messages, and URL body previews may contain
/// arbitrary bytes from the network. Stripping them prevents garbage glyphs
/// such as `%???` from appearing at the start of rendered lines.
/// Sanitize raw text so it is safe to render as Markdown/ANSI in the TUI.
pub fn sanitize_for_display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // ANSI escape: skip the ESC and everything up to (and including) a
            // letter terminator or the canonical ST sequence (`ESC \`).
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() || next == '\u{7}' {
                    break;
                }
            }
            continue;
        }
        // Allow printable chars, whitespace and common Unicode. Drop control
        // chars except newline and tab.
        if c == '\n' || c == '\t' || (!c.is_control()) {
            out.push(c);
        }
    }
    out
}

/// Resolve an image reference to a placeholder description for terminal display.
///
/// Returns a short string such as `[Image: alt text (100x50)]` if the file can
/// be inspected, or `[Image: alt text (path)]` otherwise. Terminal TUI panels
/// cannot render bitmaps directly, so this placeholder keeps the layout useful.
#[must_use]
/// Render an image reference as placeholder text showing dimensions.
pub fn image_dimensions_or_placeholder(alt: &str, src: &str, base_dir: &std::path::Path) -> String {
    let is_url = src.starts_with("http://") || src.starts_with("https://");
    let resolved = if is_url {
        src.to_string()
    } else {
        base_dir.join(src).to_string_lossy().to_string()
    };
    let dims = if is_url {
        None
    } else {
        std::fs::metadata(&resolved).ok().and_then(|_| {
            let p = std::path::Path::new(&resolved);
            image_dimensions(p)
        })
    };
    match dims {
        Some((w, h)) => format!("[Image: {alt} ({w}x{h})]"),
        None => format!("[Image: {alt} ({resolved})]"),
    }
}

fn image_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    // Lightweight dimension parsing for PNG/JPEG headers without decoding pixels.
    let data = std::fs::read(path).ok()?;
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        // PNG IHDR width/height at offsets 16-23 (big-endian).
        if data.len() >= 24 {
            let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            return Some((w, h));
        }
    } else if data.starts_with(b"\xff\xd8") {
        // Minimal JPEG SOF scan for width/height.
        let mut i = 2usize;
        while i + 8 < data.len() {
            if data[i] != 0xff {
                i += 1;
                continue;
            }
            let marker = data[i + 1];
            if marker == 0xd9 {
                break;
            }
            if marker == 0xd8 || marker == 0x00 {
                i += 2;
                continue;
            }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            if matches!(marker, 0xc0..=0xcf) && marker != 0xc4 && marker != 0xc8 && marker != 0xcc {
                if i + 9 < data.len() {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                    return Some((w, h));
                }
            }
            i += len + 2;
        }
    }
    None
}
