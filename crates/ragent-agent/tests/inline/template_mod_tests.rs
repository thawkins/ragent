//! Relocated inline tests for `template/mod.rs` (ANTIPAT M2 test relocation).
//!
//! The body previously lived in an inline `#[cfg(test)] mod tests` in the
//! source file; it is now compiled from this file via a `#[path]` hook.

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
