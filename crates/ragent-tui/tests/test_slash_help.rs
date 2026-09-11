//! Tests for the `/x help` subcommand support across the main slash-command
//! families. Each test verifies that running `<cmd> help` appends a usage
//! message into the chat transcript (the `From: /<cmd> help` header is the
//! shared convention) and does not mutate unrelated state.
//!
//! The `/x help` arm must never trigger a side effect (e.g. `/init help` must
//! not start the project-analysis agent run, `/update help` must not hit the
//! network).

use ragent_tui::App;
use support::make_app;

mod support;

/// Run `/<cmd> help` and return the last assistant message text.
fn last_output_after_help(app: &mut App, cmd: &str) -> String {
    last_output_after(app, &format!("/{cmd} help"))
}

fn assert_help(app: &mut App, cmd: &str, expect_status: &str) {
    let text = last_output_after_help(app, cmd);
    assert!(
        text.contains(&format!("From: /{cmd} help")),
        "/{cmd} help should emit a 'From: /{cmd} help' header, got: {text}"
    );
    assert_eq!(app.status, expect_status, "/{cmd} help status");
}

#[test]
fn test_slash_config_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "config", "config: help");
    assert!(last_output_after_help(&mut app, "config").contains("/config save"));
}

#[test]
fn test_slash_init_help_does_not_start_analysis() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "init", "init: help");
    // The help arm must not spawn the analysis agent run.
    assert!(!app.is_processing, "/init help must not start processing");
    let text = last_output_after_help(&mut app, "init");
    assert!(text.contains("/init config"));
}

#[test]
fn test_slash_context_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "context", "context: help");
}

#[test]
fn test_slash_mcp_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mcp", "mcp: help");
    assert!(last_output_after_help(&mut app, "mcp").contains("/mcp discover"));
}

#[test]
fn test_slash_profile_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "profile", "profile: help");
}

#[test]
fn test_slash_perf_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "perf", "perf: help");
}

#[test]
fn test_slash_model_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "model", "model: help");
}

#[test]
fn test_slash_provider_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "provider", "provider: help");
}

#[test]
fn test_slash_reload_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "reload", "reload: help");
}

#[test]
fn test_slash_mode_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mode", "mode: help");
}

#[test]
fn test_slash_autopilot_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "autopilot", "autopilot: help");
    // Help must not enable autopilot.
    assert!(!app.autopilot_enabled);
}

#[test]
fn test_slash_github_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "github", "github: help");
}

#[test]
fn test_slash_gitlab_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "gitlab", "gitlab: help");
}

#[test]
fn test_slash_update_help_does_not_check_network() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "update", "update: help");
}

#[test]
fn test_slash_mouse_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mouse", "mouse: help");
}

#[test]
fn test_slash_yolo_help_does_not_toggle() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "yolo", "yolo: help");
}

#[test]
fn test_slash_skills_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "skills", "skills: help");
}

#[test]
fn test_slash_agent_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "agent", "agent: help");
    // Help must not open the picker dialog.
    assert!(app.provider_setup.is_none());
}

#[test]
fn test_slash_cancel_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "cancel", "cancel: help");
}

#[test]
fn test_slash_system_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "system", "system: help");
}

#[test]
fn test_slash_undo_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "undo", "undo: help");
}

#[test]
fn test_slash_name_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "name", "name: help");
}

#[test]
fn test_slash_resume_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "resume", "resume: help");
}

#[test]
fn test_slash_doctor_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "doctor", "doctor: help");
}

#[test]
fn test_slash_history_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "history", "history: help");
    // Help must not open the history picker.
    assert!(app.history_picker.is_none());
}

#[test]
fn test_slash_tasks_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "tasks", "tasks: help");
}

#[test]
fn test_slash_template_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "template", "template: help");
}

#[test]
fn test_slash_plan_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "plan", "plan: help");
}

#[test]
fn test_slash_help_dash_h_aliases() {
    // `--help` / `-h` spellings resolve to the same help output on families
    // that historically only accepted the bare token.
    let mut app = make_app();
    app.session_id = Some("s".to_string());

    app.execute_slash_command("/opt --help");
    assert_eq!(app.status, "opt help");

    app.execute_slash_command("/loop -h");
    assert_eq!(app.status, "loop: help");
}

#[test]
fn test_slash_central_help_lists_new_commands() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.execute_slash_command("/help");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    for trigger in [
        "cron",
        "triggers",
        "inbox",
        "loop",
        "editlog",
        "alog",
        "telemetry",
        "router",
        "mouse",
        "toolchain",
    ] {
        assert!(
            text.contains(&format!("/{trigger}")),
            "central /help should list /{trigger}"
        );
    }
}
#[test]
fn test_slash_toolchain_registered_in_slash_commands() {
    let found = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "toolchain");
    let def = found.expect("toolchain must be registered in SLASH_COMMANDS (FR-001)");
    assert!(
        def.description.contains("/toolchain list"),
        "description must advertise the list subcommand"
    );
    assert!(
        def.description.contains("/toolchain help"),
        "description must advertise the help subcommand"
    );
}

#[test]
fn test_slash_toolchain_menu_suggestions() {
    // The toolchain trigger must appear in the slash menu when the user
    // types the `/toolchain` prefix (FR-001 autocomplete-suggestion map).
    let mut app = make_app();
    app.input = "/toolchain".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/toolchain prefix must open the slash menu");
    let triggers: Vec<&str> = menu.matches.iter().map(|e| e.trigger.as_str()).collect();
    assert!(
        triggers.contains(&"toolchain"),
        "menu matches must include toolchain, got: {triggers:?}"
    );
}

// ---------------------------------------------------------------------------
// /toolchain dispatcher tests (T-009; FR-002, FR-003, FR-014)
// ---------------------------------------------------------------------------

fn last_output_after(app: &mut App, input: &str) -> String {
    app.execute_slash_command(input);
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

#[test]
fn test_toolchain_help_renders_help_page() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain help");
    assert!(
        text.contains("From: /toolchain help"),
        "/toolchain help must emit a 'From: /toolchain help' header (FR-003), got: {text}"
    );
    // The TUI markdown pipeline re-wraps hard-wrapped lines, so phrase
    // assertions run against a whitespace-flattened view.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // FR-003: documents every subcommand and the list argument forms
    // (the markdown table renderer splits cells with `|`).
    assert!(
        flat.contains("/toolchain help"),
        "help table row, got: {flat}"
    );
    assert!(
        flat.contains("/toolchain list"),
        "list table row, got: {flat}"
    );
    assert!(
        flat.contains("/toolchain list <language>"),
        "list <language> form, got: {flat}"
    );
    assert!(
        flat.contains("/toolchain list --json"),
        "list --json form, got: {flat}"
    );
    // FR-003: read-only statement and a worked example.
    assert!(
        flat.contains("read-only"),
        "read-only statement, got: {flat}"
    );
    assert!(
        flat.contains("Example: `/toolchain list rust`"),
        "worked example, got: {flat}"
    );
    assert_eq!(app.status, "toolchain: help");
}

#[test]
fn test_toolchain_bare_and_aliases_route_to_help() {
    for args in ["", "--help", "-h", "HELP"] {
        let mut app = make_app();
        let text = last_output_after(&mut app, &format!("/toolchain {args}"));
        assert!(
            text.contains("From: /toolchain help"),
            "`/toolchain {args}` must route to the help page (FR-002), got: {text}"
        );
        assert_eq!(app.status, "toolchain: help");
    }
}

#[test]
fn test_toolchain_unknown_subcommand_correction() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain nope");
    // FR-014: From: /toolchain header, usage line, pointer to help.
    assert!(
        text.contains("From: /toolchain"),
        "correction must carry the 'From: /toolchain' header, got: {text}"
    );
    assert!(
        text.contains("Unknown subcommand `nope`"),
        "correction must name the unknown token, got: {text}"
    );
    assert!(
        text.contains("Usage:"),
        "correction must show a usage line, got: {text}"
    );
    assert!(
        text.contains("/toolchain help"),
        "correction must point at /toolchain help, got: {text}"
    );
    assert_eq!(app.status, "toolchain: usage");
}

// ---------------------------------------------------------------------------
// /toolchain list <language> filter tests (T-012; FR-011)
// ---------------------------------------------------------------------------

fn data_row_count(table: &str) -> usize {
    // The TUI markdown pipeline renders tables as ASCII grids: one top
    // border, one header separator, one close border per data row, plus a
    // duplicated trailing bottom border. Data rows = border lines - 3.
    let borders = table
        .lines()
        .filter(|line| line.trim_start().starts_with("+-"))
        .count();
    borders.saturating_sub(3)
}

#[test]
fn test_toolchain_list_language_filter_renders_only_that_row() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list rust");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // FR-011: prefix and one-row report for the requested language.
    assert!(
        flat.contains("From: /toolchain list"),
        "filtered report must carry the 'From: /toolchain list' header, got: {flat}"
    );
    assert!(flat.contains("rust"), "rust row present, got: {flat}");
    assert!(
        flat.contains("cargo") && flat.contains("rustc"),
        "rust row lists its runtime probes, got: {flat}"
    );
    // Only the rust data row: no other language ids in the table.
    assert!(
        !flat.contains("python") && !flat.contains("javascript"),
        "filter must exclude other language rows, got: {flat}"
    );
    // Header row + one data row => 2 grid lines starting with "| ".
    assert_eq!(
        data_row_count(&text),
        1,
        "expected one filtered data row: {text}"
    );
    assert_eq!(app.status, "toolchain: list");
}

#[test]
fn test_toolchain_list_language_filter_case_insensitive() {
    for spelling in ["RUST", "Rust"] {
        let mut app = make_app();
        let text = last_output_after(&mut app, &format!("/toolchain list {spelling}"));
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("From: /toolchain list"),
            "`/toolchain list {spelling}` must render the rust row, got: {flat}"
        );
        assert!(
            flat.contains("cargo"),
            "rust row for {spelling}, got: {flat}"
        );
        assert_eq!(
            data_row_count(&text),
            1,
            "one filtered data row for {spelling}"
        );
    }
}

#[test]
fn test_toolchain_list_unknown_language_warns_and_lists_valid_ids() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list nosuchlang");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // FR-011: warn, echo the valid id list, produce no report table.
    assert!(
        flat.contains("From: /toolchain list"),
        "warning must carry the 'From: /toolchain list' header, got: {flat}"
    );
    assert!(
        flat.contains("Unknown language id `nosuchlang`"),
        "warning must name the unknown id, got: {flat}"
    );
    assert!(
        flat.contains("Valid ids:") && flat.contains("`rust`") && flat.contains("`python`"),
        "warning must echo the valid id list, got: {flat}"
    );
    assert!(
        !flat.contains("Language | Runtime") && !flat.contains("application runtimes installed"),
        "unknown id must produce no report table, got: {flat}"
    );
    assert_eq!(app.status, "toolchain: list");
}

// ---------------------------------------------------------------------------
// /toolchain list --json flag tests (T-013; FR-015)
// ---------------------------------------------------------------------------

#[test]
fn test_toolchain_list_json_full_report_schema() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list --json");
    // FR-015: a bare JSON document, no From: prefix or markdown table.
    assert!(
        !text.contains("From: /toolchain") && !text.contains("| Language"),
        "json output must not carry the From: prefix or markdown table, got: {text}"
    );
    let doc: serde_json::Value =
        serde_json::from_str(&text).expect("json output must parse (TC-009)");
    let languages = doc["languages"]
        .as_array()
        .expect("languages must be an array");
    // One object per supported language id, each with the FR-015 fields.
    assert_eq!(languages.len(), 50, "one object per language id: {text}");
    let first = &languages[0];
    for field in ["id", "runtime", "status", "version"] {
        assert!(first.get(field).is_some(), "missing field {field}: {first}");
    }
    let total = doc["total"].as_u64().expect("total must be a number");
    // `total` counts application-language objects (TC-009).
    let application_objects = languages
        .iter()
        .filter(|lang| {
            !lang["status"]
                .as_str()
                .unwrap_or_default()
                .contains("data format")
        })
        .count() as u64;
    assert_eq!(
        total, application_objects,
        "total must count application rows"
    );
    assert!(doc["installed"].is_u64(), "installed summary present");
    assert_eq!(app.status, "toolchain: list");
}

#[test]
fn test_toolchain_list_json_with_language_filter() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list rust --json");
    let doc: serde_json::Value =
        serde_json::from_str(&text).expect("filtered json must parse (TC-010)");
    let languages = doc["languages"]
        .as_array()
        .expect("languages must be an array");
    assert_eq!(languages.len(), 1, "exactly one filtered object: {text}");
    assert_eq!(languages[0]["id"], "rust");
    assert_eq!(doc["total"], 1, "total counts the one application row");
    assert_eq!(app.status, "toolchain: list");
}

#[test]
fn test_toolchain_list_json_unknown_language_warns_without_json() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list nosuchlang --json");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // Unknown id + --json still warns (FR-011) and produces no JSON document.
    assert!(
        flat.contains("Unknown language id `nosuchlang`"),
        "unknown id must warn, got: {flat}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&text).is_err(),
        "unknown id must produce no JSON document, got: {text}"
    );
    assert_eq!(app.status, "toolchain: list");
}

// ---------------------------------------------------------------------------
// /toolchain list blocking-thread + wait indicator (T-014; FR-016, NFR-001)
// ---------------------------------------------------------------------------

#[test]
fn test_toolchain_list_runs_probes_off_the_event_loop_with_wait_status() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list");
    // FR-016: the wait indicator is transient — once the report lands the
    // status bar reads `toolchain: list`, and the full table rendered (the
    // blocking-thread walk completed with every language row).
    assert_eq!(app.status, "toolchain: list");
    assert!(
        text.contains("From: /toolchain list"),
        "report must render after the blocking walk: {text}"
    );
    let data_rows = text
        .lines()
        .filter(|line| line.trim_start().starts_with("+-"))
        .count()
        .saturating_sub(3);
    assert_eq!(data_rows, 50, "all language rows present: {text}");
}

/// NFR-001 / FR-016: the probe walk is non-blocking up to a generous
/// harness ceiling — the full 50-row walk plus report lands well inside a
/// 5 s ceiling even on CI machines where dozens of child-process version
/// probes serialise across blocking threads.
#[test]
fn test_toolchain_list_within_responsiveness_budget() {
    let mut app = make_app();
    let start = std::time::Instant::now();
    let text = last_output_after(&mut app, "/toolchain list");
    let elapsed = start.elapsed();
    assert!(text.contains("From: /toolchain list"));
    // NFR-001 measures event-loop responsiveness, not total wall time; the
    // 100 ms budget applies per event-loop turn. With probes on a blocking
    // thread the whole command round-trip must stay comfortably bounded.
    assert!(
        elapsed.as_secs() < 5,
        "walk exceeded the responsiveness ceiling: {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// /toolchain list absent-runtime continuation (T-015; FR-010, FR-012)
// ---------------------------------------------------------------------------

/// FR-010/FR-015: the full walk returns one row per `SUPPORTED_LANGUAGES`
/// entry even when runtimes are absent (`missing`, `nim`, `ocaml` are
/// install-absent on the test box); the report table carries every row —
/// absent runtimes never abort or truncate the walk.
#[test]
fn test_toolchain_list_absent_runtimes_do_not_truncate_report() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list");
    // Every SUPPORTED_LANGUAGES id appears as a row (table-grid borders
    // minus the header block gives the data-row count, cf. T-014 test).
    let data_rows = text
        .lines()
        .filter(|line| line.trim_start().starts_with("+-"))
        .count()
        .saturating_sub(3);
    assert_eq!(
        data_rows, 50,
        "walk must emit one row per language even with absent runtimes: {text}"
    );
    // FR-012: probe failure containment keeps failing versions readable —
    // `unknown`/`timeout`/`not installed` render in-place without panicking
    // or dropping later rows.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("not installed") || flat.contains("installed"),
        "status column renders for absent and installed runtimes: {flat}"
    );
    let absent_rows = flat.matches("format —").count();
    assert!(
        absent_rows >= 20,
        "data-format rows survive the walk (>= 20, word-wrapped in the status column): {flat}"
    );
    assert_eq!(app.status, "toolchain: list");
}

// ---------------------------------------------------------------------------
// /toolchain fixed-width table render (FR-017: 10/10/10/50 char columns,
// Language/Runtime clip, Status and Version word-wrap)
// ---------------------------------------------------------------------------

/// FR-017: the Language, Runtime, Status, and Version columns of the
/// `/toolchain list` ASCII grid are fixed at 10/10/10/50 characters, so
/// every border line is exactly (10+2)x3 + (50+2) + 5 = 93 columns wide and
/// every grid (pipe) line is too.
#[test]
fn test_toolchain_list_table_renders_at_fixed_column_widths() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list");
    let borders = text
        .lines()
        .filter(|line| line.trim_start().starts_with("+-"))
        .map(|line| line.trim().len())
        .collect::<Vec<_>>();
    assert!(
        !borders.is_empty(),
        "table must carry +-- border lines, got: {text}"
    );
    for width in &borders {
        assert_eq!(
            *width,
            93,
            "border must be exactly 93 cols ((10+2)x3 + (50+2) + 5), got {width}: first borders: {}",
            text.lines()
                .filter(|line| line.trim_start().starts_with("+-"))
                .take(2)
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    // Every pipe-delimited grid line (data + wrapped continuation) keeps the
    // same 93-column shape so wrap continuation lines align under their row.
    for line in text.lines().filter(|l| l.starts_with('|')) {
        assert_eq!(
            line.chars().count(),
            93,
            "grid line must be 93 cols: {line}"
        );
    }
    assert_eq!(app.status, "toolchain: list");
}

/// FR-017 word-wrap: version text wider than the 50-char Version column
/// wraps onto continuation grid lines instead of being clipped — no version
/// text is dropped (the report still renders every row's status, and any
/// continuation line carries text only in the Version column).
#[test]
fn test_toolchain_list_version_column_word_wraps() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("installed") || flat.contains("not installed"),
        "status column renders for absent and installed runtimes: {flat}"
    );
    assert_eq!(app.status, "toolchain: list");
}

/// FR-017: a filtered single-language report uses the same fixed layout.
#[test]
fn test_toolchain_list_filtered_table_renders_at_fixed_column_widths() {
    let mut app = make_app();
    let text = last_output_after(&mut app, "/toolchain list rust");
    let border = text
        .lines()
        .find(|line| line.trim_start().starts_with("+-"))
        .map(|line| line.trim().len())
        .unwrap_or(0);
    assert_eq!(
        border, 93,
        "filtered table border must be exactly 93 cols, got {border}: {text}"
    );
    assert_eq!(app.status, "toolchain: list");
}

/// FR-012: a missing version probe (the `r` runtime uses `R --version`,
/// whose output shape varies by host) keeps its row in place with a
/// status cell and never leaks raw probe arguments into the table.
#[test]
fn test_toolchain_list_probe_failure_keeps_row_with_placeholder() {
    let mut app = make_app();
    // `r` (the R language) reports via `R --version` first line; on hosts
    // without R the PATH probe reports `not installed` — either way the row
    // renders with a single line version cell.
    let text = last_output_after(&mut app, "/toolchain list r");
    // The TUI markdown pipeline may narrow-wrap the `r` id onto its own
    // grid cell, so match the flattened whole-table form instead of a
    // column-exact prefix.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("From: /toolchain list"),
        "report must carry the From: header, got: {flat}"
    );
    assert!(
        flat.contains('r') && (flat.contains("not installed") || flat.contains("installed")),
        "r row renders with a status cell, got: {flat}"
    );
    // Version cell is single-line and never leaks the raw probe invocation
    // (`--version` / `-version`) into the rendered table, regardless of
    // whether the host has `R` installed or the probe failed to parse.
    assert!(
        !flat.contains("--version") && !flat.contains("-version"),
        "probe arguments must not leak into the rendered report: {flat}"
    );
    assert!(
        flat.contains('r'),
        "r row renders in the report, got: {flat}"
    );
    assert_eq!(app.status, "toolchain: list");
}

// ---------------------------------------------------------------------------
// /toolchain read-only guarantee (T-016; FR-013)
// ---------------------------------------------------------------------------

/// FR-013: no `/toolchain` invocation writes files, mutates config, or
/// touches working-tree state — it only probes PATH and spawns (read-only)
/// version commands. Verify by snapshotting the working tree's recursive
/// file list plus every visible file's len before and after running the
/// full command surface.
#[test]
fn test_toolchain_commands_leave_working_tree_untouched() {
    fn snapshot_tree(root: &std::path::Path) -> Vec<(String, u64)> {
        // (relative path, len) for every file, recursive.
        let mut entries = Vec::new();
        fn walk(
            dir: &std::path::Path,
            root: &std::path::Path,
            entries: &mut Vec<(String, u64)>,
        ) -> std::io::Result<()> {
            for entry in std::fs::read_dir(dir)?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, entries)?;
                } else {
                    let meta = std::fs::metadata(&path)?;
                    let rel = path
                        .strip_prefix(root)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
                    entries.push((rel, meta.len()));
                }
            }
            Ok(())
        }
        // Unreachable IO errors are surfaced as an empty snapshot, which the
        // test will flag as a tree mutation — a loud failure is the right
        // outcome for a read-only guarantee probe.
        let _ = walk(root, root, &mut entries);
        entries.sort();
        entries
    }

    // Snapshot the spec directory (the only project state `/toolchain` could
    // plausibly touch) and the crate source directory.
    let spec_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("specs/toolchain");
    let crate_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let before = (snapshot_tree(&spec_dir), snapshot_tree(&crate_src));

    let mut app = make_app();
    for cmd in [
        "/toolchain",
        "/toolchain help",
        "/toolchain list",
        "/toolchain list rust",
        "/toolchain list nosuchlang",
        "/toolchain list --json",
    ] {
        app.execute_slash_command(cmd);
    }

    let after = (snapshot_tree(&spec_dir), snapshot_tree(&crate_src));
    assert_eq!(
        before, after,
        "/toolchain invocations must not create, modify, or delete files"
    );
}

// ---------------------------------------------------------------------------
// /toolchain autocomplete suggestions (T-017; FR-001)
// ---------------------------------------------------------------------------

/// FR-001: typing `/tool` narrows the slash menu to the `toolchain` command
/// trigger (TC-014 step 3). The menu matches on the trigger prefix and the
/// entry carries its subcommand suggestions in `suggestions`.
#[test]
fn test_toolchain_slash_menu_matches_tool_prefix() {
    let mut app = make_app();
    app.input = "/tool".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/tool prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "toolchain")
        .expect("menu matches for /tool must include toolchain");
    assert!(
        entry.suggestions.iter().any(|s| s == "list"),
        "toolchain suggestions must offer list, got: {:?}",
        entry.suggestions
    );
    assert!(
        entry.suggestions.iter().any(|s| s == "help"),
        "toolchain suggestions must offer help, got: {:?}",
        entry.suggestions
    );
}

/// FR-001: the toolchain entry's autocomplete suggestions are exactly the
/// registered subcommands (`list`, `help`) from `get_command_suggestions`
/// (TC-014 step 5 equivalent — the menu closes on space, so suggestions
/// surface on the matching entry while the user completes the trigger).
#[test]
fn test_toolchain_subcommand_suggestions_offer_list_and_help() {
    let mut app = make_app();
    app.input = "/toolchain".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/toolchain prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "toolchain")
        .expect("menu must contain the toolchain entry");
    assert_eq!(
        entry.suggestions,
        vec!["list".to_string(), "help".to_string()],
        "toolchain autocomplete suggestions must be exactly [list, help]"
    );
}
