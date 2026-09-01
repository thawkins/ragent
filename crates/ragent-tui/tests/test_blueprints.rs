//! Tests for the team blueprint helper module.

use std::path::PathBuf;

use ragent_tui::app::blueprints::{
    BlueprintInfo, list_installed_blueprints, render_blueprint_detail, render_blueprint_list,
};

fn temp_blueprint_root() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(".ragent").join("blueprints").join("teams");
    std::fs::create_dir_all(&root).unwrap();
    (dir, root)
}

fn write_blueprint(
    root: &std::path::Path,
    name: &str,
    readme: &str,
    teammates: usize,
    tasks: usize,
) {
    let bp_dir = root.join(name);
    std::fs::create_dir_all(&bp_dir).unwrap();
    std::fs::write(bp_dir.join("README.md"), readme).unwrap();

    let teammates: Vec<serde_json::Value> = (0..teammates)
        .map(|i| {
            serde_json::json!({
                "teammate_name": format!("mate-{i}"),
                "agent_type": "general",
                "prompt": "help"
            })
        })
        .collect();
    std::fs::write(
        bp_dir.join("spawn-prompts.json"),
        serde_json::to_string(&teammates).unwrap(),
    )
    .unwrap();

    let tasks: Vec<serde_json::Value> = (0..tasks)
        .map(|i| {
            serde_json::json!({
                "title": format!("task-{i}"),
                "description": "do work"
            })
        })
        .collect();
    std::fs::write(
        bp_dir.join("task-seed.json"),
        serde_json::to_string(&tasks).unwrap(),
    )
    .unwrap();
}

#[test]
fn test_list_installed_blueprints_sorts_and_scopes() {
    let (_dir, root) = temp_blueprint_root();
    write_blueprint(&root, "zeta", "# Zeta\nlast", 0, 0);
    write_blueprint(&root, "alpha", "# Alpha\nfirst", 1, 2);

    let bps = list_installed_blueprints(root.parent().unwrap());
    assert_eq!(bps.len(), 2);
    assert_eq!(bps[0].name, "alpha");
    assert_eq!(bps[0].scope, "project");
    assert_eq!(bps[1].name, "zeta");
}

#[test]
fn test_render_blueprint_list_empty() {
    let output = render_blueprint_list(&[], "/team blueprint");
    assert!(output.contains("From: /team blueprint"));
    assert!(output.contains("No blueprints found"));
}

#[test]
fn test_render_blueprint_list_table_counts() {
    let (_dir, root) = temp_blueprint_root();
    write_blueprint(&root, "alpha", "# Alpha\nA short description", 2, 3);

    let bps = list_installed_blueprints(root.parent().unwrap());
    let output = render_blueprint_list(&bps, "/team blueprint");

    assert!(output.contains("| `alpha` | project | 2 | 3 | A short description |"));
    assert!(output.contains("## Installed Team Blueprints"));
}

#[test]
fn test_render_blueprint_list_skips_heading_for_description() {
    let (_dir, root) = temp_blueprint_root();
    write_blueprint(&root, "alpha", "# Alpha\n\n\nBody text", 0, 0);

    let bps = list_installed_blueprints(root.parent().unwrap());
    let output = render_blueprint_list(&bps, "/team blueprint");
    assert!(output.contains("Body text"));
    assert!(!output.contains("# Alpha"));
}

#[test]
fn test_blueprint_info_public_fields() {
    let info = BlueprintInfo {
        name: "demo".to_string(),
        path: PathBuf::from("/tmp/demo"),
        scope: "project".to_string(),
    };
    assert_eq!(info.name, "demo");
    assert!(info.path.ends_with("demo"));
    assert_eq!(info.scope, "project");
}

#[test]
fn test_render_blueprint_detail_found() {
    let (_dir, root) = temp_blueprint_root();
    write_blueprint(&root, "alpha", "# Alpha\nA short description", 1, 1);

    let bps = list_installed_blueprints(root.parent().unwrap());
    let output = render_blueprint_detail(&bps, "alpha", "/blueprints").expect("detail found");
    assert!(output.contains("From: /blueprints alpha"));
    assert!(output.contains("## Blueprint: `alpha`"));
    assert!(output.contains("A short description"));
    assert!(output.contains("### Teammates"));
    assert!(output.contains("### Seed Tasks"));
    assert!(output.contains("**Usage:** `/team create alpha`"));
}

#[test]
fn test_render_blueprint_detail_missing() {
    let (_dir, root) = temp_blueprint_root();
    write_blueprint(&root, "alpha", "# Alpha\nDesc", 0, 0);

    let bps = list_installed_blueprints(root.parent().unwrap());
    assert!(render_blueprint_detail(&bps, "missing", "/blueprints").is_none());
}
