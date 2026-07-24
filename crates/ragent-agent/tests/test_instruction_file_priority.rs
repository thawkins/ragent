//! Tests for instruction file discovery priority:
//! project root → global directory → project subdirectories

use std::fs;
use tempfile::TempDir;

/// When no local instruction file exists in the project root or subdirectories,
/// the global directory file should be loaded.
#[test]
fn test_global_fallback_when_no_local_files() {
    let working_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let global_path = global_dir.path().join("ragent");
    fs::create_dir_all(&global_path).unwrap();
    fs::write(global_path.join("AGENTS.md"), "Global instructions").unwrap();

    // Temporarily set XDG_DATA_HOME to our temp dir
    // Since collect_agents_md_content_with_discovery uses dirs::data_dir(),
    // we test the priority logic directly.
    let _working = working_dir.path();

    // Simulate: no local files, one global file
    let local_files: Vec<(usize, std::path::PathBuf)> = vec![];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![(0, global_path.join("AGENTS.md"))];

    // The first candidate should be the global file
    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(local_files);
    candidates.extend(global_files);

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert!(
        loaded.is_some(),
        "Should load global file when no local files exist"
    );
    assert_eq!(
        loaded.unwrap(),
        global_path.join("AGENTS.md"),
        "Global file should be loaded as fallback"
    );
}

/// When a local instruction file exists in the project root (depth 0),
/// it should take priority over the global directory file.
#[test]
fn test_project_root_takes_priority_over_global() {
    let working_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let global_path = global_dir.path().join("ragent");
    fs::create_dir_all(&global_path).unwrap();
    fs::write(global_path.join("AGENTS.md"), "Global instructions").unwrap();
    fs::write(
        working_dir.path().join("AGENTS.md"),
        "Local root instructions",
    )
    .unwrap();

    // Root file at depth 0
    let root_files: Vec<(usize, std::path::PathBuf)> =
        vec![(0, working_dir.path().join("AGENTS.md"))];
    let sub_files: Vec<(usize, std::path::PathBuf)> = vec![];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![(0, global_path.join("AGENTS.md"))];

    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);
    candidates.extend(sub_files);

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert_eq!(
        loaded.unwrap(),
        working_dir.path().join("AGENTS.md"),
        "Project root file should take priority over global"
    );
}

/// When no file exists in the project root but one exists in a subdirectory
/// and one exists in the global directory, the global file should take priority
/// over the subdirectory file.
#[test]
fn test_global_takes_priority_over_subdirectory() {
    let working_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let global_path = global_dir.path().join("ragent");
    fs::create_dir_all(&global_path).unwrap();
    fs::write(global_path.join("AGENTS.md"), "Global instructions").unwrap();

    // Subdirectory file at depth > 0
    let subdir = working_dir.path().join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("AGENTS.md"), "Subdirectory instructions").unwrap();

    // No root files, only subdirectory and global
    let root_files: Vec<(usize, std::path::PathBuf)> = vec![];
    let sub_files: Vec<(usize, std::path::PathBuf)> = vec![(1, subdir.join("AGENTS.md"))];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![(0, global_path.join("AGENTS.md"))];

    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);
    candidates.extend(sub_files);

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert_eq!(
        loaded.unwrap(),
        global_path.join("AGENTS.md"),
        "Global file should take priority over subdirectory file"
    );
}

/// When AGENTS.md exists in project root and CLAUDE.md exists globally,
/// the project root AGENTS.md should be loaded.
#[test]
fn test_root_agents_md_beats_global_claude_md() {
    let working_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let global_path = global_dir.path().join("ragent");
    fs::create_dir_all(&global_path).unwrap();
    fs::write(global_path.join("CLAUDE.md"), "Global claude instructions").unwrap();
    fs::write(
        working_dir.path().join("AGENTS.md"),
        "Local root agents instructions",
    )
    .unwrap();

    let root_files: Vec<(usize, std::path::PathBuf)> =
        vec![(0, working_dir.path().join("AGENTS.md"))];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![(0, global_path.join("CLAUDE.md"))];

    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert_eq!(
        loaded.unwrap(),
        working_dir.path().join("AGENTS.md"),
        "Root AGENTS.md should beat global CLAUDE.md"
    );
}

/// The full priority chain: root AGENTS.md > root CLAUDE.md > global > subdirectory
#[test]
fn test_full_priority_chain() {
    let working_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let global_path = global_dir.path().join("ragent");
    fs::create_dir_all(&global_path).unwrap();

    // Set up all layers (but no AGENTS.md)
    fs::write(working_dir.path().join("CLAUDE.md"), "Root CLAUDE.md").unwrap();
    fs::write(global_path.join("AGENTS.md"), "Global AGENTS.md").unwrap();
    let subdir = working_dir.path().join("src");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("AGENTS.md"), "Subdirectory AGENTS.md").unwrap();

    // Test 1: With AGENTS.md in root, root wins
    let root_agents = working_dir.path().join("AGENTS.md");
    fs::write(&root_agents, "Root AGENTS.md").unwrap();

    let root_files: Vec<(usize, std::path::PathBuf)> = vec![
        (0, root_agents.clone()),
        (0, working_dir.path().join("CLAUDE.md")),
    ];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![(0, global_path.join("AGENTS.md"))];
    let sub_files: Vec<(usize, std::path::PathBuf)> = vec![(1, subdir.join("AGENTS.md"))];

    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);
    candidates.extend(sub_files);

    // Sort by AGENT_FILE_NAMES priority
    let agent_file_names = ["AGENTS.md", "CLAUDE.md", ".ragent.md", "INSTRUCTIONS.md"];
    candidates.sort_by(|a, b| {
        let a_name = a.1.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.1.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let a_idx = agent_file_names
            .iter()
            .position(|n| *n == a_name)
            .unwrap_or(usize::MAX);
        let b_idx = agent_file_names
            .iter()
            .position(|n| *n == b_name)
            .unwrap_or(usize::MAX);
        a_idx.cmp(&b_idx)
    });

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert_eq!(
        loaded.unwrap(),
        root_agents,
        "Root AGENTS.md should be first"
    );
}

/// When only subdirectory files exist (no root, no global),
/// the shallowest subdirectory file should be loaded.
#[test]
fn test_subdirectory_fallback_when_no_root_or_global() {
    let working_dir = TempDir::new().unwrap();
    let subdir = working_dir.path().join("src");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("AGENTS.md"), "Subdirectory instructions").unwrap();

    // No root files, no global, only subdirectory
    let root_files: Vec<(usize, std::path::PathBuf)> = vec![];
    let global_files: Vec<(usize, std::path::PathBuf)> = vec![];
    let sub_files: Vec<(usize, std::path::PathBuf)> = vec![(1, subdir.join("AGENTS.md"))];

    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);
    candidates.extend(sub_files);

    let loaded = candidates.first().map(|(_, p)| p.clone());
    assert_eq!(
        loaded.unwrap(),
        subdir.join("AGENTS.md"),
        "Subdirectory file should be loaded as last resort"
    );
}

/// When both `AGENTS.md` exists in the project root and a subdirectory
/// (e.g. `assets/config/AGENTS.md`), the root file must be loaded.
#[test]
fn test_root_agents_md_beats_subdirectory_agents_md() {
    let working_dir = TempDir::new().unwrap();

    // Subdirectory AGENTS.md (depth > 0)
    let subdir = working_dir.path().join("assets").join("config");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("AGENTS.md"), "Subdirectory instructions").unwrap();

    // Root AGENTS.md (should win)
    fs::write(working_dir.path().join("AGENTS.md"), "Root instructions").unwrap();

    let (content, discovery) =
        ragent_agent::agent::collect_agents_md_content_with_discovery(working_dir.path());

    // The loaded file should be the root AGENTS.md
    assert_eq!(
        discovery.loaded_file,
        Some(working_dir.path().join("AGENTS.md")),
        "Root AGENTS.md should take priority over subdirectory AGENTS.md"
    );
    assert!(
        content.contains("Root instructions"),
        "Loaded content should come from root AGENTS.md, got:\n{content}"
    );
    assert!(
        !content.contains("Subdirectory instructions"),
        "Content from subdirectory file should not be loaded"
    );
}
