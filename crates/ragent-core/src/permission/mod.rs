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
/// use ragent_core::permission::Permission;
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
    /// # Errors
    ///
    /// This function is infallible; unknown strings become [`Permission::Custom`].
    fn from(s: &str) -> Self {
        match s {
            "read" => Self::Read,
            "edit" => Self::Edit,
            "bash" => Self::Bash,
            "web" => Self::Web,
            "question" => Self::Question,
            "plan_enter" => Self::PlanEnter,
            "plan_exit" => Self::PlanExit,
            "todo" => Self::Todo,
            "external_directory" => Self::ExternalDirectory,
            "doom_loop" => Self::DoomLoop,
            other => Self::Custom(other.to_string()),
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
    pub pattern: String,
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
    /// Creates a new checker with the given static ruleset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_core::permission::{PermissionChecker, PermissionRule, PermissionAction, Permission};
    ///
    /// let rules = vec![PermissionRule {
    ///     permission: Permission::Read,
    ///     pattern: "src/**".to_string(),
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
    /// use ragent_core::permission::{PermissionChecker, PermissionRule, PermissionAction, Permission};
    ///
    /// let checker = PermissionChecker::new(vec![PermissionRule {
    ///     permission: Permission::Read,
    ///     pattern: "src/**".to_string(),
    ///     action: PermissionAction::Allow,
    /// }]);
    /// assert_eq!(checker.check("read", "src/main.rs"), PermissionAction::Allow);
    /// assert_eq!(checker.check("read", "secrets.env"), PermissionAction::Ask);
    /// ```
    #[must_use]
    pub fn check(&self, permission: &str, path: &str) -> PermissionAction {
        let target = Permission::from(permission);
        let wildcard = Permission::Custom("*".to_string());

        // Check "always" grants first
        if let Some(matchers) = self.always_grants.get(&target) {
            for matcher in matchers {
                if matcher.is_match(path) {
                    return PermissionAction::Allow;
                }
            }
        }

        // Evaluate ruleset (last matching rule wins, like CSS specificity)
        let mut result = PermissionAction::Ask;
        for rule in &self.ruleset {
            if (rule.permission == target || rule.permission == wildcard)
                && let Ok(glob) = globset::Glob::new(&rule.pattern)
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
    /// use ragent_core::permission::{PermissionChecker, PermissionAction};
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
