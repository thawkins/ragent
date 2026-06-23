//! Tests for spec agent tools.

use ragent_agent::tool::{
    Tool, ToolContext, spec_coverage::SpecCoverageTool, spec_list::SpecListTool,
    spec_read::SpecReadTool, spec_search::SpecSearchTool, spec_task_update::SpecTaskUpdateTool,
};
use ragent_specs::{SpecIo, SpecManager};
use serde_json::json;
use std::sync::Arc;

fn base_ctx() -> ToolContext {
    use ragent_agent::event::EventBus;
    use std::path::PathBuf;
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
    }
}

#[tokio::test]
async fn test_spec_read_not_configured() {
    let tool = SpecReadTool;
    let result = tool.execute(json!({"spec_id": "test"}), &base_ctx()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not configured"));
}

#[tokio::test]
async fn test_spec_list_not_configured() {
    let tool = SpecListTool;
    let result = tool.execute(json!({}), &base_ctx()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_spec_search_not_configured() {
    let tool = SpecSearchTool;
    let result = tool.execute(json!({"query": "auth"}), &base_ctx()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_spec_task_update_not_configured() {
    let tool = SpecTaskUpdateTool;
    let result = tool
        .execute(
            json!({"spec_id": "test", "task_id": "T-001", "status": "completed"}),
            &base_ctx(),
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_spec_coverage_not_configured() {
    let tool = SpecCoverageTool;
    let result = tool.execute(json!({"spec_id": "test"}), &base_ctx()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_spec_read_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n\n## Requirements\n\n- FR-001: The system shall do X.\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Implement X | FR-001 | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecReadTool;
    let result = tool.execute(json!({"spec_id": "testspec"}), &ctx).await;
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.content.contains("Test Spec"));
    assert!(output.content.contains("FR-001"));
    assert!(output.content.contains("T-001"));
}

#[tokio::test]
async fn test_spec_list_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    SpecIo::create_spec_dir(specs_root, &id, "# Test\n", "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecListTool;
    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("testspec"));
}

#[tokio::test]
async fn test_spec_task_update_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Do X | — | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecTaskUpdateTool;
    let result = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await;
    assert!(result.is_ok(), "{:?}", result);

    // Verify task was updated
    let mgr2 = SpecManager::new(specs_root);
    let spec = mgr2.read_spec(&id).await.unwrap();
    let task = spec.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(task.status, ragent_specs::spec::TaskStatus::Completed);
    assert!(task.completed_at.is_some());
}

#[tokio::test]
async fn test_spec_search_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n\n## Requirements\n\n- FR-001: The system shall authenticate users.\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Add auth | FR-001 | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecSearchTool;
    let result = tool.execute(json!({"query": "authenticate"}), &ctx).await;
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.content.contains("testspec"));
    assert!(output.content.contains("authenticate"));
}

#[tokio::test]
async fn test_spec_search_no_results() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n\n## Requirements\n\n- FR-001: The system shall do X.\n";
    let plan_md = "# Plan\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecSearchTool;
    let result = tool
        .execute(json!({"query": "quantum_entanglement"}), &ctx)
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("No matching specs found"));
}

#[tokio::test]
async fn test_spec_coverage_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: in_progress\n---\n\n# Test Spec\n\n## Requirements\n\n- FR-001: The system shall do X.\n- FR-002: The system shall do Y.\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Implement X | FR-001 | S | High | — |\n| T-002 | Implement Y | FR-002 | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    // Mark T-001 as completed
    let mgr = SpecManager::new(specs_root);
    {
        let mut spec = mgr.read_spec(&id).await.unwrap();
        let task = spec.tasks.iter_mut().find(|t| t.id == "T-001").unwrap();
        task.status = ragent_specs::spec::TaskStatus::Completed;
        task.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        mgr.write_spec(&spec).await.unwrap();
    }

    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecCoverageTool;
    let result = tool.execute(json!({"spec_id": "testspec"}), &ctx).await;
    assert!(result.is_ok(), "{:?}", result);
    let output = result.unwrap();
    assert!(output.content.contains("Coverage Report"));
    assert!(output.content.contains("FR-001"));
    assert!(output.content.contains("FR-002"));
    assert!(output.content.contains("T-001"));
    assert!(output.content.contains("T-002"));
    // Verify metadata
    let meta = output.metadata.unwrap();
    assert_eq!(meta["spec_id"], "testspec");
    // coverage_pct is a number (may be serialized as integer or float depending on value)
    assert!(meta["coverage_pct"].is_number());
}

#[tokio::test]
async fn test_spec_coverage_invalid_id() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecCoverageTool;
    let result = tool
        .execute(json!({"spec_id": "!!!invalid!!!"}), &ctx)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid spec ID"));
}

#[tokio::test]
async fn test_spec_task_update_status_transitions() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let id = ragent_specs::spec::SpecId::new("testspec").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Test Spec\n";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|----|-------|-------------|--------|----------|--------------|\n| T-001 | Do X | — | S | High | — |\n";
    SpecIo::create_spec_dir(specs_root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(specs_root);

    // Transition: pending → in_progress
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr.clone()));
    let tool = SpecTaskUpdateTool;
    let result = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "in_progress"}),
            &ctx,
        )
        .await;
    assert!(result.is_ok(), "{:?}", result);

    // Verify task is now in_progress
    let spec = mgr.read_spec(&id).await.unwrap();
    let task = spec.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(task.status, ragent_specs::spec::TaskStatus::InProgress);

    // Transition: in_progress → completed
    let result = tool
        .execute(
            json!({"spec_id": "testspec", "task_id": "T-001", "status": "completed"}),
            &ctx,
        )
        .await;
    assert!(result.is_ok(), "{:?}", result);

    // Verify task is now completed
    let spec = mgr.read_spec(&id).await.unwrap();
    let task = spec.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(task.status, ragent_specs::spec::TaskStatus::Completed);
    assert!(task.completed_at.is_some());
}

#[tokio::test]
async fn test_spec_list_with_status_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    // Create two specs with different statuses
    let id1 = ragent_specs::spec::SpecId::new("spec-draft").unwrap();
    let id2 = ragent_specs::spec::SpecId::new("spec-approved").unwrap();
    SpecIo::create_spec_dir(
        specs_root,
        &id1,
        "---\nstatus: draft\n---\n\n# Draft Spec\n",
        "# Plan\n",
    )
    .await
    .unwrap();
    SpecIo::create_spec_dir(
        specs_root,
        &id2,
        "---\nstatus: approved\n---\n\n# Approved Spec\n",
        "# Plan\n",
    )
    .await
    .unwrap();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecListTool;

    // Filter by draft
    let result = tool.execute(json!({"status": "draft"}), &ctx).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.content.contains("spec-draft"));
    assert!(!output.content.contains("spec-approved"));
}

#[tokio::test]
async fn test_spec_read_with_invalid_id() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecReadTool;
    let result = tool.execute(json!({"spec_id": "!!!bad-id!!!"}), &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid spec ID"));
}

#[tokio::test]
async fn test_spec_read_nonexistent_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_root = tmp.path();

    let mgr = SpecManager::new(specs_root);
    let mut ctx = base_ctx();
    ctx.spec_manager = Some(Arc::new(mgr));

    let tool = SpecReadTool;
    let result = tool.execute(json!({"spec_id": "nonexistent"}), &ctx).await;
    assert!(result.is_err());
}
