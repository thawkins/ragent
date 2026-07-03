//! Argument substitution for skill bodies.
//!
//! When a skill is invoked (e.g. `/deploy staging prod`), the arguments are
//! substituted into the skill body using placeholder variables:
//!
//! | Variable              | Replacement                              |
//! |-----------------------|------------------------------------------|
//! | `$ARGUMENTS`          | All arguments as a single string         |
//! | `$ARGUMENTS[N]`       | Specific argument by 0-based index       |
//! | `$N` (e.g. `$0`)      | Shorthand for `$ARGUMENTS[N]`            |
//! | `${RAGENT_SESSION_ID}`| Current session ID                       |
//! | `${RAGENT_SKILL_DIR}` | Directory containing the skill's SKILL.md|

use std::path::Path;

/// Substitute argument and environment placeholders in a skill body.
///
/// # Arguments
///
/// * `body` — The raw skill body text (markdown after frontmatter).
/// * `args` — The raw argument string passed when invoking the skill
///   (e.g. for `/deploy staging`, this is `"staging"`).
/// * `session_id` — The current ragent session identifier.
/// * `skill_dir` — Absolute path to the directory containing SKILL.md.
///
/// # Examples
///
/// ```
/// use ragent_agent::skill::args::substitute_args;
/// use std::path::Path;
///
/// let body = "Deploy $ARGUMENTS to $0 environment";
/// let result = substitute_args(body, "staging", "sess-123", Path::new("/skills/deploy"));
/// assert_eq!(result, "Deploy staging to staging environment");
/// ```
#[must_use]
pub fn substitute_args(body: &str, args: &str, session_id: &str, skill_dir: &Path) -> String {
    let parsed_args = parse_args(args);
    let mut result = body.to_string();

    // Order matters: replace longer patterns first to avoid partial matches.
    // 1. ${RAGENT_SESSION_ID} and ${RAGENT_SKILL_DIR} (braced env vars)
    result = result.replace("${RAGENT_SESSION_ID}", session_id);
    result = result.replace("${RAGENT_SKILL_DIR}", &skill_dir.display().to_string());

    // 2. $ARGUMENTS[N] — indexed argument access (must come before $ARGUMENTS)
    result = substitute_indexed_args(&result, &parsed_args);

    // 3. $ARGUMENTS — all arguments as a single string
    result = result.replace("$ARGUMENTS", args);

    // 4. $N shorthand — bare positional references ($0, $1, etc.)
    result = substitute_positional_shorthand(&result, &parsed_args);

    result
}

/// Parse a raw argument string into individual arguments.
///
/// Supports:
/// - Whitespace-separated tokens: `staging prod` → `["staging", "prod"]`
/// - Double-quoted strings: `"hello world" foo` → `["hello world", "foo"]`
/// - Single-quoted strings: `'hello world' foo` → `["hello world", "foo"]`
/// - Empty string returns an empty vec
///
/// # Errors
///
/// This function does not return errors. Malformed quotes (e.g., unclosed quotes)
/// are handled by consuming characters until the end of the input.
///
/// # Examples
///
/// ```
/// use ragent_agent::skill::args::parse_args;
///
/// assert_eq!(parse_args("staging"), vec!["staging"]);
/// assert_eq!(parse_args("a b c"), vec!["a", "b", "c"]);
/// assert_eq!(parse_args(r#""hello world" foo"#), vec!["hello world", "foo"]);
/// assert_eq!(parse_args(""), Vec::<String>::new());
/// ```
#[must_use]
pub fn parse_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch == '"' || ch == '\'' {
            let quote = ch;
            chars.next(); // consume opening quote
            let mut arg = String::new();
            for c in chars.by_ref() {
                if c == quote {
                    break;
                }
                arg.push(c);
            }
            args.push(arg);
        } else {
            let mut arg = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                arg.push(c);
                chars.next();
            }
            args.push(arg);
        }
    }

    args
}

/// Replace `$ARGUMENTS[N]` patterns with the Nth argument.
///
/// If the index is out of bounds, the placeholder is replaced with an empty string.
fn substitute_indexed_args(body: &str, args: &[String]) -> String {
    let mut result = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            // Check for $ARGUMENTS[N]
            let rest: String = chars.clone().collect();
            if let Some(stripped) = rest.strip_prefix("ARGUMENTS[")
                && let Some(bracket_pos) = stripped.find(']')
            {
                let index_str = &stripped[..bracket_pos];
                if let Ok(idx) = index_str.parse::<usize>() {
                    let replacement = args.get(idx).map_or("", String::as_str);
                    result.push_str(replacement);
                    // Consume "ARGUMENTS[N]"
                    for _ in 0..=("ARGUMENTS[".len() + bracket_pos) {
                        chars.next();
                    }
                    continue;
                }
            }
            result.push(ch);
        } else {
            result.push(ch);
        }
    }

    result
}

/// Replace `$N` shorthand patterns (e.g. `$0`, `$1`, `$12`) with the Nth argument.
///
/// Only matches `$` followed by one or more digits that are NOT preceded by
/// `ARGUMENTS[` (those are handled separately). A `$` followed by non-digit
/// characters is left as-is.
///
/// If the index is out of bounds, the placeholder is replaced with an empty string.
fn substitute_positional_shorthand(body: &str, args: &[String]) -> String {
    let mut result = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            // Collect all consecutive digits
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let index_str = &body[start..end];
            if let Ok(idx) = index_str.parse::<usize>() {
                let replacement = args.get(idx).map_or("", String::as_str);
                result.push_str(replacement);
                i = end;
                continue;
            }
        }
        result.push(body[i..].chars().next().unwrap_or(' '));
        i += body[i..].chars().next().map_or(1, char::len_utf8);
    }

    result
}


#[cfg(test)]
#[path = "../../tests/inline/skill_args.rs"]
mod tests_tests;
