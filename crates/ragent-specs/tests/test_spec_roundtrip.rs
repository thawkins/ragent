//! Round-trip tests for spec persistence: PLAN.md task rows and SPEC.md
//! frontmatter must survive a write -> read cycle without losing or
//! corrupting data.

use ragent_specs::io::SpecIo;
use ragent_specs::manager::SpecManager;
use ragent_specs::plan_parser::PlanParser;
use ragent_specs::spec::{SpecId, SpecStatus, TaskStatus};

const PLAN: &str = "# Plan\n\n## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n\
|----|-------|-------------|--------|----------|--------|--------------|\n\
| T-001 | First | FR-001 | S | High | pending | - |\n\
| T-002 | Second |  | M | Low | pending | T-001 |\n\
\nTrailing prose after the table.\n\n## Notes\n\nKeep me.\n";

#[test]
fn test_spec_roundtrip_empty_requirement_cell_keeps_columns() {
    let tasks = PlanParser::parse(PLAN).unwrap();
    let t2 = tasks.iter().find(|t| t.id == "T-002").unwrap();
    assert_eq!(t2.requirement, "");
    assert_eq!(t2.effort.as_str(), "M");
    assert_eq!(t2.priority.as_str(), "Low");
    assert_eq!(t2.status, TaskStatus::Pending);
    assert_eq!(t2.dependencies, vec!["T-001".to_string()]);
}

#[test]
fn test_spec_roundtrip_dash_placeholders_parse_as_empty() {
    let plan = "## Tasks\n\n\
| ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n\
|----|-------|-------------|--------|----------|--------|--------------|\n\
| T-001 | A | \u{2014} | S | High | pending | \u{2014} |\n\
| T-002 | B | \u{2013} | S | High | pending | \u{2013} |\n";
    let tasks = PlanParser::parse(plan).unwrap();
    for t in &tasks {
        assert!(t.dependencies.is_empty(), "{}: {:?}", t.id, t.dependencies);
    }
}

#[tokio::test]
async fn test_spec_roundtrip_task_update_preserves_placeholders_and_prose() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("rt").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &id, "# RT\n", PLAN)
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());
    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.update_task_status(&mut spec, "T-001", TaskStatus::Completed)
        .await
        .unwrap();

    let reread = mgr.read_spec(&id).await.unwrap();
    let t1 = reread.tasks.iter().find(|t| t.id == "T-001").unwrap();
    let t2 = reread.tasks.iter().find(|t| t.id == "T-002").unwrap();
    assert_eq!(t1.status, TaskStatus::Completed);
    assert!(t1.dependencies.is_empty(), "{:?}", t1.dependencies);
    assert!(
        t2.linked_requirements.is_empty(),
        "{:?}",
        t2.linked_requirements
    );
    assert!(reread.plan_md.contains("Trailing prose after the table."));
    assert!(reread.plan_md.contains("Keep me."));
    assert!(reread.plan_md.ends_with('\n'));
}

#[tokio::test]
async fn test_spec_roundtrip_audit_trail_survives_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("audit").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &id, "---\nstatus: draft\n---\n\n# A\n", "")
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());

    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.transition(&mut spec, SpecStatus::InReview, "alice")
        .await
        .unwrap();
    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.transition(&mut spec, SpecStatus::Draft, "bob")
        .await
        .unwrap();

    let spec = mgr.read_spec(&id).await.unwrap();
    let actors: Vec<&str> = spec.audit_trail.iter().map(|e| e.3.as_str()).collect();
    assert!(actors.contains(&"alice"), "{:?}", spec.audit_trail);
    assert!(actors.contains(&"bob"), "{:?}", spec.audit_trail);
    let last = spec.audit_trail.last().unwrap();
    assert_eq!(last.1, "in_review");
    assert_eq!(last.2, "draft");
}

#[tokio::test]
async fn test_spec_roundtrip_transition_preserves_other_frontmatter_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("fm").unwrap();
    let spec_md = "---\nstatus: draft\nresearch: [auth-study]\nowner: carol\n---\n\n# FM\n";
    SpecIo::create_spec_dir(tmp.path(), &id, spec_md, "")
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());
    let mut spec = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec.research, vec!["auth-study".to_string()]);
    mgr.transition(&mut spec, SpecStatus::InReview, "alice")
        .await
        .unwrap();

    let spec = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec.status, SpecStatus::InReview);
    assert_eq!(spec.research, vec!["auth-study".to_string()]);
    assert!(spec.spec_md.contains("owner: carol"));
    assert_eq!(spec.spec_md.matches("status:").count(), 1);
}

#[tokio::test]
async fn test_spec_roundtrip_research_multiline_list() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("rl").unwrap();
    let spec_md = "---\nstatus: draft\nresearch:\n  - one\n  - two\n---\n\n# RL\n";
    SpecIo::create_spec_dir(tmp.path(), &id, spec_md, "")
        .await
        .unwrap();
    let spec = SpecIo::read_spec(tmp.path(), &id).await.unwrap();
    assert_eq!(spec.research, vec!["one".to_string(), "two".to_string()]);
}

#[tokio::test]
async fn test_spec_roundtrip_discover_skips_unreadable_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let good = SpecId::new("good").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &good, "# Good\n", "")
        .await
        .unwrap();
    let bad = tmp.path().join("bad");
    std::fs::create_dir(&bad).unwrap();
    std::fs::write(bad.join("SPEC.md"), [0xff, 0xfe, 0xfd]).unwrap();

    let specs = SpecIo::discover_specs(tmp.path()).await.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].id.as_str(), "good");
}

#[tokio::test]
async fn test_spec_roundtrip_completed_at_persists_and_clears() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("ts").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &id, "# TS\n", PLAN)
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());

    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.update_task_status(&mut spec, "T-001", TaskStatus::Completed)
        .await
        .unwrap();
    let stamped = mgr.read_spec(&id).await.unwrap();
    let t1 = stamped.tasks.iter().find(|t| t.id == "T-001").unwrap();
    let when = t1.completed_at.expect("completion time must be persisted");
    assert!(
        stamped.plan_md.contains("| completed ("),
        "{}",
        stamped.plan_md
    );

    // Updating a different task keeps T-001's original timestamp.
    let mut spec = stamped;
    mgr.update_task_status(&mut spec, "T-002", TaskStatus::InProgress)
        .await
        .unwrap();
    let reread = mgr.read_spec(&id).await.unwrap();
    let t1 = reread.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(t1.completed_at, Some(when));

    // Reopening the task drops the timestamp.
    let mut spec = reread;
    mgr.update_task_status(&mut spec, "T-001", TaskStatus::Pending)
        .await
        .unwrap();
    let reread = mgr.read_spec(&id).await.unwrap();
    let t1 = reread.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(t1.status, TaskStatus::Pending);
    assert_eq!(t1.completed_at, None);
}

#[test]
fn test_spec_roundtrip_status_cell_parsing() {
    assert_eq!(
        TaskStatus::parse_status_cell("completed (2026-11-08T10:12:00Z)"),
        Some((TaskStatus::Completed, Some(1_794_132_720)))
    );
    // Legacy bare status, bad timestamp, and a stamp on a non-completed task.
    assert_eq!(
        TaskStatus::parse_status_cell("completed"),
        Some((TaskStatus::Completed, None))
    );
    assert_eq!(
        TaskStatus::parse_status_cell("completed (yesterday)"),
        Some((TaskStatus::Completed, None))
    );
    assert_eq!(
        TaskStatus::parse_status_cell("pending (2026-11-08T10:12:00Z)"),
        Some((TaskStatus::Pending, None))
    );
    assert_eq!(TaskStatus::parse_status_cell("done"), None);
    assert_eq!(
        TaskStatus::Completed.status_cell(Some(1_794_132_720)),
        "completed (2026-11-08T10:12:00Z)"
    );
    assert_eq!(TaskStatus::Pending.status_cell(Some(1)), "pending");
}

#[tokio::test]
async fn test_spec_roundtrip_yaml_comments_do_not_end_lists() {
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("cm").unwrap();
    let spec_md = "---\nstatus: draft\nreviewers:\n  - alice\n# second reviewer below\n  - bob\n\
research:\n  - one\n# note\n  - two\n---\n\n# CM\n";
    SpecIo::create_spec_dir(tmp.path(), &id, spec_md, "")
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());
    let mut spec = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec.reviewers, vec!["alice".to_string(), "bob".to_string()]);
    assert_eq!(spec.research, vec!["one".to_string(), "two".to_string()]);

    mgr.transition(&mut spec, SpecStatus::InReview, "carol")
        .await
        .unwrap();
    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.transition(&mut spec, SpecStatus::Draft, "dave")
        .await
        .unwrap();

    let spec = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec.reviewers, vec!["alice".to_string(), "bob".to_string()]);
    assert_eq!(spec.research, vec!["one".to_string(), "two".to_string()]);
    let actors: Vec<&str> = spec.audit_trail.iter().map(|e| e.3.as_str()).collect();
    assert!(
        actors.contains(&"carol") && actors.contains(&"dave"),
        "{actors:?}"
    );
    // No orphaned reviewer list items copied into the rewritten frontmatter.
    let (fm, _) = spec.spec_md[4..].split_once("\n---").unwrap();
    assert!(!fm.contains("  - bob"), "{fm}");
}

#[tokio::test]
async fn test_spec_roundtrip_legacy_task_id_header_is_normalised() {
    let plan = "## Tasks\n\n\
| Task ID | Title | Requirement | Effort | Priority | Dependencies |\n\
|---|---|---|---|---|---|\n\
| T-001 | A | FR-001 | S | High | - |\n\
| T-002 | B | FR-001 | S | High | T-001 |\n";
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("legacy").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &id, "# L\n", plan)
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());
    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.update_task_status(&mut spec, "T-001", TaskStatus::Completed)
        .await
        .unwrap();

    let reread = mgr.read_spec(&id).await.unwrap();
    let t1 = reread.tasks.iter().find(|t| t.id == "T-001").unwrap();
    let t2 = reread.tasks.iter().find(|t| t.id == "T-002").unwrap();
    assert_eq!(t1.status, TaskStatus::Completed);
    assert!(t1.completed_at.is_some());
    assert_eq!(t2.dependencies, vec!["T-001".to_string()]);
}

#[tokio::test]
async fn test_spec_roundtrip_unrelated_table_in_tasks_section_untouched() {
    let review = "### Review assignments\n\n\
| ID | Title | Reviewer |\n\
|---|---|---|\n\
| T-001 | Security review | Alice |\n";
    let plan = format!("{PLAN}\n## Tasks appendix\n");
    let plan = plan.replace("\nTrailing prose", &format!("\n{review}\nTrailing prose"));
    let tmp = tempfile::tempdir().unwrap();
    let id = SpecId::new("rev").unwrap();
    SpecIo::create_spec_dir(tmp.path(), &id, "# R\n", &plan)
        .await
        .unwrap();
    let mgr = SpecManager::new(tmp.path());
    let mut spec = mgr.read_spec(&id).await.unwrap();
    mgr.update_task_status(&mut spec, "T-001", TaskStatus::Completed)
        .await
        .unwrap();

    let reread = mgr.read_spec(&id).await.unwrap();
    assert!(reread.plan_md.contains(review), "{}", reread.plan_md);
    let t1 = reread.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(t1.status, TaskStatus::Completed);
    assert_eq!(t1.title, "First");
}
