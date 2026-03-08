use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: PermissionAction,
}

pub type PermissionRuleset = Vec<PermissionRule>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub session_id: String,
    pub permission: String,
    pub patterns: Vec<String>,
    pub metadata: Value,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Once,
    Always,
    Deny,
}

pub struct PermissionChecker {
    ruleset: PermissionRuleset,
    always_grants: HashMap<String, Vec<globset::GlobMatcher>>,
}

impl PermissionChecker {
    pub fn new(ruleset: PermissionRuleset) -> Self {
        Self {
            ruleset,
            always_grants: HashMap::new(),
        }
    }

    pub fn check(&self, permission: &str, path: &str) -> PermissionAction {
        // Check "always" grants first
        if let Some(matchers) = self.always_grants.get(permission) {
            for matcher in matchers {
                if matcher.is_match(path) {
                    return PermissionAction::Allow;
                }
            }
        }

        // Evaluate ruleset (last matching rule wins, like CSS specificity)
        let mut result = PermissionAction::Ask;
        for rule in &self.ruleset {
            if rule.permission == permission || rule.permission == "*" {
                if let Ok(glob) = globset::Glob::new(&rule.pattern) {
                    let matcher = glob.compile_matcher();
                    if matcher.is_match(path) {
                        result = rule.action.clone();
                    }
                }
            }
        }
        result
    }

    pub fn record_always(&mut self, permission: &str, pattern: &str) {
        if let Ok(glob) = globset::Glob::new(pattern) {
            let matcher = glob.compile_matcher();
            self.always_grants
                .entry(permission.to_string())
                .or_default()
                .push(matcher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_action_is_ask() {
        let checker = PermissionChecker::new(vec![]);
        assert_eq!(checker.check("file:read", "/some/path"), PermissionAction::Ask);
    }

    #[test]
    fn test_allow_rule() {
        let checker = PermissionChecker::new(vec![PermissionRule {
            permission: "file:read".into(),
            pattern: "/home/**".into(),
            action: PermissionAction::Allow,
        }]);
        assert_eq!(
            checker.check("file:read", "/home/user/file.txt"),
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_always_grant() {
        let mut checker = PermissionChecker::new(vec![]);
        checker.record_always("file:write", "/tmp/**");
        assert_eq!(
            checker.check("file:write", "/tmp/output.txt"),
            PermissionAction::Allow
        );
    }
}
