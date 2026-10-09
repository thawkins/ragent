//! Inline tests for `toolchain.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

/// Build one expected grid line from `(width, text)` column pairs: each
/// segment is clipped to its column width and right-padded with spaces,
/// mirroring [`TABLE_COLUMN_WIDTHS`] rendering (`| seg | seg | ... |`).
fn grid_line(columns: [(usize, &str); 4]) -> String {
    let mut line = String::from("|");
    for (width, text) in columns {
        let text: String = text.chars().take(width).collect();
        let pad = width.saturating_sub(text.chars().count());
        line.push(' ');
        line.push_str(&text);
        line.push_str(&" ".repeat(pad));
        line.push(' ');
        line.push('|');
    }
    line
}

/// T-002 primary guarantee: the exported exhaustiveness check reports no
/// violations for the current table.
#[test]
fn exhaustiveness_check_reports_no_violations() {
    assert_eq!(exhaustiveness_errors(), Vec::<String>::new());
}

/// Every scanner language id must have exactly one mapping row.
///
/// This is the T-002 exhaustiveness guarantee expressed as a test; it
/// lives here so the const table cannot drift from the scanner list
/// without a CI-visible failure.
#[test]
fn every_supported_language_has_a_mapping() {
    for lang in SUPPORTED_LANGUAGES {
        let match_count = LANGUAGE_RUNTIMES
            .iter()
            .filter(|(id, _, _)| *id == *lang)
            .count();
        assert_eq!(match_count, 1, "language {lang:?} must map exactly once");
    }
}

/// No mapping row may reference a language id outside the scanner list.
#[test]
fn no_mapping_rows_outside_scanner_list() {
    for (id, _, _) in LANGUAGE_RUNTIMES {
        assert!(
            SUPPORTED_LANGUAGES.contains(id),
            "mapping id {id:?} is not in SUPPORTED_LANGUAGES"
        );
    }
}

/// The FR-005 partition is exactly 30 application languages and 20
/// data-format entries over 50 canonical ids.
#[test]
fn partition_counts_match_fr005() {
    let application = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::Application)
        .count();
    let data_format = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::DataFormat)
        .count();
    assert_eq!(application, 30, "FR-005 application partition");
    assert_eq!(data_format, 20, "FR-005 data-format partition");
    assert_eq!(
        application + data_format,
        SUPPORTED_LANGUAGES.len(),
        "rows must total the scanner list length"
    );
}

/// `supported_language_rows` walks the scanner list in list order and
/// resolves every id through the mapping table (FR-004/FR-005 derivation).
#[test]
fn supported_language_rows_walk_in_scanner_order() {
    let rows: Vec<(&str, Option<(LanguageClass, Option<LanguageRuntime>)>)> =
        supported_language_rows().collect();
    assert_eq!(
        rows.len(),
        SUPPORTED_LANGUAGES.len(),
        "one row per scanner id"
    );
    for (index, (id, resolved)) in rows.iter().enumerate() {
        assert_eq!(*id, SUPPORTED_LANGUAGES[index], "list order preserved");
        assert!(
            resolved.is_some(),
            "every scanner id must resolve via lookup (exhaustive mapping)"
        );
    }
    // The first scanner id is `rust`; spot-check the classification path.
    assert_eq!(rows[0].0, "rust");
    let (class, runtime) = rows[0].1.expect("rust resolves");
    assert_eq!(class, LanguageClass::Application);
    let (want_class, want_runtime) = lookup("rust").unwrap();
    assert_eq!(class, want_class);
    assert_eq!(
        runtime.map(|rt| rt.commands),
        want_runtime.map(|rt| rt.commands)
    );
}

/// The exhaustiveness checker's counting logic detects a gap: a table
/// with one mapping row removed leaves some scanner id with zero rows.
/// (The real table's clean state is asserted by
/// `exhaustiveness_check_reports_no_violations`.)
#[test]
fn exhaustiveness_detects_gap_shape() {
    let mut table: Vec<(&str, LanguageClass, Option<LanguageRuntime>)> = LANGUAGE_RUNTIMES.to_vec();
    table.remove(0);
    let gap_detected = SUPPORTED_LANGUAGES
        .iter()
        .any(|lang| !table.iter().any(|(id, _, _)| *id == *lang));
    assert!(
        gap_detected,
        "removing a mapping row must leave a detectable gap"
    );
}

/// Application languages carry at least one runtime command; data-format
/// entries carry none.
#[test]
fn application_languages_have_runtime_data_formats_do_not() {
    for (id, class, runtime) in LANGUAGE_RUNTIMES {
        match class {
            LanguageClass::Application => {
                let rt = runtime.expect("application language must have a runtime");
                assert!(!rt.commands.is_empty(), "{id:?} has empty runtime list");
                assert!(
                    rt.commands.iter().all(|c| !c.is_empty()),
                    "{id:?} has an empty runtime command name"
                );
            }
            LanguageClass::DataFormat => {
                assert!(runtime.is_none(), "{id:?} is data-format but has a runtime");
            }
        }
    }
}

/// Version-flag overrides must target known runtime commands.
#[test]
fn version_flag_overrides_target_known_commands() {
    let known: std::collections::HashSet<&str> = LANGUAGE_RUNTIMES
        .iter()
        .filter_map(|(_, _, rt)| rt.as_ref())
        .flat_map(|rt| rt.commands.iter().copied())
        .collect();
    for o in VERSION_FLAG_OVERRIDES {
        assert!(
            known.contains(o.command),
            "override for {:?} targets an unknown runtime command",
            o.command
        );
        assert!(
            !o.flag.is_empty(),
            "override for {:?} has an empty flag",
            o.command
        );
    }
}

/// `lookup` resolves application and data-format ids, and unknown ids
/// return `None`.
#[test]
fn lookup_classifies_known_and_unknown_ids() {
    let (class, runtime) = lookup("rust").expect("rust mapped");
    assert_eq!(class, LanguageClass::Application);
    let rt = runtime.expect("rust runtime present");
    assert!(rt.commands.contains(&"cargo"));
    assert!(rt.commands.contains(&"rustc"));
    assert!(is_application("rust"));
    assert!(is_application("python"));
    assert!(!is_application("toml"));
    assert!(!is_application("hcl"));
    assert!(lookup("notalanguage").is_none());
}

/// `version_flag_for` returns the override when present and the
/// `--version` default otherwise.
#[test]
fn version_flag_defaults_and_overrides() {
    assert_eq!(version_flag_for("java"), "-version");
    assert_eq!(version_flag_for("erl"), "-version");
    assert_eq!(version_flag_for("lua"), "-v");
    assert_eq!(version_flag_for("luajit"), "-v");
    assert_eq!(version_flag_for("go"), "version");
    assert_eq!(version_flag_for("zig"), "version");
    assert_eq!(version_flag_for("cargo"), "--version");
    assert_eq!(version_flag_for("python3"), "--version");
}

/// The dotnet multi-arg probe override targets `dotnet` with
/// `--list-sdks` + `--list-runtimes` (FR-008 notes).
#[test]
fn version_probe_args_override_covers_dotnet() {
    assert_eq!(
        VERSION_PROBE_ARGS_OVERRIDES,
        &[("dotnet", &["--list-sdks", "--list-runtimes"][..])]
    );
}

/// Multi-line probe output keeps every non-empty line (joined with
/// `, `), so all installed SDK/runtime versions are reported - not just
/// the first (e.g. `dotnet --list-sdks` lists 8.x and 9.x).
#[test]
fn probe_version_args_all_lines_keeps_every_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = write_script(
        &dir,
        "list-multi",
        "echo '8.0.130 [/opt/sdk]'\necho ''\necho '9.0.120 [/opt/sdk]'\necho 'noise' >&2",
    );
    assert_eq!(
        probe_version_args_all_lines(script.to_string_lossy().as_ref(), &["--list-sdks"]),
        VersionResult::Text("8.0.130 [/opt/sdk], 9.0.120 [/opt/sdk]".to_string()),
        "every non-empty stdout line must be kept"
    );
}

/// Write an executable shell script into a temp directory and return its
/// path (test helper for probe-version fixtures).
fn write_script(dir: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod script");
    }
    path
}

/// A version probe captures the first non-empty stdout line (FR-008).
#[test]
fn probe_version_captures_first_stdout_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = write_script(
        &dir,
        "versioned",
        "echo ''\necho '   '\necho 'tool 9.9 output'\necho 'noise' >&2",
    );
    assert_eq!(
        probe_version_args(script.to_string_lossy().as_ref(), &["--version"]),
        VersionResult::Text("tool 9.9 output".to_string()),
        "first non-empty line wins, blank lines skipped"
    );
}

/// A probe falls back to stderr when stdout carries no usable text
/// (e.g. `erl -version` prints to stderr).
#[test]
fn probe_version_falls_back_to_stderr() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = write_script(&dir, "versioned-err", "echo 'Erlang style version' >&2");
    assert_eq!(
        probe_version_args(script.to_string_lossy().as_ref(), &["-version"]),
        VersionResult::Text("Erlang style version".to_string()),
        "stderr must be consulted when stdout is empty"
    );
}

/// Spawn failure and a non-zero exit without usable text yield
/// `Unknown`, never a panic (FR-012).
#[test]
fn probe_version_reports_unknown_on_failure() {
    assert_eq!(
        probe_version_args("/nonexistent-dir-xyz/no-such-tool", &["--version"]),
        VersionResult::Unknown,
        "spawn failure must map to Unknown"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let script = write_script(&dir, "silent-failure", "exit 1");
    assert_eq!(
        probe_version_args(script.to_string_lossy().as_ref(), &["--version"]),
        VersionResult::Unknown,
        "non-zero exit without text must map to Unknown"
    );
}

/// A probe exceeding the per-probe timeout is killed and reported as
/// `Timeout` (FR-012), within a bounded wall-clock window.
#[test]
fn probe_version_times_out_and_kills_child() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = write_script(&dir, "sleeper", "sleep 30");
    let started = std::time::Instant::now();
    let result = probe_version_args(script.to_string_lossy().as_ref(), &["--version"]);
    let elapsed = started.elapsed();
    assert_eq!(result, VersionResult::Timeout, "sleeper must time out");
    assert!(
        elapsed >= VERSION_PROBE_TIMEOUT,
        "timeout must not fire early (elapsed {elapsed:?})"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "probe must be killed promptly, not run to child completion (elapsed {elapsed:?})"
    );
}

/// `placeholder_or_text` maps outcomes to FR-012 report placeholders.
#[test]
fn version_result_placeholder_mapping() {
    assert_eq!(
        VersionResult::Text("1.2.3".to_string()).placeholder_or_text(),
        Some("1.2.3".to_string())
    );
    assert_eq!(
        VersionResult::Unknown.placeholder_or_text(),
        Some("unknown".to_string())
    );
    assert_eq!(
        VersionResult::Timeout.placeholder_or_text(),
        Some("timeout".to_string())
    );
}

/// Renderer produces the FR-009 fenced ASCII-grid shape with the
/// FR-017 fixed-width columns, summary line, and `From:` prefix for a
/// hand-built probe set.
#[test]
fn render_markdown_report_table_summary_and_prefix() {
    let rows = vec![
        ReportRow {
            id: "rust",
            class: LanguageClass::Application,
            probes: vec![
                CommandProbe {
                    command: "cargo",
                    installed: true,
                    version: Some("cargo 1.85.0".to_string()),
                },
                CommandProbe {
                    command: "rustc",
                    installed: true,
                    version: Some("rustc 1.85.0".to_string()),
                },
            ],
        },
        ReportRow {
            id: "go",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "go",
                installed: true,
                version: Some("go version go1.22".to_string()),
            }],
        },
        ReportRow {
            id: "nim",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "nim",
                installed: false,
                version: None,
            }],
        },
        ReportRow {
            id: "toml",
            class: LanguageClass::DataFormat,
            probes: Vec::new(),
        },
    ];
    let report = render_markdown_report(&rows);
    assert!(report.starts_with("From: /toolchain list\n\n```\n"));
    // FR-017: fixed columns are 10/10/10/50; Language/Runtime cells are
    // clipped (the rust row's 12-char runtime clips to 10) and the Status
    // and Version columns word-wrap onto continuation grid lines (the
    // rust row's 34-char status wraps to 4 lines; the nim row's 13-char
    // status wraps to 2; the toml row's 27-char data-format marker wraps
    // to 4; every version cell here fits its 50-char column).
    let expected_row = |lang: &str, runtime: &str, status: &str, version: &str| {
        grid_line([(10, lang), (10, runtime), (10, status), (50, version)])
    };
    // First line of a multi-line row: clipped Language/Runtime plus the
    // first wrapped Status segment; the Version cell fits its column.
    assert!(
        report.contains(&expected_row("Language", "Runtime", "Status", "Version")),
        "fixed-width header row, got: {report}"
    );
    assert!(
        report.contains(&expected_row(
            "rust",
            "cargo, rus",
            "cargo:",
            "cargo: cargo 1.85.0; rustc: rustc 1.85.0"
        )),
        "rust first line, got: {report}"
    );
    assert!(report.contains(&expected_row("go", "go", "installed", "go version go1.22")));
    assert!(report.contains(&expected_row("nim", "nim", "not", "-")));
    assert!(report.contains(&expected_row("toml", "-", "(data", "-")));
    // Status continuation lines: blank Language/Runtime, wrapped segments.
    let cont = |status: &str| expected_row("", "", status, "");
    assert!(
        report.contains(&cont("installed;")),
        "rust cont 1: {report}"
    );
    assert!(report.contains(&cont("rustc:")), "rust cont 2: {report}");
    assert!(
        report.contains(&cont("installed")),
        "rust/nim last seg: {report}"
    );
    assert!(report.contains(&cont("format -")), "toml cont 1: {report}");
    assert!(report.contains(&cont("runtime")), "toml cont 2: {report}");
    assert!(report.contains(&cont("n/a)")), "toml cont 3: {report}");
    assert!(
        report.ends_with("\n2/3 application runtimes installed.\n```\n"),
        "summary line wrong: {report}"
    );
    // Pipe lines: header(1) + rust(4) + go(1) + nim(2) + toml(4).
    let row_lines = report.lines().filter(|l| l.starts_with('|')).count();
    assert_eq!(row_lines, 12, "wrapped row grid lines: {report}");
}

/// FR-017 word-wrap: a version cell wider than the 50-char column wraps
/// onto continuation grid lines (blank Language/Runtime/Status cells) and
/// the Language/Runtime columns keep clipping, so no version text is lost.
#[test]
fn render_markdown_report_version_cell_word_wraps() {
    let rows = vec![ReportRow {
        id: "python",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "python3",
            installed: true,
            version: Some(
                "Python 3.12.4 (main, Jun  6 2024, long tail text that exceeds fifty)".to_string(),
            ),
        }],
    }];
    let report = render_markdown_report(&rows);
    let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
    // Header + 1 data row wrapping to 2 lines = 3 pipe lines total
    // (borders are `+-` lines, not pipe lines).
    assert_eq!(
        grid_lines.len(),
        3,
        "wrapped row spans 2 grid lines: {report}"
    );
    // First line carries the row id and the first wrapped segment.
    assert!(
        grid_lines[1].contains("python     ") && grid_lines[1].contains("Python 3.12.4 (main,"),
        "first wrapped line: {}",
        grid_lines[1]
    );
    // Continuation line: blank Language/Runtime/Status, remainder text.
    assert!(
        grid_lines[2].contains("that exceeds fifty)"),
        "continuation line carries the wrap remainder: {}",
        grid_lines[2]
    );
    let blank_prefix = "|            |            |            |";
    assert!(
        grid_lines[2].starts_with(blank_prefix),
        "continuation line blanks the first three columns: {}",
        grid_lines[2]
    );
    // Borders = data rows + 2 (2 borders above/below header + one row
    // border after each row group).
    let borders = report
        .lines()
        .filter(|l| l.trim_start().starts_with("+-"))
        .count();
    assert_eq!(borders, 3, "one border per row group plus 2: {report}");
}

/// Long unbroken version tokens hard-split across wrap lines so every
/// line still fits the 50-char column.
#[test]
fn render_markdown_report_version_cell_hard_splits_long_tokens() {
    let long_token = "v".repeat(80);
    let rows = vec![ReportRow {
        id: "zig",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "zig",
            installed: true,
            version: Some(long_token.clone()),
        }],
    }];
    let report = render_markdown_report(&rows);
    let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
    assert_eq!(
        grid_lines.len(),
        3,
        "80-char token wraps to 2 lines: {report}"
    );
    assert!(
        grid_lines[1].contains(&long_token[..50]) && grid_lines[2].contains(&long_token[50..]),
        "token hard-splits at the column boundary: {report}"
    );
    // Every grid line stays 93 columns wide.
    for line in grid_lines {
        assert_eq!(line.chars().count(), 93, "grid line width: {line}");
    }
}

/// A report with zero application rows renders the `0/0` summary, and a
/// data-format-only table never shows presence/version columns.
#[test]
fn render_markdown_report_empty_and_data_only() {
    // Build expected grid lines from the fixed column widths.
    let row = |widths: [usize; 4], cells: [&str; 4]| {
        grid_line([
            (widths[0], cells[0]),
            (widths[1], cells[1]),
            (widths[2], cells[2]),
            (widths[3], cells[3]),
        ])
    };

    let report = render_markdown_report(&[]);
    assert!(report.starts_with("From: /toolchain list\n\n```\n"));
    // Empty rows: headers pad inside the fixed 10/10/10/50 columns.
    assert!(
        report.contains(&row(
            [10, 10, 10, 50],
            ["Language", "Runtime", "Status", "Version"]
        )),
        "fixed-width header row, got: {report}"
    );
    assert!(report.ends_with("\n0/0 application runtimes installed.\n```\n"));

    let data_only = render_markdown_report(&[ReportRow {
        id: "yaml",
        class: LanguageClass::DataFormat,
        probes: Vec::new(),
    }]);
    // The 27-char data-format marker word-wraps in the Status column.
    assert!(
        data_only.contains(&row([10, 10, 10, 50], ["yaml", "-", "(data", "-"])),
        "data-format row first line, got: {data_only}"
    );
    assert!(
        data_only.contains(&row([10, 10, 10, 50], ["", "", "format -", ""])),
        "data-format marker wrap line 1, got: {data_only}"
    );
    assert!(
        data_only.contains(&row([10, 10, 10, 50], ["", "", "runtime", ""])),
        "data-format marker wrap line 2, got: {data_only}"
    );
    assert!(
        data_only.contains(&row([10, 10, 10, 50], ["", "", "n/a)", ""])),
        "data-format marker wrap line 3, got: {data_only}"
    );
    assert!(data_only.ends_with("\n0/0 application runtimes installed.\n```\n"));
}

/// Version text containing pipes or line breaks must not break the
/// table grid (FR-009 single-line rendering via `cell`).
#[test]
fn render_markdown_report_sanitizes_version_text() {
    let rows = vec![ReportRow {
        id: "r",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "R",
            installed: true,
            version: Some("R version | 4.3\nwith newline\ttab".to_string()),
        }],
    }];
    let report = render_markdown_report(&rows);
    assert!(report.contains(r"R version \| 4.3 with newline tab"));
    assert!(report.ends_with("\n1/1 application runtimes installed.\n```\n"));
}

/// Row order is preserved in the rendered table (NFR-002 determinism).
#[test]
fn render_markdown_report_preserves_row_order() {
    let rows = vec![
        ReportRow {
            id: "python",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "python3",
                installed: true,
                version: Some("Python 3.12".to_string()),
            }],
        },
        ReportRow {
            id: "json",
            class: LanguageClass::DataFormat,
            probes: Vec::new(),
        },
        ReportRow {
            id: "zig",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "zig",
                installed: false,
                version: None,
            }],
        },
    ];
    let report = render_markdown_report(&rows);
    let python_pos = report.find("| python ").expect("python row");
    let json_pos = report.find("| json ").expect("json row");
    let zig_pos = report.find("| zig ").expect("zig row");
    assert!(python_pos < json_pos && json_pos < zig_pos, "order kept");
}

/// FR-017: the Language, Runtime, Status, and Version columns are fixed
/// at 10/10/10/50 characters; Language/Runtime over-wide content clips to
/// the column width, Status and Version word-wrap (FR-017).
#[test]
fn render_markdown_report_uses_fixed_column_widths() {
    let rows = vec![ReportRow {
        id: "rust",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "cargo",
            installed: true,
            version: Some("cargo 1.85.0".to_string()),
        }],
    }];
    let report = render_markdown_report(&rows);
    let widths: Vec<usize> = report
        .lines()
        .find(|l| l.starts_with("+--"))
        .expect("border row")
        .split('+')
        .filter(|s| !s.is_empty())
        .map(|seg| seg.len() - 2)
        .collect();
    assert_eq!(widths, vec![10, 10, 10, 50], "FR-017 widths: {report}");
    // A deliberately over-wide id clips to the 10-char Language column.
    let over = render_markdown_report(&[ReportRow {
        id: "verylonglanguageid",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "cargo",
            installed: true,
            version: Some("cargo 1.85.0".to_string()),
        }],
    }]);
    assert!(over.contains("| verylongla |"), "clip to 10: {over}");
}

/// FR-017: a status cell wider than the 10-char column word-wraps onto
/// continuation grid lines (blank Language/Runtime cells) so no status
/// text is clipped away.
#[test]
fn render_markdown_report_status_cell_word_wraps() {
    let rows = vec![ReportRow {
        id: "nim",
        class: LanguageClass::Application,
        probes: vec![CommandProbe {
            command: "nim",
            installed: false,
            version: None,
        }],
    }];
    let report = render_markdown_report(&rows);
    let grid_lines: Vec<&str> = report.lines().filter(|l| l.starts_with('|')).collect();
    // Header + 1 data row wrapping to 2 lines = 3 pipe lines total
    // (borders are `+-` lines, not pipe lines).
    assert_eq!(
        grid_lines.len(),
        3,
        "wrapped row spans 2 grid lines: {report}"
    );
    // First line carries the row id/runtime and the first wrap segment.
    assert!(
        grid_lines[1].contains("| nim        | nim        | not        |"),
        "first wrapped line: {}",
        grid_lines[1]
    );
    // Continuation line: blank Language/Runtime, remainder in Status.
    assert!(
        grid_lines[2].contains("|            |            | installed  |"),
        "continuation line carries the wrap remainder: {}",
        grid_lines[2]
    );
    // Borders = data rows + 2 (2 borders above/below header + one row
    // border after each row group).
    let borders = report
        .lines()
        .filter(|l| l.trim_start().starts_with("+-"))
        .count();
    assert_eq!(borders, 3, "one border per row group plus 2: {report}");
}

/// JSON report contains the FR-015 schema: `languages` array with
/// `id`/`runtime`/`status`/`version` objects plus `installed`/`total`
/// summary fields, and parses as valid JSON (TC-009).
#[test]
fn render_json_report_schema_and_validity() {
    let rows = vec![
        ReportRow {
            id: "rust",
            class: LanguageClass::Application,
            probes: vec![
                CommandProbe {
                    command: "cargo",
                    installed: true,
                    version: Some("cargo 1.85.0".to_string()),
                },
                CommandProbe {
                    command: "rustc",
                    installed: true,
                    version: Some("rustc 1.85.0".to_string()),
                },
            ],
        },
        ReportRow {
            id: "yaml",
            class: LanguageClass::DataFormat,
            probes: Vec::new(),
        },
        ReportRow {
            id: "zig",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "zig",
                installed: false,
                version: None,
            }],
        },
    ];
    let report = render_json_report(&rows);
    let parsed: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
    let langs = parsed["languages"].as_array().expect("languages array");
    assert_eq!(langs.len(), 3, "one object per report row");
    let rust = &langs[0];
    assert_eq!(rust["id"], "rust");
    assert_eq!(rust["runtime"], "cargo, rustc");
    assert_eq!(rust["status"], "cargo: installed; rustc: installed");
    assert_eq!(rust["version"], "cargo: cargo 1.85.0; rustc: rustc 1.85.0");
    assert_eq!(langs[1]["status"], DATA_FORMAT_MARKER);
    assert_eq!(langs[2]["status"], "not installed");
    assert_eq!(langs[2]["version"], "-");
    assert_eq!(parsed["installed"], 1);
    assert_eq!(parsed["total"], 2, "application rows only");
    assert!(report.starts_with('{'), "bare document, no From: prefix");
}

/// JSON summary matches the markdown summary on identical rows, and the
/// empty report renders a bare `{ languages: [], installed: 0, total: 0
/// }` document (FR-009 / FR-015 parity).
#[test]
fn render_json_report_summary_parity_with_markdown() {
    let rows = vec![
        ReportRow {
            id: "python",
            class: LanguageClass::Application,
            probes: vec![CommandProbe {
                command: "python3",
                installed: true,
                version: Some("Python 3.12".to_string()),
            }],
        },
        ReportRow {
            id: "json",
            class: LanguageClass::DataFormat,
            probes: Vec::new(),
        },
    ];
    let json = render_json_report(&rows);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["installed"], 1);
    assert_eq!(parsed["total"], 1);
    let markdown = render_markdown_report(&rows);
    assert!(
        markdown.contains("1/1 application runtimes installed."),
        "summary parity with JSON totals"
    );

    let empty: Vec<ReportRow> = Vec::new();
    let empty_json = render_json_report(&empty);
    let parsed: serde_json::Value = serde_json::from_str(&empty_json).expect("valid JSON");
    assert_eq!(parsed["languages"].as_array().map(Vec::len), Some(0));
    assert_eq!(parsed["installed"], 0);
    assert_eq!(parsed["total"], 0);
}

/// `resolve_in_dirs` finds an executable file in a supplied directory
/// and skips non-executable files (FR-007, command -v semantics).
#[test]
fn resolve_in_dirs_finds_executable_skips_plain_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exe = dir.path().join("mytool");
    std::fs::write(&exe, b"#!/bin/sh\n").expect("write exe");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).expect("chmod exe");
    }
    let plain = dir.path().join("other");
    std::fs::write(&plain, b"data").expect("write plain");

    let dirs = vec![dir.path().to_path_buf()];
    assert_eq!(
        resolve_in_dirs("mytool", &dirs),
        Some(exe),
        "executable file must resolve"
    );
    assert_eq!(
        resolve_in_dirs("other", &dirs),
        None,
        "non-executable must not resolve"
    );
    assert_eq!(resolve_in_dirs("missing", &dirs), None);
}

/// `resolve_in_dirs` searches directories in order and stops at the
/// first match.
#[test]
fn resolve_in_dirs_searches_dirs_in_order() {
    let first = tempfile::tempdir().expect("tempdir first");
    let second = tempfile::tempdir().expect("tempdir second");
    let exe = first.path().join("firsttool");
    std::fs::write(&exe, b"#!/bin/sh\n").expect("write");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let dirs = vec![first.path().to_path_buf(), second.path().to_path_buf()];
    assert_eq!(resolve_in_dirs("firsttool", &dirs), Some(exe.clone()));

    // Empty/missing directory entries are skipped without error.
    let with_bad = vec![
        std::path::PathBuf::from("/nonexistent-dir-xyz"),
        first.path().to_path_buf(),
    ];
    assert_eq!(resolve_in_dirs("firsttool", &with_bad), Some(exe));
}

/// `resolve_in_dirs` rejects empty names and path-carrying commands -
/// only bare command names are probed (FR-007).
#[test]
fn resolve_in_dirs_rejects_pathed_or_empty_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dirs = vec![dir.path().to_path_buf()];
    assert_eq!(resolve_in_dirs("", &dirs), None);
    assert_eq!(resolve_in_dirs("/bin/sh", &dirs), None);
    assert_eq!(resolve_in_dirs("./sh", &dirs), None);
    assert_eq!(resolve_in_dirs("../sh", &dirs), None);
}

/// `probe_installed` resolves a guaranteed-present system binary and
/// reports `None` for a name that does not exist anywhere on `PATH`.
#[test]
fn probe_installed_resolves_real_path() {
    // /bin/ls exists on every supported Linux/macOS dev environment.
    let found = probe_installed("ls").or_else(|| probe_installed("sh"));
    assert!(found.is_some(), "a system binary must resolve on PATH");

    assert_eq!(
        probe_installed("definitely-not-a-real-tool-xyz"),
        None,
        "absent command must not resolve"
    );
}

/// Every application-language runtime command must be a bare command
/// name (no path separators) so FR-007 probing applies uniformly.
#[test]
fn runtime_commands_are_bare_names() {
    for (id, _, rt) in LANGUAGE_RUNTIMES {
        let Some(rt) = rt else { continue };
        for cmd in rt.commands {
            assert!(!cmd.is_empty(), "{id:?} has an empty command");
            assert!(!cmd.contains('/'), "{id:?} command {cmd:?} carries a path");
        }
    }
}

/// The T-011 walk emits exactly one row per `SUPPORTED_LANGUAGES` entry,
/// in list order, matching the mapping-table classification (FR-004).
#[test]
fn build_report_rows_covers_scanner_list_in_order() {
    let rows = build_report_rows();
    assert_eq!(
        rows.len(),
        SUPPORTED_LANGUAGES.len(),
        "one row per scanner id"
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.id, SUPPORTED_LANGUAGES[index], "list order preserved");
        let (class, _) = lookup(row.id).expect("exhaustive mapping");
        assert_eq!(row.class, class, "classification from the mapping table");
    }
}

/// Application rows probe every mapped runtime command in FR-006 order;
/// data-format rows carry no probes (FR-005 marker path).
#[test]
fn build_report_rows_probes_match_runtime_mapping() {
    let rows = build_report_rows();
    for row in &rows {
        let (_, runtime) = lookup(row.id).expect("exhaustive mapping");
        match runtime {
            None => {
                assert!(
                    row.probes.is_empty(),
                    "data-format row {id:?} must carry no probes",
                    id = row.id
                );
            }
            Some(rt) => {
                let got: Vec<&str> = row.probes.iter().map(|p| p.command).collect();
                assert_eq!(got, rt.commands, "probe order for {id:?}", id = row.id);
                for probe in &row.probes {
                    if probe.installed {
                        // Found on PATH: version carries text or a
                        // FR-012 placeholder, never empty.
                        assert!(
                            probe.version.is_some(),
                            "{cmd:?} installed but no version text",
                            cmd = probe.command
                        );
                    } else {
                        assert!(
                            probe.version.is_none(),
                            "{cmd:?} absent but carries version text",
                            cmd = probe.command
                        );
                    }
                }
            }
        }
    }
}

/// The rust row is a two-command multi-runtime row (FR-006): `cargo`
/// and `rustc` in order, each reporting presence honestly - `cargo` is
/// installed in every environment that builds this workspace, so its
/// version probe must capture real text (spot-check against TC-001).
#[test]
fn build_report_rows_rust_row_probes_cargo_and_rustc() {
    let rows = build_report_rows();
    let rust = rows
        .iter()
        .find(|row| row.id == "rust")
        .expect("rust is in SUPPORTED_LANGUAGES");
    assert_eq!(rust.class, LanguageClass::Application);
    let commands: Vec<&str> = rust.probes.iter().map(|p| p.command).collect();
    assert_eq!(commands, ["cargo", "rustc"], "FR-006 order");
    for probe in &rust.probes {
        assert!(
            probe.installed,
            "{cmd:?} must resolve in this workspace",
            cmd = probe.command
        );
        let version = probe
            .version
            .as_deref()
            .expect("installed probe has version");
        assert!(
            !version.trim().is_empty(),
            "version text must be non-empty, got {version:?}"
        );
    }
}

/// Absent runtimes report `not installed` with no version and the walk
/// continues past them (FR-010): a command name that cannot exist keeps
/// the row shape intact.
#[test]
fn probe_command_absent_reports_not_installed_shape() {
    // Not reachable through `build_report_rows` (mapping commands are
    // real), so drive `probe_command`'s building blocks directly.
    assert_eq!(probe_installed("definitely-not-a-real-tool-xyz"), None);
    let rows: Vec<ReportRow> = supported_language_rows()
        .map(|(id, resolved)| {
            let (class, runtime) = resolved.expect("exhaustive mapping");
            let probes = runtime
                .map(|rt| {
                    rt.commands
                        .iter()
                        .copied()
                        .map(|command| CommandProbe {
                            command,
                            installed: false,
                            version: None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            ReportRow { id, class, probes }
        })
        .collect();
    assert_eq!(rows.len(), SUPPORTED_LANGUAGES.len());
    let rendered = render_markdown_report(&rows);
    assert!(rendered.contains("0/30 application runtimes installed."));
}
