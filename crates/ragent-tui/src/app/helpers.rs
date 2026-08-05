//! Free-standing helper functions for the TUI app module.

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
    let start = session_id.len().saturating_sub(8);
    session_id[start..].to_string()
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

pub(crate) fn is_discovery_notice(message: &str) -> bool {
    message.starts_with("📋 Instruction File Discovery")
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
    let resolved = if src.starts_with("http://") || src.starts_with("https://") {
        src.to_string()
    } else {
        base_dir.join(src).to_string_lossy().to_string()
    };
    let dims = if src.starts_with("http://") || src.starts_with("https://") {
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
