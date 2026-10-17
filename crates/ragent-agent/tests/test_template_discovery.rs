//! Tests-only coverage for prompt-template discovery and loading
//! (`ragent_agent::template::discover_templates`).
//!
//! Bundled templates always load; project templates come from
//! `<working_dir>/.ragent/templates/*.md` and must override a bundled template
//! of the same name. All I/O is confined to a temp working directory.

use ragent_agent::template::{TemplateInfo, TemplateScope, discover_templates};

fn write_template(dir: &std::path::Path, name: &str, body: &str) {
    let tdir = dir.join(".ragent").join("templates");
    std::fs::create_dir_all(&tdir).expect("create templates dir");
    std::fs::write(tdir.join(format!("{name}.md")), body).expect("write template");
}

#[test]
fn bundled_templates_are_discovered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let templates = discover_templates(dir.path());

    for name in ["code-review", "bug-fix", "feature", "test-plan"] {
        let t = templates
            .get(name)
            .unwrap_or_else(|| panic!("bundled template {name} must be present"));
        assert_eq!(t.scope, TemplateScope::Bundled);
        assert!(t.placeholders.iter().any(|p| p == "arguments"));
    }
}

#[test]
fn project_template_is_loaded_and_overrides_bundled() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_template(dir.path(), "code-review", "PROJECT OVERRIDE {{arguments}}");
    write_template(dir.path(), "custom-note", "Custom body {{title}}");

    let templates = discover_templates(dir.path());

    let overridden = templates.get("code-review").expect("project override");
    assert_eq!(overridden.scope, TemplateScope::Project);
    assert!(overridden.body.starts_with("PROJECT OVERRIDE"));
    assert!(overridden.source_path.ends_with("code-review.md"));

    let custom = templates.get("custom-note").expect("custom template");
    assert!(custom.placeholders.iter().any(|p| p == "title"));
    assert_eq!(custom.name, "custom-note");
}

#[test]
fn non_markdown_files_are_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tdir = dir.path().join(".ragent").join("templates");
    std::fs::create_dir_all(&tdir).expect("dir");
    std::fs::write(tdir.join("notes.txt"), "not a template").expect("write txt");

    let templates = discover_templates(dir.path());
    assert!(
        !templates.contains_key("notes"),
        "txt files are not templates"
    );
}

#[test]
fn template_new_extracts_placeholders_and_applies_substitutions() {
    let mut t = TemplateInfo::new("t", "{{a}} and {{b}} and {{a}}");
    // Unique placeholders only.
    assert_eq!(t.placeholders.len(), 2);
    t.description = Some("desc".to_string());

    let mut subs = std::collections::HashMap::new();
    subs.insert("a".to_string(), "1".to_string());
    subs.insert("b".to_string(), "2".to_string());
    assert_eq!(t.apply(&subs), "1 and 2 and 1");

    // `{{arguments}}` with no entry collapses to empty.
    let args_template = TemplateInfo::new("args", "Args: {{arguments}}");
    assert_eq!(
        args_template.apply(&std::collections::HashMap::new()),
        "Args: "
    );

    // Display for scopes.
    assert_eq!(TemplateScope::Project, TemplateScope::Project);
}
