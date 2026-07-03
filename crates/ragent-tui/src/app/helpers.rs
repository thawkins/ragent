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
