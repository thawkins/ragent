//! Bundled skill definitions that ship with ragent.
//!
//! These skills are always available and have [`SkillScope::Bundled`] priority,
//! meaning they can be overridden by personal or project skills with the same
//! name.

use super::{SkillInfo, SkillScope};

/// Returns the set of skills bundled with ragent.
///
/// Bundled skills include:
/// - `/simplify` — Reviews recently changed files for quality improvements
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

/// `/simplify` — Reviews recently changed files for code quality, reuse,
/// and efficiency issues.
fn simplify_skill() -> SkillInfo {
    SkillInfo {
        name: "simplify".to_string(),
        description: Some(
            "Reviews recently changed files for code quality, reuse, and efficiency issues"
                .to_string(),
        ),
        argument_hint: None,
        disable_model_invocation: false,
        user_invocable: true,
        allowed_tools: vec![
            "bash".to_string(),
            "read".to_string(),
            "grep".to_string(),
            "glob".to_string(),
        ],
        model: None,
        context: None,
        agent: None,
        hooks: None,
        license: None,
        compatibility: None,
        metadata: std::collections::HashMap::new(),
        source_path: std::path::PathBuf::new(),
        skill_dir: std::path::PathBuf::new(),
        scope: SkillScope::Bundled,
        body: SIMPLIFY_BODY.to_string(),
    }
}

/// `/batch <instruction>` — Orchestrates large-scale parallel changes
/// across a codebase.
fn batch_skill() -> SkillInfo {
    SkillInfo {
        name: "batch".to_string(),
        description: Some(
            "Orchestrates large-scale parallel changes across a codebase".to_string(),
        ),
        argument_hint: Some("<instruction>".to_string()),
        disable_model_invocation: true,
        user_invocable: true,
        allowed_tools: vec![
            "bash".to_string(),
            "read".to_string(),
            "edit".to_string(),
            "create".to_string(),
            "grep".to_string(),
            "glob".to_string(),
        ],
        model: None,
        context: None,
        agent: None,
        hooks: None,
        license: None,
        compatibility: None,
        metadata: std::collections::HashMap::new(),
        source_path: std::path::PathBuf::new(),
        skill_dir: std::path::PathBuf::new(),
        scope: SkillScope::Bundled,
        body: BATCH_BODY.to_string(),
    }
}

/// `/debug [description]` — Troubleshoots the current session by reading
/// debug logs.
fn debug_skill() -> SkillInfo {
    SkillInfo {
        name: "debug".to_string(),
        description: Some(
            "Troubleshoots current session by reading debug logs".to_string(),
        ),
        argument_hint: Some("[description]".to_string()),
        disable_model_invocation: false,
        user_invocable: true,
        allowed_tools: vec![
            "bash".to_string(),
            "read".to_string(),
            "grep".to_string(),
        ],
        model: None,
        context: None,
        agent: None,
        hooks: None,
        license: None,
        compatibility: None,
        metadata: std::collections::HashMap::new(),
        source_path: std::path::PathBuf::new(),
        skill_dir: std::path::PathBuf::new(),
        scope: SkillScope::Bundled,
        body: DEBUG_BODY.to_string(),
    }
}

/// `/loop [interval] <prompt>` — Runs a prompt repeatedly on an interval
/// for scheduled tasks.
fn loop_skill() -> SkillInfo {
    SkillInfo {
        name: "loop".to_string(),
        description: Some(
            "Runs a prompt repeatedly on an interval (scheduled tasks)".to_string(),
        ),
        argument_hint: Some("[interval] <prompt>".to_string()),
        disable_model_invocation: true,
        user_invocable: true,
        allowed_tools: vec![
            "bash".to_string(),
            "read".to_string(),
        ],
        model: None,
        context: None,
        agent: None,
        hooks: None,
        license: None,
        compatibility: None,
        metadata: std::collections::HashMap::new(),
        source_path: std::path::PathBuf::new(),
        skill_dir: std::path::PathBuf::new(),
        scope: SkillScope::Bundled,
        body: LOOP_BODY.to_string(),
    }
}

// ── Skill instruction bodies ────────────────────────────────────

const SIMPLIFY_BODY: &str = "\
Review recently changed files in this project for code quality improvements.

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

Focus on substance over style — ignore formatting and naming preferences.";

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
        assert!(skill.argument_hint.is_none());
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
