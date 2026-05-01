//! Permission checking and access-control primitives.
//!
//! Provides [`PermissionChecker`] which evaluates glob-based
//! [`PermissionRule`]s to decide whether an operation should be allowed,
//! denied, or require interactive confirmation ([`PermissionAction`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// The action to take when a permission rule matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    /// Grant the operation without prompting.
    Allow,
    /// Block the operation without prompting.
    Deny,
    /// Prompt the user for an interactive decision.
    Ask,
}

impl fmt::Display for PermissionAction {
    /// Format the permission action as a lowercase string.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the formatter fails.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Deny => write!(f, "deny"),
            Self::Ask => write!(f, "ask"),
        }
    }
}

/// Represents the type of permission required for a tool operation.
///
/// Standard variants cover common operations (file reads, edits, shell
/// commands, etc.). [`Custom`](Permission::Custom) allows extending the
/// permission system with arbitrary names.
///
/// # Examples
///
/// ```rust
/// use ragent_config::permission::Permission;
///
/// let read = Permission::from("read");
/// assert_eq!(read, Permission::Read);
///
/// let custom = Permission::from("deploy");
/// assert_eq!(custom, Permission::Custom("deploy".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read access to files or resources.
    Read,
    /// Write/edit access to files or resources.
    Edit,
    /// Shell command execution.
    Bash,
    /// Network or web access.
    Web,
    /// Interactive question to the user.
    Question,
    /// Enter a planning phase.
    PlanEnter,
    /// Exit a planning phase.
    PlanExit,
    /// Create or modify to-do items.
    Todo,
    /// Access directories outside the project root.
    ExternalDirectory,
    /// Detect and break infinite processing loops.
    DoomLoop,
    /// User-defined permission type with an arbitrary name.
    Custom(String),
}

impl From<&str> for Permission {
    /// Convert a string slice to a [`Permission`] variant.
    ///
    /// Supports both flat names (`read`, `bash`) and namespaced categories
    /// (`file:read`, `bash:execute`). Namespaced categories are normalized
    /// to their base permission type.
    ///
    /// # Errors
    ///
    /// This function is infallible; unknown strings become [`Permission::Custom`].
    fn from(s: &str) -> Self {
        // Normalize: strip namespace prefix (e.g. "file:read" → "read")
        let normalized = s.split(':').last().unwrap_or(s).to_lowercase();

        match normalized.as_str() {
            "read" => Self::Read,
            "edit" | "write" => Self::Edit,
            "bash" | "execute" => Self::Bash,
            "web" | "fetch" => Self::Web,
            "question" => Self::Question,
            "plan_enter" | "plan" => Self::PlanEnter,
            "plan_exit" => Self::PlanExit,
            "todo" => Self::Todo,
            "external_directory" => Self::ExternalDirectory,
            "doom_loop" => Self::DoomLoop,
            _ => Self::Custom(s.to_string()),
        }
    }
}

impl fmt::Display for Permission {
    /// Format the permission as a lowercase string.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the formatter fails.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Edit => write!(f, "edit"),
            Self::Bash => write!(f, "bash"),
            Self::Web => write!(f, "web"),
            Self::Question => write!(f, "question"),
            Self::PlanEnter => write!(f, "plan_enter"),
            Self::PlanExit => write!(f, "plan_exit"),
            Self::Todo => write!(f, "todo"),
            Self::ExternalDirectory => write!(f, "external_directory"),
            Self::DoomLoop => write!(f, "doom_loop"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl Serialize for Permission {
    /// Serialize the permission as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Permission {
    /// Deserialize a permission from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the deserializer fails or the string type is invalid.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from(s.as_str()))
    }
}

/// A single permission rule mapping a permission name and glob pattern to an
/// [`PermissionAction`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// The permission type this rule applies to.
    pub permission: Permission,
    /// Glob pattern matched against the resource path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Action taken when this rule matches.
    pub action: PermissionAction,
}

/// An ordered list of [`PermissionRule`]s evaluated last-match-wins.
pub type PermissionRuleset = Vec<PermissionRule>;

/// A pending permission request awaiting user resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Unique identifier for this request.
    pub id: String,
    /// Session that originated the request.
    pub session_id: String,
    /// Permission type being requested (e.g. `"read"`, `"bash"`).
    pub permission: String,
    /// Glob patterns describing the target resources.
    pub patterns: Vec<String>,
    // TODO: Replace `Value` with a typed `PermissionMetadata` struct.
    /// Arbitrary JSON metadata attached to the request.
    pub metadata: Value,
    /// Optional tool-call identifier that triggered this request.
    pub tool_call_id: Option<String>,
}

/// The user's response to a [`PermissionRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow this single occurrence only.
    Once,
    /// Allow now and for all future matching requests.
    Always,
    /// Deny the request.
    Deny,
}

/// Evaluates permission rules against a requested permission and resource path.
///
/// Rules are evaluated last-match-wins. Permanent grants recorded via
/// [`record_always`](Self::record_always) take precedence over ruleset entries.
pub struct PermissionChecker {
    ruleset: PermissionRuleset,
    always_grants: HashMap<Permission, Vec<globset::GlobMatcher>>,
}

impl PermissionChecker {
    fn permission_candidates(permission: &str) -> Vec<Permission> {
        let normalized = Permission::from(permission);
        let exact = Permission::Custom(permission.to_string());
        if normalized == exact {
            vec![normalized]
        } else {
            vec![exact, normalized]
        }
    }

    /// Creates a new checker with the given static ruleset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_config::{PermissionAction, PermissionChecker, PermissionRule};
    /// use ragent_config::permission::Permission;
    ///
    /// let rules = vec![PermissionRule {
    ///     permission: Permission::Read,
    ///     pattern: Some("src/**".to_string()),
    ///     action: PermissionAction::Allow,
    /// }];
    /// let checker = PermissionChecker::new(rules);
    /// ```
    #[must_use]
    pub fn new(ruleset: PermissionRuleset) -> Self {
        Self {
            ruleset,
            always_grants: HashMap::new(),
        }
    }

    /// Determines the [`PermissionAction`] for the given `permission` and
    /// resource `path`.
    ///
    /// Permanent grants (from [`record_always`](Self::record_always)) are
    /// checked first; then the static ruleset is evaluated last-match-wins.
    /// Returns [`PermissionAction::Ask`] if no rule matches.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_config::{PermissionAction, PermissionChecker, PermissionRule};
    /// use ragent_config::permission::Permission;
    ///
    /// let checker = PermissionChecker::new(vec![PermissionRule {
    ///     permission: Permission::Read,
    ///     pattern: Some("src/**".to_string()),
    ///     action: PermissionAction::Allow,
    /// }]);
    /// assert_eq!(checker.check("read", "src/main.rs"), PermissionAction::Allow);
    /// assert_eq!(checker.check("read", "secrets.env"), PermissionAction::Ask);
    /// ```
    #[must_use]
    pub fn check(&self, permission: &str, path: &str) -> PermissionAction {
        let targets = Self::permission_candidates(permission);
        let wildcard = Permission::Custom("*".to_string());

        // Check "always" grants first
        for target in &targets {
            if let Some(matchers) = self.always_grants.get(target) {
                for matcher in matchers {
                    if matcher.is_match(path) {
                        return PermissionAction::Allow;
                    }
                }
            }
        }

        // Evaluate ruleset (last matching rule wins, like CSS specificity)
        let mut result = PermissionAction::Ask;
        for rule in &self.ruleset {
            if (targets.contains(&rule.permission) || rule.permission == wildcard)
                && let Some(pattern) = &rule.pattern
                && let Ok(glob) = globset::Glob::new(pattern)
            {
                let matcher = glob.compile_matcher();
                if matcher.is_match(path) {
                    result = rule.action.clone();
                }
            }
        }
        result
    }

    /// Records a permanent "always allow" grant for the given permission and
    /// glob pattern, effective for the lifetime of this checker.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_config::{PermissionAction, PermissionChecker};
    ///
    /// let mut checker = PermissionChecker::new(vec![]);
    /// checker.record_always("edit", "src/**");
    /// assert_eq!(checker.check("edit", "src/lib.rs"), PermissionAction::Allow);
    /// ```
    pub fn record_always(&mut self, permission: &str, pattern: &str) {
        if let Ok(glob) = globset::Glob::new(pattern) {
            let matcher = glob.compile_matcher();
            self.always_grants
                .entry(Permission::from(permission))
                .or_default()
                .push(matcher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_from_flat_names() {
        assert_eq!(Permission::from("read"), Permission::Read);
        assert_eq!(Permission::from("edit"), Permission::Edit);
        assert_eq!(Permission::from("bash"), Permission::Bash);
        assert_eq!(Permission::from("web"), Permission::Web);
    }

    #[test]
    fn test_permission_from_namespaced_categories() {
        // file:read → Read
        assert_eq!(Permission::from("file:read"), Permission::Read);
        // file:write → Edit
        assert_eq!(Permission::from("file:write"), Permission::Edit);
        // bash:execute → Bash
        assert_eq!(Permission::from("bash:execute"), Permission::Bash);
        // network:fetch → Web
        assert_eq!(Permission::from("network:fetch"), Permission::Web);
    }

    #[test]
    fn test_permission_from_case_insensitive() {
        assert_eq!(Permission::from("READ"), Permission::Read);
        assert_eq!(Permission::from("File:Read"), Permission::Read);
        assert_eq!(Permission::from("BASH:EXECUTE"), Permission::Bash);
    }

    #[test]
    fn test_permission_from_aliases() {
        // "write" is an alias for Edit
        assert_eq!(Permission::from("write"), Permission::Edit);
        // "execute" is an alias for Bash
        assert_eq!(Permission::from("execute"), Permission::Bash);
        // "fetch" is an alias for Web
        assert_eq!(Permission::from("fetch"), Permission::Web);
        // "plan" is an alias for PlanEnter
        assert_eq!(Permission::from("plan"), Permission::PlanEnter);
    }

    #[test]
    fn test_permission_checker_with_namespaced_categories() {
        let rules = vec![PermissionRule {
            permission: Permission::Read,
            pattern: Some("**".to_string()),
            action: PermissionAction::Allow,
        }];
        let checker = PermissionChecker::new(rules);

        // Should match even with namespaced category
        assert_eq!(
            checker.check("file:read", "src/main.rs"),
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_permission_checker_with_bash_execute() {
        let rules = vec![PermissionRule {
            permission: Permission::Bash,
            pattern: Some("**".to_string()),
            action: PermissionAction::Deny,
        }];
        let checker = PermissionChecker::new(rules);
        // Should match bash:execute → Bash
        assert_eq!(
            checker.check("bash:execute", "ls -la"),
            PermissionAction::Deny
        );
    }
}
