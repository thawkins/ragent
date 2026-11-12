//! The reports behind `ragent session list` and `ragent project` (spec
//! `projects`, T-011).

#[path = "support/project_fixture.rs"]
mod project_fixture;

use std::path::PathBuf;

use project_fixture::{Managers, managers};
use ragent_agent::project::report::{
    grouped_sessions_json, grouped_sessions_text, project_text, projects_text,
};
use ragent_agent::project::{ProjectRemote, RemoteKind};
use ragent_storage::SessionCheckout;

/// Two projects: `alpha` (GitHub remote, base model, one plain session and
/// one on a branch with three requests) and `beta` (one session), plus an
/// empty project `gamma`.
fn seeded() -> (Managers, String, String) {
    let m = managers();
    let alpha = m
        .projects
        .resolve_for_dir(std::path::Path::new("/nonexistent/report/alpha"))
        .expect("alpha");
    m.storage
        .set_project_remote(
            &alpha.id,
            Some(
                &ProjectRemote {
                    kind: RemoteKind::GitHub,
                    host: "github.com".to_string(),
                    path: "acme/alpha".to_string(),
                    url: "https://github.com/acme/alpha.git".to_string(),
                    remote_name: "origin".to_string(),
                    default_branch: Some("main".to_string()),
                }
                .to_row(),
            ),
        )
        .expect("remote");
    m.storage
        .set_project_base_model(&alpha.id, Some(("anthropic", "claude-base")))
        .expect("model");
    let plain = m
        .sessions
        .create_session(PathBuf::from("/nonexistent/report/alpha"))
        .expect("plain");
    m.storage
        .update_session(&plain.id, "Plain work")
        .expect("title");
    let branched = m
        .sessions
        .create_session_in_project(
            &alpha.id,
            PathBuf::from("/nonexistent/report/trees/x"),
            &SessionCheckout {
                branch: Some("feature/x".to_string()),
                worktree: Some("/nonexistent/report/trees/x".to_string()),
                pr_count: Some(3),
            },
        )
        .expect("branched");
    m.storage
        .update_session(&branched.id, "Branch work")
        .expect("title");
    let beta = m
        .sessions
        .create_session(PathBuf::from("/nonexistent/report/beta"))
        .expect("beta session");
    m.storage
        .update_session(&beta.id, "Beta work")
        .expect("title");
    m.storage
        .ensure_project("/nonexistent/report/gamma")
        .expect("gamma");
    (m, alpha.id, branched.id)
}

#[test]
fn test_project_report_sessions_are_listed_under_their_project() {
    let (m, _alpha, branched) = seeded();

    let text = grouped_sessions_text(&m.storage, None).expect("text");

    let alpha_heading = text
        .find("alpha (/nonexistent/report/alpha) [github acme/alpha]")
        .unwrap_or_else(|| panic!("alpha heading missing:\n{text}"));
    let beta_heading = text
        .find("beta (/nonexistent/report/beta)")
        .unwrap_or_else(|| panic!("beta heading missing:\n{text}"));
    let plain = text.find("Plain work").expect("plain session");
    let branch = text.find("Branch work").expect("branch session");
    let beta = text.find("Beta work").expect("beta session");
    let between = |pos: usize, start: usize, end: usize| pos > start && pos < end;
    if alpha_heading < beta_heading {
        assert!(between(plain, alpha_heading, beta_heading), "{text}");
        assert!(between(branch, alpha_heading, beta_heading), "{text}");
        assert!(beta > beta_heading, "{text}");
    } else {
        assert!(between(beta, beta_heading, alpha_heading), "{text}");
        assert!(plain > alpha_heading && branch > alpha_heading, "{text}");
    }
    let branch_line = text
        .lines()
        .find(|l| l.contains("Branch work"))
        .expect("line");
    assert!(
        branch_line.starts_with(&format!("  {} | ", &branched[..8])),
        "{branch_line}"
    );
    assert!(
        branch_line.contains("feature/x (3 requests)"),
        "{branch_line}"
    );
    assert!(
        !text.contains("gamma"),
        "a project with no sessions has nothing to list:\n{text}"
    );
}

#[test]
fn test_project_report_filter_limits_to_one_project() {
    let (m, alpha, _branched) = seeded();

    for key in ["alpha", "ALPHA", alpha.as_str(), &alpha[..10]] {
        let text = grouped_sessions_text(&m.storage, Some(key)).expect("text");
        assert!(
            text.contains("Plain work") && text.contains("Branch work"),
            "{key}:\n{text}"
        );
        assert!(!text.contains("Beta work"), "{key}:\n{text}");
    }
    assert_eq!(
        grouped_sessions_text(&m.storage, Some("gamma")).expect("text"),
        "No sessions found\n"
    );
    let err = grouped_sessions_text(&m.storage, Some("nope")).expect_err("unknown project");
    assert!(err.to_string().contains("no project named 'nope'"), "{err}");
}

#[test]
fn test_project_report_json_carries_project_and_checkout() {
    let (m, alpha, branched) = seeded();

    let json = grouped_sessions_json(&m.storage, Some("alpha")).expect("json");

    let groups = json.as_array().expect("array");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["project"]["id"], alpha);
    assert_eq!(groups[0]["project"]["remote"]["kind"], "github");
    assert_eq!(
        groups[0]["project"]["base_model"]["model_id"],
        "claude-base"
    );
    let sessions = groups[0]["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 2);
    let on_branch = sessions
        .iter()
        .find(|s| s["id"] == branched)
        .expect("branched session");
    assert_eq!(on_branch["branch"], "feature/x");
    assert_eq!(on_branch["requests"], 3);
    assert_eq!(on_branch["worktree"], "/nonexistent/report/trees/x");
    let plain = sessions
        .iter()
        .find(|s| s["id"] != branched)
        .expect("plain");
    assert_eq!(plain["branch"], serde_json::Value::Null);

    let all = grouped_sessions_json(&m.storage, None).expect("json");
    assert_eq!(
        all.as_array().expect("array").len(),
        3,
        "the unfiltered JSON lists every project, the empty one included"
    );
}

#[test]
fn test_project_report_projects_and_project_text() {
    let (m, alpha, _branched) = seeded();

    let list = projects_text(&m.storage).expect("text");
    let alpha_line = list
        .lines()
        .find(|l| l.contains("| alpha |"))
        .expect("alpha");
    assert!(alpha_line.starts_with(&alpha[..8]), "{alpha_line}");
    assert!(alpha_line.contains("github acme/alpha"), "{alpha_line}");
    assert!(alpha_line.contains("anthropic/claude-base"), "{alpha_line}");
    assert!(alpha_line.ends_with("2 session(s)"), "{alpha_line}");
    let gamma_line = list
        .lines()
        .find(|l| l.contains("| gamma |"))
        .expect("gamma");
    assert!(gamma_line.ends_with("0 session(s)"), "{gamma_line}");

    let project = m.projects.get(&alpha).expect("get").expect("alpha");
    let text = project_text(&project, 2);
    assert!(text.contains("Project:    alpha"), "{text}");
    assert!(
        text.contains("Remote:     github acme/alpha on github.com (default branch main)"),
        "{text}"
    );
    assert!(text.contains("Base model: anthropic/claude-base"), "{text}");
    assert!(text.contains("Sessions:   2"), "{text}");

    let empty = managers();
    assert_eq!(
        projects_text(&empty.storage).expect("text"),
        "No projects found\n"
    );
    assert_eq!(
        grouped_sessions_text(&empty.storage, None).expect("text"),
        "No sessions found\n"
    );
}
