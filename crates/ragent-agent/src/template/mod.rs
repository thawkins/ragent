//! Prompt template discovery, loading, and substitution.
//!
//! Templates are reusable prompt fragments stored as `.md` files in template directories.
//! Each template can contain placeholders that are substituted at runtime.
//!
//! # Template Structure
//!
//! ```text
//! .ragent/templates/
//!   code-review.md        # Template for code review prompts
//!   bug-fix.md            # Template for bug fix prompts
//!   feature-request.md    # Template for feature requests
//!   test-plan.md          # Template for test plan generation
//! ```
//!
//! # Placeholder Syntax
//!
//! Templates support the following placeholders:
//!
//! - `{{title}}` — Title or name of the task
//! - `{{description}}` — Detailed description
//! - `{{context}}` — Additional context or background
//! - `{{requirements}}` — List of requirements
//! - `{{constraints}}` — Constraints or limitations
//! - `{{examples}}` — Example inputs/outputs
//! - `{{arguments}}` — Raw arguments passed to the template
//!
//! # Template Scopes
//!
//! | Scope              | Path                                    | Applies To                    |
//! |--------------------|-----------------------------------------|-------------------------------|
//! | Bundled            | `crates/ragent-agent/templates/`        | Templates bundled with ragent |
//! | Personal           | `~/.ragent/templates/<name>.md`         | All projects for this user    |
//! | Project            | `.ragent/templates/<name>.md`           | This project only             |
//!
//! Higher-priority scopes override lower ones when names conflict.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A reusable prompt template loaded from a `.md` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Unique identifier for the template (defaults to filename without extension).
    pub name: String,
    /// What the template is for; used for autocomplete matching.
    pub description: Option<String>,
    /// Template body with optional placeholders.
    #[serde(skip)]
    pub body: String,
    /// Absolute path to the `.md` file this template was loaded from.
    #[serde(skip)]
    pub source_path: PathBuf,
    /// Directory containing the template file.
    #[serde(skip)]
    pub template_dir: PathBuf,
    /// Where this template was discovered.
    pub scope: TemplateScope,
    /// List of placeholder names found in the template body (e.g., `["title", "description"]`).
    #[serde(skip)]
    pub placeholders: Vec<String>,
}

/// Resolution scope indicating where a template was loaded from.
///
/// Higher-numbered scopes take precedence over lower ones when names conflict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum TemplateScope {
    /// Bundled with ragent (lowest priority).
    Bundled = 0,
    /// User-level template from `~/.ragent/templates/`.
    Personal = 1,
    /// Project-level template from `.ragent/templates/`.
    Project = 2,
}

impl std::fmt::Display for TemplateScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bundled => write!(f, "bundled"),
            Self::Personal => write!(f, "personal"),
            Self::Project => write!(f, "project"),
        }
    }
}

impl TemplateInfo {
    /// Creates a new template with the given name and body.
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        let body = body.into();
        let placeholders = extract_placeholders(&body);
        Self {
            name: name.into(),
            description: None,
            body,
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders,
        }
    }

    /// Apply substitutions to the template body.
    ///
    /// Replaces `{{placeholder}}` patterns with corresponding values from `substitutions`.
    /// Any placeholders without a matching substitution are left unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::template::TemplateInfo;
    ///
    /// let template = TemplateInfo::new("greeting", "Hello, {{name}}!");
    /// let mut subs = std::collections::HashMap::new();
    /// subs.insert("name".to_string(), "World".to_string());
    /// let result = template.apply(&subs);
    /// assert_eq!(result, "Hello, World!");
    /// ```
    pub fn apply(&self, substitutions: &HashMap<String, String>) -> String {
        let mut result = self.body.clone();
        for (key, value) in substitutions {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        // Handle {{arguments}} specially - replace with empty string if not provided
        result = result.replace(
            "{{arguments}}",
            substitutions
                .get("arguments")
                .map(|s| s.as_str())
                .unwrap_or(""),
        );
        result
    }

    /// Apply substitutions with a simple key=value string for arguments.
    ///
    /// This is a convenience method for the common case where the user provides
    /// a single string of arguments.
    pub fn apply_simple(&self, args: &str) -> String {
        let mut substitutions = HashMap::new();
        substitutions.insert("arguments".to_string(), args.to_string());
        self.apply(&substitutions)
    }
}

/// Extract placeholder names from a template body.
///
/// Finds all `{{name}}` patterns and returns the unique placeholder names.
fn extract_placeholders(body: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in regex::Regex::new(r"\{\{(\w+)\}\}")
        .unwrap()
        .captures_iter(body)
    {
        if let Some(name) = cap.get(1).and_then(|m| m.as_str().into()) {
            if seen.insert(name.to_string()) {
                placeholders.push(name.to_string());
            }
        }
    }

    placeholders
}

/// Discovers and loads templates from all known directories.
///
/// Returns a map of template name → TemplateInfo, with higher-priority scopes
/// overriding lower-priority ones when names conflict.
pub fn discover_templates(working_dir: &Path) -> HashMap<String, TemplateInfo> {
    let mut templates = HashMap::new();

    // Load bundled templates first (lowest priority)
    if let Ok(bundled) = load_bundled_templates() {
        for template in bundled {
            templates.insert(template.name.clone(), template);
        }
    }

    // Load personal templates from ~/.ragent/templates/
    if let Some(home_dir) = dirs::home_dir() {
        let personal_dir = home_dir.join(".ragent/templates");
        if personal_dir.exists() {
            for template in load_templates_from_dir(&personal_dir, TemplateScope::Personal) {
                templates.insert(template.name.clone(), template);
            }
        }
    }

    // Load project templates from .ragent/templates/
    let project_dir = working_dir.join(".ragent/templates");
    if project_dir.exists() {
        for template in load_templates_from_dir(&project_dir, TemplateScope::Project) {
            templates.insert(template.name.clone(), template);
        }
    }

    templates
}

/// Load bundled templates from the crate's templates directory.
fn load_bundled_templates() -> Result<Vec<TemplateInfo>, std::io::Error> {
    // Bundled templates are in the crate's templates/ directory
    // For now, we'll use a few built-in templates defined inline
    let templates = vec![
        // Code review template
        TemplateInfo {
            name: "code-review".to_string(),
            description: Some("Review code for quality, security, and best practices".to_string()),
            body: r"Review the following code for quality, security, and best practices:

**Code to Review:**
```
{{arguments}}
```

**Review Criteria:**
1. Code correctness and potential bugs
2. Security vulnerabilities
3. Performance issues
4. Code style and readability
5. Test coverage
6. Documentation

Provide specific, actionable feedback for each issue found."
                .to_string(),
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders: vec!["arguments".to_string()],
        },
        // Bug fix template
        TemplateInfo {
            name: "bug-fix".to_string(),
            description: Some("Analyze and fix a reported bug".to_string()),
            body: r"Analyze and fix the following bug:

**Bug Description:**
{{arguments}}

**Steps to Reproduce:**
[Describe steps to reproduce the bug]

**Expected Behavior:**
[What should happen]

**Actual Behavior:**
[What actually happens]

**Proposed Fix:**
[Describe the fix]

**Testing:**
[How to verify the fix works]"
                .to_string(),
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders: vec!["arguments".to_string()],
        },
        // Feature request template
        TemplateInfo {
            name: "feature".to_string(),
            description: Some("Implement a new feature".to_string()),
            body: r"Implement the following feature:

**Feature Description:**
{{arguments}}

**User Story:**
As a [user type], I want [goal] so that [benefit].

**Acceptance Criteria:**
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

**Implementation Notes:**
[Technical details, constraints, or considerations]

**Testing Strategy:**
[How to test the feature]"
                .to_string(),
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders: vec!["arguments".to_string()],
        },
        // Test plan template
        TemplateInfo {
            name: "test-plan".to_string(),
            description: Some("Generate a comprehensive test plan".to_string()),
            body: r"Generate a comprehensive test plan for:

**Feature/Component:**
{{arguments}}

**Test Scope:**
- Unit tests
- Integration tests
- End-to-end tests
- Performance tests
- Security tests

**Test Cases:**
| ID | Description | Preconditions | Steps | Expected Result |
|----|-------------|---------------|-------|-----------------|
| TC-001 | | | | |
| TC-002 | | | | |

**Test Data Requirements:**
[List any special test data needed]

**Environment Requirements:**
[List any special environment needs]

**Risk Assessment:**
[Identify potential testing risks and mitigations]"
                .to_string(),
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders: vec!["arguments".to_string()],
        },
        // Documentation template
        TemplateInfo {
            name: "docs".to_string(),
            description: Some("Write documentation for a feature or API".to_string()),
            body: r"Write comprehensive documentation for:

**Topic:**
{{arguments}}

**Documentation Type:**
- [ ] API Reference
- [ ] User Guide
- [ ] Tutorial
- [ ] Conceptual Guide
- [ ] Troubleshooting Guide

**Target Audience:**
[Describe the intended readers]

**Prerequisites:**
[What readers should know before reading this]

**Content Outline:**
1. Introduction
2. Key Concepts
3. Usage Examples
4. API Reference (if applicable)
5. Troubleshooting
6. Related Resources

**Style Guidelines:**
- Use clear, concise language
- Include code examples where relevant
- Link to related documentation
- Keep sections focused and scannable"
                .to_string(),
            source_path: PathBuf::new(),
            template_dir: PathBuf::new(),
            scope: TemplateScope::Bundled,
            placeholders: vec!["arguments".to_string()],
        },
    ];

    Ok(templates)
}

/// Load templates from a directory.
fn load_templates_from_dir(dir: &Path, scope: TemplateScope) -> Vec<TemplateInfo> {
    let mut templates = Vec::new();

    if !dir.exists() {
        return templates;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return templates,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(template) = load_template_from_file(&path, scope) {
                templates.push(template);
            }
        }
    }

    templates
}

/// Load a single template from a file.
fn load_template_from_file(
    path: &Path,
    scope: TemplateScope,
) -> Result<TemplateInfo, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string();

    let body = content;
    let placeholders = extract_placeholders(&body);
    let template_dir = path.parent().unwrap_or(path).to_path_buf();

    Ok(TemplateInfo {
        name,
        description: None,
        body,
        source_path: path.to_path_buf(),
        template_dir,
        scope,
        placeholders,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_apply() {
        let template = TemplateInfo::new("test", "Hello, {{name}}! Welcome to {{place}}.");
        let mut subs = HashMap::new();
        subs.insert("name".to_string(), "Alice".to_string());
        subs.insert("place".to_string(), "Wonderland".to_string());

        let result = template.apply(&subs);
        assert_eq!(result, "Hello, Alice! Welcome to Wonderland.");
    }

    #[test]
    fn test_template_apply_missing_placeholder() {
        let template = TemplateInfo::new("test", "Hello, {{name}}! {{missing}}");
        let mut subs = HashMap::new();
        subs.insert("name".to_string(), "Bob".to_string());

        let result = template.apply(&subs);
        assert_eq!(result, "Hello, Bob! {{missing}}");
    }

    #[test]
    fn test_extract_placeholders() {
        let body = "Hello {{name}}, welcome to {{place}}. Say {{name}} again.";
        let placeholders = extract_placeholders(body);
        assert_eq!(placeholders.len(), 2);
        assert!(placeholders.contains(&"name".to_string()));
        assert!(placeholders.contains(&"place".to_string()));
    }

    #[test]
    fn test_apply_simple() {
        let template = TemplateInfo::new("test", "Args: {{arguments}}");
        let result = template.apply_simple("test value");
        assert_eq!(result, "Args: test value");
    }
}
