//! Bundled skill definitions that ship with ragent.
//!
//! These skills are always available and have [`SkillScope::Bundled`] priority,
//! meaning they can be overridden by personal or project skills with the same
//! name.

use super::{SkillInfo, SkillScope};
use std::collections::HashMap;
use std::path::PathBuf;

/// Creates a default `SkillInfo` struct with common bundled skill fields.
///
/// # Arguments
///
/// * `name` - The skill identifier (e.g., "simplify")
/// * `description` - Human-readable description
/// * `argument_hint` - Optional usage hint (e.g., "[output_path]")
/// * `disable_model_invocation` - If true, model cannot invoke this skill
/// * `allowed_tools` - Tools the skill may use
/// * `body` - The instruction body
///
/// # Examples
///
/// ```
/// use ragent_core::skill::bundled::make_bundled_skill;
///
/// let skill = make_bundled_skill(
///     "test",
///     "Test skill",
///     None,
///     false,
///     vec!["read".to_string()],
///     "Do something",
/// );
/// assert_eq!(skill.name, "test");
/// ```
pub fn make_bundled_skill(
    name: &str,
    description: &str,
    argument_hint: Option<&str>,
    disable_model_invocation: bool,
    allowed_tools: Vec<String>,
    body: &str,
) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        description: Some(description.to_string()),
        argument_hint: argument_hint.map(|h| h.to_string()),
        disable_model_invocation,
        user_invocable: true,
        allowed_tools,
        model: None,
        context: None,
        agent: None,
        hooks: None,
        license: None,
        compatibility: None,
        metadata: HashMap::new(),
        source_path: PathBuf::new(),
        skill_dir: PathBuf::new(),
        scope: SkillScope::Bundled,
        body: body.to_string(),
    }
}

/// Returns the set of skills bundled with ragent.
///
/// Bundled skills include:
/// - `/simplify [output_path]` — Reviews recently changed files for quality improvements
/// - `/batch` — Orchestrates large-scale parallel changes
/// - `/debug` — Troubleshoots the current session via debug logs
/// - `/loop` — Runs a prompt repeatedly on an interval
///
/// # Examples
///
/// ```
/// use ragent_core::skill::bundled::bundled_skills;
///
/// let skills = bundled_skills();
/// assert_eq!(skills.len(), 4);
///
/// let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
/// assert!(names.contains(&"simplify"));
/// assert!(names.contains(&"batch"));
/// assert!(names.contains(&"debug"));
/// assert!(names.contains(&"loop"));
/// ```
pub fn bundled_skills() -> Vec<SkillInfo> {
    vec![
        simplify_skill(),
        batch_skill(),
        debug_skill(),
        loop_skill(),
    ]
}

/// `/simplify [output_path]` — Reviews recently changed files for code quality, reuse,
/// and efficiency issues. Optionally saves summary to the specified file path.
fn simplify_skill() -> SkillInfo {
    make_bundled_skill(
        "simplify",
        "Reviews recently changed files for code quality, reuse, and efficiency issues",
        Some("[output_path]"),
        false,
        vec![
            "bash".to_string(),
            "read".to_string(),
            "grep".to_string(),
            "glob".to_string(),
            "create".to_string(),
            "write".to_string(),
        ],
        SIMPLIFY_BODY,
    )
}

/// `/batch <instruction>` — Orchestrates large-scale parallel changes
/// across a codebase.
fn batch_skill() -> SkillInfo {
    make_bundled_skill(
        "batch",
        "Orchestrates large-scale parallel changes across a codebase",
        Some("<instruction>"),
        true,
        vec![
            "bash".to_string(),
            "read".to_string(),
            "edit".to_string(),
            "create".to_string(),
            "grep".to_string(),
            "glob".to_string(),
        ],
        BATCH_BODY,
    )
}

/// `/debug [description]` — Troubleshoots the current session by reading
/// debug logs.
fn debug_skill() -> SkillInfo {
    make_bundled_skill(
        "debug",
        "Troubleshoots current session by reading debug logs",
        Some("[description]"),
        false,
        vec![
            "bash".to_string(),
            "read".to_string(),
            "grep".to_string(),
        ],
        DEBUG_BODY,
    )
}

/// `/loop [interval] <prompt>` — Runs a prompt repeatedly on an interval
/// for scheduled tasks.
fn loop_skill() -> SkillInfo {
    make_bundled_skill(
        "loop",
        "Runs a prompt repeatedly on an interval (scheduled tasks)",
        Some("[interval] <prompt>"),
        true,
        vec![
            "bash".to_string(),
            "read".to_string(),
        ],
        LOOP_BODY,
    )
}

// ── Skill instruction bodies ────────────────────────────────────

const SIMPLIFY_BODY: &str = "\
Review recently changed files in this project for code quality improvements.

**Output path (if provided): $ARGUMENTS**

Steps:
1. Run `git diff --name-only HEAD~3` to find recently changed files
2. Read each changed file
3. Look for:
   - Code duplication that could be extracted into shared functions
   - Overly complex logic that could be simplified
   - Missing error handling
   - Performance inefficiencies (unnecessary allocations, redundant operations)
   - Dead code or unused imports
4. For each issue found, explain the problem and suggest a concrete fix
5. Apply the fixes if they are safe and straightforward

Focus on substance over style — ignore formatting and naming preferences.

**IMPORTANT - Output file requirement:**
Check the output path above. If it contains a file path (not empty/blank), you MUST:
1. Create a markdown summary document with all your findings
2. Use the `write` tool to save the summary to that EXACT path
3. Confirm to the user that the file was saved

Example: If output path shows 'docs/review.md', save findings to 'docs/review.md'.";


const BATCH_BODY: &str = "\
Apply the following instruction across all matching files in the codebase:

$ARGUMENTS

Steps:
1. Use `grep` and `glob` to identify all files that need changes
2. Group files by the type of change needed
3. For each file:
   a. Read the current content
   b. Apply the requested change
   c. Verify the change is correct
4. After all changes, run any available linters or tests to verify nothing is broken
5. Summarize all changes made

Be thorough — ensure every matching file is updated consistently.";

const DEBUG_BODY: &str = "\
Troubleshoot the current issue by examining debug logs and session state.

$ARGUMENTS

Steps:
1. Check recent command output and error messages in the session
2. Run `cat` or `tail` on relevant log files if available
3. Look for:
   - Error messages and stack traces
   - Configuration issues
   - Missing dependencies or files
   - Permission problems
   - Network or connectivity errors
4. Identify the root cause
5. Suggest or apply a fix

If a description of the problem was provided, focus the investigation on that area.";

const LOOP_BODY: &str = "\
Run the following prompt repeatedly on the specified interval.

$ARGUMENTS

Parse the arguments:
- If the first argument looks like a duration (e.g. `5m`, `30s`, `1h`), use it as the interval
- Otherwise default to a 5-minute interval
- The remaining text is the prompt to execute each iteration

For each iteration:
1. Execute the prompt
2. Report the result
3. Wait for the specified interval before the next iteration

Continue until cancelled by the user (ESC).

Note: This skill provides the instruction framework. The actual scheduling \
loop must be implemented by the agent runtime.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_skills_count() {
        let skills = bundled_skills();
        assert_eq!(skills.len(), 4);
    }

    #[test]
    fn test_bundled_skill_names() {
        let skills = bundled_skills();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"simplify"));
        assert!(names.contains(&"batch"));
        assert!(names.contains(&"debug"));
        assert!(names.contains(&"loop"));
    }

    #[test]
    fn test_bundled_skills_scope() {
        for skill in bundled_skills() {
            assert_eq!(
                skill.scope,
                SkillScope::Bundled,
                "Skill '{}' should have Bundled scope",
                skill.name
            );
        }
    }

    #[test]
    fn test_bundled_skills_have_descriptions() {
        for skill in bundled_skills() {
            assert!(
                skill.description.is_some(),
                "Skill '{}' should have a description",
                skill.name
            );
        }
    }

    #[test]
    fn test_bundled_skills_user_invocable() {
        for skill in bundled_skills() {
            assert!(
                skill.user_invocable,
                "Skill '{}' should be user-invocable",
                skill.name
            );
        }
    }

    #[test]
    fn test_simplify_skill() {
        let skill = simplify_skill();
        assert_eq!(skill.name, "simplify");
        assert!(skill.body.contains("git diff"));
        assert!(!skill.disable_model_invocation);
        assert_eq!(skill.argument_hint.as_deref(), Some("[output_path]"));
    }

    #[test]
    fn test_batch_skill() {
        let skill = batch_skill();
        assert_eq!(skill.name, "batch");
        assert!(skill.body.contains("$ARGUMENTS"));
        assert!(skill.disable_model_invocation, "batch is user-only");
        assert_eq!(skill.argument_hint.as_deref(), Some("<instruction>"));
    }

    #[test]
    fn test_debug_skill() {
        let skill = debug_skill();
        assert_eq!(skill.name, "debug");
        assert!(skill.body.contains("$ARGUMENTS"));
        assert!(!skill.disable_model_invocation);
        assert_eq!(skill.argument_hint.as_deref(), Some("[description]"));
    }

    #[test]
    fn test_loop_skill() {
        let skill = loop_skill();
        assert_eq!(skill.name, "loop");
        assert!(skill.body.contains("$ARGUMENTS"));
        assert!(skill.disable_model_invocation, "loop is user-only");
        assert_eq!(
            skill.argument_hint.as_deref(),
            Some("[interval] <prompt>")
        );
    }

    #[test]
    fn test_bundled_skills_have_nonempty_bodies() {
        for skill in bundled_skills() {
            assert!(
                !skill.body.is_empty(),
                "Skill '{}' should have a non-empty body",
                skill.name
            );
        }
    }

    #[test]
    fn test_bundled_skills_have_allowed_tools() {
        for skill in bundled_skills() {
            assert!(
                !skill.allowed_tools.is_empty(),
                "Skill '{}' should have at least one allowed tool",
                skill.name
            );
            assert!(
                skill.allowed_tools.contains(&"bash".to_string()),
                "Skill '{}' should allow bash",
                skill.name
            );
        }
    }
}
