//! Tests for `SpecManager::status_command`, the shared core of the TUI
//! `/spec status` and the `ragent spec status` CLI (#40).

use ragent_specs::io::SpecIo;
use ragent_specs::manager::SpecManager;
use ragent_specs::spec::{SpecId, SpecStatus};
use ragent_specs::{SddFlags, StatusOutcome};

/// A per-test scratch directory under the workspace `target/temp`.
fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("target/temp creatable");
    tempfile::Builder::new()
        .prefix("spec-status-")
        .tempdir_in(root)
        .expect("scratch dir")
}

async fn draft_spec(root: &std::path::Path, name: &str) -> SpecManager {
    let id = SpecId::new(name).expect("valid id");
    SpecIo::create_spec_dir(root, &id, "---\nstatus: draft\n---\n\n# Spec\n", "")
        .await
        .expect("spec created");
    SpecManager::new(root)
}

#[tokio::test]
async fn test_spec_status_reports_current_and_allowed() {
    let tmp = scratch();
    let mgr = draft_spec(tmp.path(), "show").await;
    let outcome = mgr
        .status_command("show", None, "cli", &SddFlags::all_enabled())
        .await
        .expect("status shown");
    match outcome {
        StatusOutcome::Current {
            id,
            status,
            allowed,
        } => {
            assert_eq!(id, "show");
            assert_eq!(status, SpecStatus::Draft);
            assert!(allowed.contains(&SpecStatus::InReview), "{allowed:?}");
        }
        other => panic!("expected Current, got {other:?}"),
    }
}

#[tokio::test]
async fn test_spec_status_transition_is_persisted_with_actor() {
    let tmp = scratch();
    let mgr = draft_spec(tmp.path(), "move").await;
    let outcome = mgr
        .status_command("move", Some("in_review"), "cli", &SddFlags::all_enabled())
        .await
        .expect("transition allowed");
    assert_eq!(
        outcome,
        StatusOutcome::Transitioned {
            id: "move".to_string(),
            from: SpecStatus::Draft,
            to: SpecStatus::InReview,
        }
    );
    let spec = mgr
        .read_spec(&SpecId::new("move").expect("id"))
        .await
        .expect("re-read");
    assert_eq!(spec.status, SpecStatus::InReview);
    let last = spec.audit_trail.last().expect("audit entry");
    assert_eq!(
        (last.1.as_str(), last.2.as_str(), last.3.as_str()),
        ("draft", "in_review", "cli")
    );
}

#[tokio::test]
async fn test_spec_status_errors_are_user_facing() {
    let tmp = scratch();
    let mgr = draft_spec(tmp.path(), "err").await;
    let flags = SddFlags::all_enabled();

    let unknown = mgr
        .status_command("err", Some("bogus"), "cli", &flags)
        .await;
    assert!(
        unknown
            .as_ref()
            .is_err_and(|e| e.starts_with("unknown status 'bogus' (valid: draft,")),
        "{unknown:?}"
    );

    let refused = mgr
        .status_command("err", Some("implemented"), "cli", &flags)
        .await;
    assert!(
        refused
            .as_ref()
            .is_err_and(|e| e.starts_with("transition failed")),
        "{refused:?}"
    );

    let missing = mgr.status_command("nope", None, "cli", &flags).await;
    assert!(
        missing
            .as_ref()
            .is_err_and(|e| e.starts_with("failed to read nope")),
        "{missing:?}"
    );

    let invalid = mgr.status_command("../x", None, "cli", &flags).await;
    assert!(
        invalid
            .as_ref()
            .is_err_and(|e| e.starts_with("invalid spec id")),
        "{invalid:?}"
    );
}
