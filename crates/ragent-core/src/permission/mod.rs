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
    Allow,
    Deny,
    Ask,
}

impl fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionAction::Allow => write!(f, "allow"),
            PermissionAction::Deny => write!(f, "deny"),
            PermissionAction::Ask => write!(f, "ask"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    Read,
    Edit,
    Bash,
    Web,
    Question,
    PlanEnter,
    PlanExit,
    Todo,
    ExternalDirectory,
    DoomLoop,
    Custom(String),
}

impl From<&str> for Permission {
    fn from(s: &str) -> Self {
        match s {
            "read" => Permission::Read,
            "edit" => Permission::Edit,
            "bash" => Permission::Bash,
            "web" => Permission::Web,
            "question" => Permission::Question,
            "plan_enter" => Permission::PlanEnter,
            "plan_exit" => Permission::PlanExit,
            "todo" => Permission::Todo,
            "external_directory" => Permission::ExternalDirectory,
            "doom_loop" => Permission::DoomLoop,
            other => Permission::Custom(other.to_string()),
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::Read => write!(f, "read"),
            Permission::Edit => write!(f, "edit"),
            Permission::Bash => write!(f, "bash"),
            Permission::Web => write!(f, "web"),
            Permission::Question => write!(f, "question"),
            Permission::PlanEnter => write!(f, "plan_enter"),
            Permission::PlanExit => write!(f, "plan_exit"),
            Permission::Todo => write!(f, "todo"),
            Permission::ExternalDirectory => write!(f, "external_directory"),
            Permission::DoomLoop => write!(f, "doom_loop"),
            Permission::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl Serialize for Permission {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Permission::from(s.as_str()))
    }
}

/// A single permission rule mapping a permission name and glob pattern to an
/// [`PermissionAction`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: Permission,
    pub pattern: String,
    pub action: PermissionAction,
}

/// An ordered list of [`PermissionRule`]s evaluated last-match-wins.
pub type PermissionRuleset = Vec<PermissionRule>;

/// A pending permission request awaiting user resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub session_id: String,
    pub permission: String,
    pub patterns: Vec<String>,
    // TODO: Replace `Value` with a typed `PermissionMetadata` struct.
    pub metadata: Value,
    pub tool_call_id: Option<String>,
}

/// The user's response to a [`PermissionRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Once,
    Always,
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
