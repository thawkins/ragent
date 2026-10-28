//! Guard: the TUI's two surface enumerations agree with the registry
//! (spec `uniformtools` FR-030, task T-037).
//!
//! FR-030 names six enumerations that must stay consistent with the registry.
//! Four of them are reachable from `ragent-agent` and live in
//! `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` (`--dry-run` in
//! both encodings, `tool_info`, `commands_info` and the outbound provider list).
//! The remaining two are TUI render paths and this crate cannot reach them from
//! the agent side, so they are pinned here:
//!
//! | # | Case | Surface it pins |
//! |---|---|---|
//! | 1 | `test_tools_list_reports_the_fr030_total_and_every_name` | `/tools list` header + rows |
//! | 2 | `test_bug_report_records_the_fr030_total` | the `Tools registered` line in `/bug-report` |
//!
//! The numbers come from `specs/uniformtools/T-041-BASELINE.md`, measured
//! before the rename. This file re-derives them from the live registry as well,
//! so a failing case says whether the *count* moved (a tool was added, removed,
//! merged or split) or only a *renderer* drifted from the registry - the two
//! different defects FR-030 forbids in one sentence.
//!
//! ## Two facts measured on this tree, not assumed from the baseline
//!
//! - **`/tools list` renders the whole registry in one table.** Its header is
//!   `Tools (N total):` where `N` is `definitions().len()`. There is no second,
//!   disabled block: tool-family visibility was removed and every registered
//!   tool is advertised, so the rows and the registry are the same set.
//! - **The `Tools registered` line FR-030 names did not exist.** `handle_bug_report`
//!   used to write only `- **Tools visible**: {n}`, where `n` was the count of
//!   *enabled family switches* out of seven - 1 on the shipped default
//!   configuration, and never a tool count (T-041 §4). `handle_bug_report` now
//!   writes `- **Tools registered**: {registry.list().len()}` as the first line of
//!   `## Tool Configuration`; the switch-count line went with the switches.
//!
//! ## What this file deliberately does not check
//!
//! - **The `/tools` table's column layout.** `test_slash_tools.rs` owns the
//!   command's behaviour (bare `/tools`, `/tools show`, `/tools help`, the error
//!   arms); this file reads the rendered rows only to compare them with the
//!   registry.
//! - **The completion list.** FR-033's "no retired spelling is ever completed" is
//!   T-045's.
//! - **`/bug-report`'s redaction and content.** `test_slash_deep2.rs` owns those;
//!   this file reads the one line FR-030 names.

#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// The FR-030 surface size `specs/uniformtools/T-041-BASELINE.md` measured
/// before the rename, plus the `DateTime` tool added afterwards. Pinned as a
/// number for the same reason the agent-side file pins it: a derived
/// expectation would agree with whatever the tree did.
const FR030_TOTAL: usize = 149;

/// An app in an isolated project with a live session.
///
/// The isolation harness returns the crate-wide env guard, which also serialises
/// this test against every other env-mutating test in the binary, so no second
/// lock is taken here (`std::sync::Mutex` is not reentrant).
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// The registry names as the app's session sees them, ascending.
fn registry_names(app: &App) -> Vec<String> {
    app.session_processor.tool_registry.list()
}

#[tokio::test]
async fn test_tools_list_reports_the_fr030_total_and_every_name() {
    let (_guard, _env, _temp, mut app) = isolated_app();
    let registry = registry_names(&app);
    assert_eq!(
        registry.len(),
        FR030_TOTAL,
        "FR-030: the registry itself holds {} tools, not the {FR030_TOTAL} the \
         baseline records",
        registry.len()
    );

    app.execute_slash_command("/tools list").await;
    assert_eq!(app.status, "tools");
    let rendered = last_message(&app);

    // The header is `Tools (N total):`, where `N` is `definitions().len()`.
    let header = rendered
        .lines()
        .find(|line| line.starts_with("Tools ("))
        .unwrap_or_else(|| {
            panic!("FR-030: `/tools list` has no `Tools (N total):` header: {rendered:?}")
        });
    let numbers: Vec<usize> = header
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();
    assert_eq!(
        numbers.len(),
        1,
        "FR-030: could not read the tool count from {header:?}"
    );
    assert_eq!(
        numbers[0], FR030_TOTAL,
        "FR-030: `/tools list` reports {} tools, not the {FR030_TOTAL} baseline: \
         {header:?}",
        numbers[0]
    );

    // Read every data row of every tool table in the message. The message window
    // renders markdown to ASCII, so a ```text fence survives only as a leading
    // backtick on the heading row and as a lone "`" line at the close. A tool table
    // is the block under a heading that names all three columns - the family
    // visibility table above it has only two, so the heading is what distinguishes
    // them.
    let lines: Vec<&str> = rendered.lines().collect();
    let mut listed: Vec<String> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let is_heading = lines[index].contains("name")
            && lines[index].contains("source")
            && lines[index].contains("description");
        if !is_heading {
            index += 1;
            continue;
        }
        // Skip the heading and its underline.
        index += 2;
        while index < lines.len() {
            let trimmed = lines[index].trim();
            if trimmed.is_empty() || trimmed == "`" {
                break;
            }
            let name = lines[index].split_whitespace().next().unwrap_or_default();
            assert!(
                !name.is_empty(),
                "FR-030: `/tools list` rendered a row with no tool name: {:?}",
                lines[index]
            );
            listed.push(name.to_string());
            index += 1;
        }
        index += 1;
    }

    assert_eq!(
        listed.len(),
        FR030_TOTAL,
        "FR-030: `/tools list` rendered {} rows, not the {FR030_TOTAL} the header \
         accounts for",
        listed.len()
    );
    listed.sort();
    assert_eq!(
        listed, registry,
        "FR-030: `/tools list` enumerates a different set of tools than the registry \
         it renders"
    );
    assert_eq!(
        listed
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        FR030_TOTAL,
        "FR-030: `/tools list` rendered a duplicate tool row"
    );
}

#[tokio::test]
async fn test_bug_report_records_the_fr030_total() {
    let (_guard, _env, temp, mut app) = isolated_app();
    assert_eq!(registry_names(&app).len(), FR030_TOTAL);

    app.execute_slash_command("/bug-report").await;
    assert_eq!(
        app.status,
        "bug-report: generated",
        "/bug-report did not write a report: {:?}",
        last_message(&app)
    );

    let log_dir = temp.path().join("log");
    let report_path = std::fs::read_dir(&log_dir)
        .expect("log dir exists")
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("bug-report-")
        })
        .expect("bug-report file written")
        .path();
    let body = std::fs::read_to_string(&report_path).expect("read report");

    let line = body
        .lines()
        .find(|line| line.contains("**Tools registered**"))
        .unwrap_or_else(|| {
            panic!("FR-030: the bug report has no `Tools registered` line:\n{body}")
        });
    assert_eq!(
        line.trim(),
        format!("- **Tools registered**: {FR030_TOTAL}"),
        "FR-030: `/bug-report` records {line:?}, not the {FR030_TOTAL} baseline"
    );

    // The report has a `## Tool Configuration` section, and the count is its
    // first line - that is what T-041 §5 reads to produce this number.
    let section_index = body
        .lines()
        .position(|line| line == "## Tool Configuration")
        .expect("FR-030: the report has a `## Tool Configuration` section");
    let first_item = body
        .lines()
        .skip(section_index + 1)
        .find(|line| !line.trim().is_empty())
        .expect("the section is not empty");
    assert!(
        first_item.contains(&FR030_TOTAL.to_string()),
        "FR-030: the first line of `## Tool Configuration` is {first_item:?}, which \
         does not carry the {FR030_TOTAL} baseline"
    );
}
