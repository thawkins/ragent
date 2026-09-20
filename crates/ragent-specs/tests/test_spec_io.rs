//! External tests for `tests` from `crates/ragent-specs/src/io.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_specs::error::SpecError;
use ragent_specs::io::SpecIo;
use ragent_specs::spec::{Spec, SpecId, SpecStatus};
use tokio::fs;

#[tokio::test]
async fn test_create_spec_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("my-spec").unwrap();
    let spec_md = "# Test Spec\n";
    let plan_md = "# Plan\n";
    let dir = SpecIo::create_spec_dir(tmp.path(), &id, spec_md, plan_md)
        .await
        .unwrap();
    assert!(dir.is_dir());
    assert!(dir.join("SPEC.md").is_file());
    assert!(dir.join("PLAN.md").is_file());
    assert_eq!(
        fs::read_to_string(dir.join("SPEC.md")).await.unwrap(),
        spec_md
    );
}

#[tokio::test]
async fn test_create_spec_dir_already_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("my-spec").unwrap();
    fs::create_dir(tmp.path().join("my-spec")).await.unwrap();
    let result = SpecIo::create_spec_dir(tmp.path(), &id, "x", "y").await;
    assert!(matches!(result, Err(SpecError::AlreadyExists(_))));
}

#[tokio::test]
async fn test_atomic_write() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("test.txt");
    SpecIo::atomic_write(&path, "hello world").await.unwrap();
    assert_eq!(fs::read_to_string(&path).await.unwrap(), "hello world");
    // No temp file should remain
    assert!(!tmp.path().join("test.txt.tmp").exists());
}

#[tokio::test]
async fn test_read_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("data.md");
    fs::write(&path, "content").await.unwrap();
    let data = SpecIo::read_file(&path).await.unwrap();
    assert_eq!(data, "content");
}

#[tokio::test]
async fn test_spec_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("exists").unwrap();
    assert!(!SpecIo::spec_exists(tmp.path(), &id).await);
    fs::create_dir(tmp.path().join("exists")).await.unwrap();
    assert!(SpecIo::spec_exists(tmp.path(), &id).await);
}

#[tokio::test]
async fn test_discover_specs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // Create two valid specs and one invalid directory
    let id1 = SpecId::new("spec-one").unwrap();
    let id2 = SpecId::new("spec-two").unwrap();
    SpecIo::create_spec_dir(root, &id1, "# Spec One\n", "# Plan One\n")
        .await
        .unwrap();
    SpecIo::create_spec_dir(root, &id2, "# Spec Two\n", "# Plan Two\n")
        .await
        .unwrap();
    fs::create_dir(root.join("no-files")).await.unwrap();

    let specs = SpecIo::discover_specs(root).await.unwrap();
    assert_eq!(specs.len(), 2);
    let titles: Vec<_> = specs.iter().map(|s| s.title.clone()).collect();
    assert!(titles.contains(&"Spec One".to_string()));
    assert!(titles.contains(&"Spec Two".to_string()));
}

#[tokio::test]
async fn test_read_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("read-test").unwrap();
    SpecIo::create_spec_dir(root, &id, "# Read Test\n", "# Plan\n")
        .await
        .unwrap();
    let spec = SpecIo::read_spec(root, &id).await.unwrap();
    assert_eq!(spec.id.as_str(), "read-test");
    assert_eq!(spec.title, "Read Test");
    assert!(spec.path.is_some());
}

#[tokio::test]
async fn test_read_spec_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("missing").unwrap();
    let result = SpecIo::read_spec(tmp.path(), &id).await;
    assert!(matches!(result, Err(SpecError::NotFound(_))));
}

#[tokio::test]
async fn test_write_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("write-test").unwrap();
    let mut spec = Spec::new(id.clone(), "Write Test");
    spec.spec_md = "# Updated Spec\n".to_string();
    spec.plan_md = "# Updated Plan\n".to_string();
    SpecIo::write_spec(root, &spec).await.unwrap();

    let spec_path = root.join("write-test/SPEC.md");
    let plan_path = root.join("write-test/PLAN.md");
    assert!(spec_path.is_file());
    assert_eq!(
        fs::read_to_string(spec_path).await.unwrap(),
        "# Updated Spec\n"
    );
    assert_eq!(
        fs::read_to_string(plan_path).await.unwrap(),
        "# Updated Plan\n"
    );
}

#[tokio::test]
async fn test_extract_title() {
    assert_eq!(SpecIo::extract_title("# My Title\n"), "My Title");
    assert_eq!(SpecIo::extract_title("\n# Title Here\n"), "Title Here");
    assert_eq!(SpecIo::extract_title("no header"), "");
}

#[tokio::test]
async fn test_extract_status() {
    let fm = "---\nstatus: approved\n---\n# Title\n";
    assert_eq!(SpecIo::extract_status(fm), Some(SpecStatus::Approved));

    let no_fm = "# Title\n";
    assert_eq!(SpecIo::extract_status(no_fm), None);

    let draft_fm = "---\nstatus: draft\n---\n";
    assert_eq!(SpecIo::extract_status(draft_fm), Some(SpecStatus::Draft));
}
// ── FEEDBACK.md I/O tests (T-031, FR-017) ──────────────────────────────────

#[tokio::test]
async fn test_read_spec_loads_feedback_md() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("feedback-test").unwrap();
    SpecIo::create_spec_dir(root, &id, "# Test\n", "# Plan\n")
        .await
        .unwrap();
    SpecIo::atomic_write(
        root.join("feedback-test/FEEDBACK.md"),
        "# Feedback: Test\n\nSome notes.\n",
    )
    .await
    .unwrap();
    let spec = SpecIo::read_spec(root, &id).await.unwrap();
    assert_eq!(spec.feedback_md, "# Feedback: Test\n\nSome notes.\n");
}

#[tokio::test]
async fn test_read_spec_without_feedback_md_defaults_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("no-feedback").unwrap();
    SpecIo::create_spec_dir(root, &id, "# Test\n", "# Plan\n")
        .await
        .unwrap();
    let spec = SpecIo::read_spec(root, &id).await.unwrap();
    assert!(spec.feedback_md.is_empty());
}

#[tokio::test]
async fn test_write_spec_persists_feedback_md() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("write-feedback").unwrap();
    let mut spec = Spec::new(id.clone(), "Write Test");
    spec.spec_md = "# Spec\n".to_string();
    spec.plan_md = "# Plan\n".to_string();
    spec.feedback_md = "# Feedback: Test\n\nProduction note.\n".to_string();
    SpecIo::write_spec(root, &spec).await.unwrap();

    let feedback_path = root.join("write-feedback/FEEDBACK.md");
    assert!(feedback_path.is_file());
    assert_eq!(
        fs::read_to_string(feedback_path).await.unwrap(),
        "# Feedback: Test\n\nProduction note.\n"
    );
}

#[tokio::test]
async fn test_write_spec_omits_feedback_md_when_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("no-write-feedback").unwrap();
    let mut spec = Spec::new(id.clone(), "Write Test");
    spec.spec_md = "# Spec\n".to_string();
    spec.plan_md = "# Plan\n".to_string();
    spec.feedback_md = String::new();
    SpecIo::write_spec(root, &spec).await.unwrap();

    let feedback_path = root.join("no-write-feedback/FEEDBACK.md");
    assert!(!feedback_path.exists());
}

#[tokio::test]
async fn test_discover_specs_loads_feedback_md() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let id = SpecId::new("disc-feedback").unwrap();
    SpecIo::create_spec_dir(root, &id, "# Test\n", "# Plan\n")
        .await
        .unwrap();
    SpecIo::atomic_write(
        root.join("disc-feedback/FEEDBACK.md"),
        "# Feedback: Test\n\nDiscovered.\n",
    )
    .await
    .unwrap();
    let specs = SpecIo::discover_specs(root).await.unwrap();
    let spec = specs
        .iter()
        .find(|s| s.id.as_str() == "disc-feedback")
        .expect("spec should be discovered");
    assert_eq!(spec.feedback_md, "# Feedback: Test\n\nDiscovered.\n");
}

#[test]
fn test_spec_feedback_md_path() {
    let id = SpecId::new("path-test").unwrap();
    let spec = Spec::new(id, "Test");
    let root = std::path::Path::new("specs");
    assert_eq!(
        spec.feedback_md_path(root),
        std::path::Path::new("specs/path-test/FEEDBACK.md")
    );
}

#[test]
fn test_spec_new_feedback_md_empty() {
    let id = SpecId::new("new-test").unwrap();
    let spec = Spec::new(id, "Test");
    assert!(spec.feedback_md.is_empty());
}
