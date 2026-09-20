//! Integration tests for the dry-run readiness report (T-022).
//!
//! These tests exercise the public [`run_dry_run`] function against temporary
//! configuration directories so that global/project config files in the
//! repository do not influence the verdict. Each test asserts both the
//! reported [`ReadinessVerdict`] and the suggested exit code.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::{
    dry_run::{DryRunInputs, ReadinessStatus, ReadinessVerdict},
    provider::create_default_registry as create_provider_registry,
    tool::create_default_registry as create_tool_registry,
};
use tempfile::TempDir;

/// Write `contents` to `dir/ragent.json` and return the file path.
fn write_config(dir: &TempDir, contents: &str) -> PathBuf {
    let path = dir.path().join("ragent.json");
    std::fs::write(&path, contents).expect("failed to write temp config");
    path
}

/// Build a default set of registries and inputs for a temp directory.
fn dry_run_inputs(dir: &TempDir, config_path: Option<PathBuf>) -> DryRunInputs {
    DryRunInputs {
        config_path,
        agent_name: "general".to_string(),
        model_override: None,
        provider_registry: Arc::new(create_provider_registry()),
        tool_registry: Arc::new(create_tool_registry()),
        working_dir: dir.path().to_path_buf(),
        hidden_tools: Vec::new(),
    }
}

#[tokio::test]
async fn test_dry_run_known_good_ready_exit_0() {
    let dir = TempDir::new().expect("failed to create temp dir");
    // Use an explicit provider config with no required environment variables so
    // the provider section is READY regardless of the host environment.
    let config = r#"
{
  "defaultAgent": "general",
  "provider": {
    "anthropic": {
      "env": []
    }
  }
}
"#;
    let path = write_config(&dir, config);
    let inputs = dry_run_inputs(&dir, Some(path));

    let (report, exit_code) = ragent_agent::dry_run::run_dry_run(inputs).await;

    assert_eq!(
        report.verdict,
        ReadinessVerdict::Ready,
        "known-good config should report READY; sections: {:?}",
        report.sections
    );
    assert_eq!(exit_code, 0, "READY should map to exit code 0");
}

#[tokio::test]
async fn test_dry_run_broken_mcp_blocked_exit_1() {
    let dir = TempDir::new().expect("failed to create temp dir");
    // A reachable loopback address on a port that is almost certainly closed.
    let config = r#"
{
  "defaultAgent": "general",
  "provider": {
    "anthropic": {
      "env": []
    }
  },
  "mcp": {
    "broken": {
      "type": "http",
      "url": "http://127.0.0.1:1/sse"
    }
  }
}
"#;
    let path = write_config(&dir, config);
    let inputs = dry_run_inputs(&dir, Some(path));

    let (report, exit_code) = ragent_agent::dry_run::run_dry_run(inputs).await;

    assert_eq!(
        report.verdict,
        ReadinessVerdict::Blocked,
        "unreachable HTTP MCP server should block the deployment; sections: {:?}",
        report.sections
    );
    assert_eq!(exit_code, 1, "BLOCKED should map to exit code 1");

    let mcp_section = report
        .sections
        .iter()
        .find(|s| s.name == "mcp")
        .expect("mcp section should be present");
    assert_eq!(mcp_section.status, ReadinessStatus::Blocked);
    assert!(mcp_section.items.iter().any(|item| item.name == "broken"));
}

#[tokio::test]
async fn test_dry_run_missing_skill_warning_exit_0() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let missing_dir = dir.path().join("missing-skills");
    // The directory is intentionally absent.
    assert!(!missing_dir.exists());
    let config = format!(
        r#"
{{
  "defaultAgent": "general",
  "provider": {{
    "anthropic": {{
      "env": []
    }}
  }},
  "skill_dirs": ["{}"]
}}
"#,
        missing_dir.display().to_string().replace('\\', "/")
    );
    let path = write_config(&dir, &config);
    let inputs = dry_run_inputs(&dir, Some(path));

    let (report, exit_code) = ragent_agent::dry_run::run_dry_run(inputs).await;

    assert_eq!(
        report.verdict,
        ReadinessVerdict::Warning,
        "missing configured skill directory should warn; sections: {:?}",
        report.sections
    );
    assert_eq!(exit_code, 0, "WARNING should map to exit code 0");

    let skills_section = report
        .sections
        .iter()
        .find(|s| s.name == "skills")
        .expect("skills section should be present");
    assert_eq!(skills_section.status, ReadinessStatus::Warning);
    let missing_name = missing_dir.display().to_string();
    assert!(
        skills_section
            .items
            .iter()
            .any(|item| item.name == missing_name && item.status == ReadinessStatus::Warning)
    );
}
