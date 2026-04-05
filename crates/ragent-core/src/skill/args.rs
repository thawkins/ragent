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
/// use ragent_core::skill::args::substitute_args;
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
/// use ragent_core::skill::args::parse_args;
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    // --- parse_args tests ---

    #[test]
    fn test_parse_empty() {
        assert!(parse_args("").is_empty());
    }

    #[test]
    fn test_parse_single_arg() {
        assert_eq!(parse_args("staging"), vec!["staging"]);
    }

    #[test]
    fn test_parse_multiple_args() {
        assert_eq!(parse_args("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_extra_whitespace() {
        assert_eq!(parse_args("  a   b  c  "), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_double_quoted() {
        assert_eq!(
            parse_args(r#""hello world" foo"#),
            vec!["hello world", "foo"]
        );
    }

    #[test]
    fn test_parse_single_quoted() {
        assert_eq!(parse_args("'hello world' foo"), vec!["hello world", "foo"]);
    }

    #[test]
    fn test_parse_mixed_quotes() {
        assert_eq!(
            parse_args(r#""first arg" 'second arg' third"#),
            vec!["first arg", "second arg", "third"]
        );
    }

    #[test]
    fn test_parse_only_whitespace() {
        assert!(parse_args("   ").is_empty());
    }

    // --- substitute_args tests ---

    #[test]
    fn test_substitute_arguments_all() {
        let result = substitute_args(
            "Deploy $ARGUMENTS now",
            "staging",
            "s1",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "Deploy staging now");
    }

    #[test]
    fn test_substitute_arguments_multi_word() {
        let result = substitute_args(
            "Run: $ARGUMENTS",
            "staging prod",
            "s1",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "Run: staging prod");
    }

    #[test]
    fn test_substitute_indexed_args() {
        let result = substitute_args(
            "Env: $ARGUMENTS[0], Target: $ARGUMENTS[1]",
            "staging us-east-1",
            "s1",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "Env: staging, Target: us-east-1");
    }

    #[test]
    fn test_substitute_indexed_out_of_bounds() {
        let result = substitute_args(
            "Arg: $ARGUMENTS[5]",
            "only-one",
            "s1",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "Arg: ");
    }

    #[test]
    fn test_substitute_positional_shorthand() {
        let result = substitute_args(
            "First: $0, Second: $1",
            "alpha beta",
            "s1",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "First: alpha, Second: beta");
    }

    #[test]
    fn test_substitute_positional_out_of_bounds() {
        let result = substitute_args("Missing: $3", "a b", "s1", Path::new("/skills/deploy"));
        assert_eq!(result, "Missing: ");
    }

    #[test]
    fn test_substitute_session_id() {
        let result = substitute_args(
            "Session: ${RAGENT_SESSION_ID}",
            "",
            "my-session-42",
            Path::new("/skills/deploy"),
        );
        assert_eq!(result, "Session: my-session-42");
    }

    #[test]
    fn test_substitute_skill_dir() {
        let result = substitute_args(
            "Dir: ${RAGENT_SKILL_DIR}",
            "",
            "s1",
            Path::new("/project/.ragent/skills/deploy"),
        );
        assert_eq!(result, "Dir: /project/.ragent/skills/deploy");
    }

    #[test]
    fn test_substitute_all_variable_types() {
        let result = substitute_args(
            "All: $ARGUMENTS, First: $0, Indexed: $ARGUMENTS[1], Session: ${RAGENT_SESSION_ID}, Dir: ${RAGENT_SKILL_DIR}",
            "foo bar",
            "sess-99",
            Path::new("/my/skills/test"),
        );
        assert_eq!(
            result,
            "All: foo bar, First: foo, Indexed: bar, Session: sess-99, Dir: /my/skills/test"
        );
    }

    #[test]
    fn test_substitute_no_placeholders() {
        let result = substitute_args(
            "Just plain text with no variables",
            "args",
            "s1",
            Path::new("/skills"),
        );
        assert_eq!(result, "Just plain text with no variables");
    }

    #[test]
    fn test_substitute_empty_args() {
        let result = substitute_args(
            "Deploy $ARGUMENTS here, first: $0",
            "",
            "s1",
            Path::new("/skills"),
        );
        assert_eq!(result, "Deploy  here, first: ");
    }

    #[test]
    fn test_substitute_quoted_args() {
        let result = substitute_args(
            "Message: $0, Target: $1",
            r#""hello world" production"#,
            "s1",
            Path::new("/skills"),
        );
        assert_eq!(result, "Message: hello world, Target: production");
    }

    #[test]
    fn test_substitute_dollar_not_variable() {
        let result = substitute_args("Price is $50 dollars", "args", "s1", Path::new("/skills"));
        // $5 matches positional $5 (out of bounds → empty), "0" stays
        // Actually $50 is parsed as index 50, which is out of bounds
        assert_eq!(result, "Price is  dollars");
    }

    #[test]
    fn test_substitute_preserves_multiline() {
        let body = "Line 1: $0\n\nLine 3: $1\n\n## Section\n\n$ARGUMENTS";
        let result = substitute_args(body, "alpha beta", "s1", Path::new("/skills"));
        assert_eq!(
            result,
            "Line 1: alpha\n\nLine 3: beta\n\n## Section\n\nalpha beta"
        );
    }

    #[test]
    fn test_substitute_double_digit_index() {
        let args_str = "a0 a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11";
        let result = substitute_args("Tenth: $10, Eleventh: $11", args_str, "s1", Path::new("/s"));
        assert_eq!(result, "Tenth: a10, Eleventh: a11");
    }

    #[test]
    fn test_substitute_skill_dir_with_pathbuf() {
        let dir = PathBuf::from("/home/user/.ragent/skills/my-skill");
        let result = substitute_args("Script: ${RAGENT_SKILL_DIR}/scripts/run.sh", "", "s1", &dir);
        assert_eq!(
            result,
            "Script: /home/user/.ragent/skills/my-skill/scripts/run.sh"
        );
    }
}
