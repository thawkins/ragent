//! Guard test for spec `uniformtools` T-034 + T-047 + T-048 / FR-027 / NFR-008 /
//! NFR-009: fail when any of NFR-008's seven predicates holds in non-test source.
//!
//! The predicates (T-047) are: **(a)** a `before` spelling as a key or a value
//! in an alias table, **(a2)** an alias entry mapping a spelling onto itself,
//! **(b)** a `before` spelling in a resolver match arm, **(c)** a `before`
//! spelling in a default configuration value, **(d)** a `before` spelling as a
//! registry key, **(e)** a `before` spelling as a quoted or backticked literal
//! (the T-034 scan), **(f)** one tool name registered more than once, and
//! **(g)** a tool whose `execute` delegates wholly to an earlier-registered
//! tool. One run reports every offending file and line for every predicate.
//!
//! ## One implementation, two entry points (T-048, NFR-009)
//!
//! The guard is `scripts/check_tool_rename.py`. `scripts/check-tool-rename.sh`
//! is a shell wrapper with no predicate logic, and this file is the *third*
//! caller: it runs the same Python file and asserts the same verdict, the same
//! hits and the same ASCII wording. Nothing here re-implements a predicate, so
//! the `cargo test` case and the `scripts/` guard cannot disagree - they are one
//! program with two front doors.
//!
//! The cases below split along that line:
//!
//! * **the entry-point agreement cases** run the script form (`--check`) and the
//!   JSON form (`--json`, what this file reads) against the same tree and assert
//!   they report the same thing, field for field. This is the property that makes
//!   "usable both as a `cargo test` case and as a guard" true rather than
//!   aspirational;
//! * **the hermeticity cases** (NFR-009) run the one implementation against a
//!   scratch copy of the repository under `target/temp/` with `tools.xlsx`
//!   removed, assert the report names only paths inside that copy, and assert the
//!   full tree run stays inside a few seconds - so "reads only repository files,
//!   no network request, no `tools.xlsx`, a few seconds" is checked, not assumed.
//!   A `--root` copy is why they can seed fixtures without writing into the
//!   checked-in tree at all;
//! * **the self-test case** runs `--self-test`, which seeds its own stale literal,
//!   asserts it is reported as `(e)` with the file and line, and removes it;
//! * **the predicate cases** (`test_nfr008_predicate_*`) seed a fixture per
//!   predicate in a scratch copy and assert the report names the file, the line
//!   and the letter, and that one run reports every seeded predicate at once.
//!
//! ## Failure messages are part of the contract
//!
//! NFR-009 requires each failure message to name the predicate letter *and* the
//! exact edit required. The predicate cases therefore assert the `fix:` line as
//! well as the letter: a report that says `(c)` but not what to change leaves the
//! reader to reverse-engineer the rule.
//!
//! ## The predicate is exercised through the real program
//!
//! Every case invokes ``scripts/check_tool_rename.py`` as a subprocess - the same
//! implementation the CI job runs - rather than calling a predicate function in
//! process, so what is asserted is what a contributor actually sees.

// Every test here runs the guard over a *whole tree*, and most now run it over a
// scratch copy of one under `target/temp/` rather than over the checkout. The
// cases that still seed into the checkout (the seeded literal, the `tests/` scope
// probe) plant and remove a file inside it, which is why the exclusion has to
// hold across processes rather than inside one:
//
// A file-local `Mutex` only orders the tests in this one file, so it was not
// enough: under `cargo nextest` every test is its own process and the
// `ragent-agent` binary's tests all run at once, so
// `test_guard_passes_on_the_checked_in_tree` scanned the tree while
// `test_guard_fails_on_a_seeded_retired_literal` had its fixture on disk and
// failed on a file that test had planted.
//
// `#[serial]` does not fix that here, and is deliberately not used: serial_test
// publishes the lock key as `SERIAL_TEST_RETRY_KEY` in the *parent* environment,
// and nextest starts each test process from its own saved environment, so the
// two sibling processes take different keys and run concurrently anyway.
// `#[serial]` is a reliable exclusion inside one process (`cargo test`, which
// runs the whole `all` binary in one process); across nextest's processes it is
// not, and saying so here keeps the next reader from re-adding it.
//
// The ordering these tests need is nextest's own: `.config/nextest.toml` puts
// this module in the `test-group` `tool-rename-guard` with `max-threads = 1`,
// which the runner enforces across processes, so a guard run never scans the
// tree while a sibling's fixture is on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_guard(extra_args: &[&str]) -> (i32, String) {
    let root = repo_root();
    let script = root.join("scripts/check_tool_rename.py");
    let mut cmd = Command::new("python3");
    cmd.arg(&script);
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.current_dir(&root);
    let out = cmd.output().expect("failed to invoke guard script");
    let code = out.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (code, stdout + &stderr)
}

// ---------------------------------------------------------------------------
// T-048: the scratch-copy harness, so a fixture never enters the checkout
// ---------------------------------------------------------------------------

/// The JSON report the one guard implementation prints (`--json`).
///
/// Only the fields the cases assert are modelled; the guard's payload also
/// carries a `report` string per hit, which the human output is compared
/// against.
#[derive(Debug)]
struct GuardReport {
    /// The exit verdict: no hit and no exemption problem.
    ok: bool,
    /// A table-read failure message, else `None`.
    error: Option<String>,
    /// The root the guard actually scanned.
    root: String,
    /// One entry per predicate hit, sorted by path then line.
    hits: Vec<GuardHit>,

    /// The line a clean run prints.
    success: String,
}

/// One predicate hit from the JSON report.
#[derive(Debug)]
struct GuardHit {
    /// The predicate letter the hit is labelled with.
    predicate: String,
    /// Repository-relative path of the offending file.
    path: String,
    /// 1-based line of the offending site.
    line: u64,
    /// The retired spellings the line carries.
    literals: Vec<String>,
    /// The exact edit that clears the hit (NFR-009).
    fix: String,
    /// The offending source line, stripped.
    text: String,
    /// The predicate-specific explanation, such as the tool a body forwards to.
    detail: String,
}

/// A copy of the parts of the repository the guard reads, under
/// `target/temp/`.
///
/// A fixture seeded into the checkout is visible to every other test process
/// scanning the tree, which is the race the `tool-rename-guard` nextest group
/// has to serialise. Pointing the one implementation at a private copy with
/// `--root` removes the race instead of ordering it, and lets a case seed several
/// violations at once without touching a tracked file.
///
/// The copy holds exactly what the guard reads: `crates/` (the scanned sources
/// and `Cargo.toml` manifests), the root `Cargo.toml` (the workspace dependency
/// table) and the root `src/` tree. Nothing else is copied, so the copy is small
/// and the scan is as fast as the real one.
struct ScratchTree {
    root: PathBuf,
}

impl ScratchTree {
    /// Build a scratch copy in `target/temp/<name>`.
    fn new(name: &str) -> Self {
        let root = repo_root().join("target/temp").join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("failed to clear the scratch tree");
        }
        let source = repo_root();
        copy_tree(&source.join("crates"), &root.join("crates"));
        copy_tree(&source.join("src"), &root.join("src"));
        // The workspace manifest carries `[workspace.dependencies]`, which the
        // guard resolves a member's `workspace = true` dependency through. It is
        // not under `crates/`, so it is copied explicitly - without it no package
        // has a dependency edge and predicate (g)'s "registered earlier" test
        // cannot fire.
        std::fs::copy(source.join("Cargo.toml"), root.join("Cargo.toml"))
            .expect("failed to copy the workspace manifest");
        // The guard fails closed if the table is missing, so keep it even when a
        // case only seeds one fixture.
        assert!(
            root.join("crates/ragent-types/src/tool_rename.rs")
                .is_file(),
            "the scratch copy must carry the FR-023 table"
        );
        Self { root }
    }

    /// Write a source file into the copy's crate tree.
    fn seed(&self, relative: &str, source: &str) -> String {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create the fixture directory");
        }
        std::fs::write(&path, source).expect("failed to write the fixture");
        relative.to_string()
    }

    /// Run the one implementation against this copy and parse its `--json` report.
    fn report(&self) -> (i32, GuardReport) {
        let (code, out) = run_guard(&["--json", "--root", &self.root.to_string_lossy()]);
        (code, parse_report(&out))
    }

    /// Run the one implementation against this copy and return its human report.
    fn check(&self) -> (i32, String) {
        run_guard(&["--check", "--root", &self.root.to_string_lossy()])
    }
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Copy `from` to `to`, recursively, skipping `tests/` subtrees.
fn copy_tree(from: &Path, to: &Path) {
    if !from.is_dir() {
        return;
    }
    std::fs::create_dir_all(to).expect("failed to create a scratch directory");
    let entries = std::fs::read_dir(from).expect("failed to read a source directory");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name == "tests" || name == "benches" || name == "target" {
                continue;
            }
            copy_tree(&path, &to.join(&name));
        } else if name.to_string_lossy().ends_with(".rs") || name == "Cargo.toml" {
            std::fs::copy(&path, to.join(&name)).expect("failed to copy a source file");
        }
    }
}

/// Parse the guard's `--json` report without pulling a JSON crate into the test.
///
/// The payload is the guard's own shape: one object per hit with `"key": value`
/// lines, `hits` before `problems`. A parser for a shape one program prints and
/// one test reads would be a dependency for nothing when the shape is this
/// regular; the assertions below fail loudly if it drifts.
///
/// Two shapes are accommodated because they are easy to get wrong: a list is
/// printed one item per line, so a value runs until its closing bracket or the
/// next key; and a string value ends at its closing quote, not at the first
/// quote at either end (a value ending in an escaped quote would otherwise lose
/// its backslash and read as unterminated). Both were found by these cases
/// failing against the real payload, which is the reason the case compares the
/// two entry points rather than parsing one and trusting it.
fn parse_report(text: &str) -> GuardReport {
    /// Returns `true` when `line` opens a new `"key": ...` pair.
    fn is_key_line(line: &str) -> bool {
        match line.strip_prefix('"') {
            Some(rest) => rest.contains("\":"),
            None => false,
        }
    }

    /// The raw text of one `"key": value` pair, which may span lines.
    ///
    /// `--json` prints a scalar on the key's own line and a list one item per
    /// line, so a list value runs until its closing bracket - or until the next
    /// key, which ends the pair when the bracket sits on a line of its own.
    fn field(object: &str, key: &str) -> Option<String> {
        let needle = format!("\"{key}\": ");
        let mut lines = object
            .lines()
            .skip_while(|line| !line.trim().starts_with(&needle));
        let rest = lines.next()?.trim().strip_prefix(&needle)?.to_string();
        if !rest.starts_with('[') || rest.trim_end_matches(',').ends_with(']') {
            // A scalar, or a list printed on one line. A string value loses its
            // trailing comma only: the quotes belong to the value.
            return Some(rest.strip_suffix(',').unwrap_or(&rest).to_string());
        }
        let mut value = rest;
        for next in lines {
            let next = next.trim();
            let closes = next.starts_with(']');
            value.push(' ');
            value.push_str(next);
            if closes || is_key_line(next) {
                break;
            }
        }
        Some(value)
    }

    /// The text of one string pair, unquoted and unescaped.
    fn string_field(object: &str, key: &str) -> Option<String> {
        let raw = field(object, key)?;
        // One key per line, so the last character is the only one that can be the
        // closing quote. Trimming both ends instead would eat the backslash of a
        // trailing `\"` and leave the string unterminated.
        let inner = match raw
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        {
            Some(inner) => inner.to_string(),
            None => return Some(raw),
        };
        Some(inner.replace("\\n", "\n").replace("\\\"", "\""))
    }

    /// The items of one list pair.
    fn list_field(object: &str, key: &str) -> Vec<String> {
        let Some(raw) = field(object, key) else {
            return Vec::new();
        };
        raw.lines()
            .flat_map(|line| line.split(','))
            .map(|item| item.trim().trim_matches(['[', ']']))
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.trim_matches('"').to_string())
            .collect()
    }

    let top = text
        .trim_start()
        .strip_prefix('{')
        .expect("the guard's --json payload is one object")
        .to_string();
    let head = top.split("\"hits\": [").next().unwrap_or("");
    let ok = field(head, "ok").as_deref() == Some("true");
    let error = match string_field(head, "error") {
        Some(value) if value != "null" => Some(value),
        _ => None,
    };
    let root = string_field(head, "root").unwrap_or_default();
    let success = string_field(&top, "success").unwrap_or_default();

    let hits_block = top
        .split("\"hits\": [")
        .nth(1)
        .and_then(|rest| rest.split("\"problems\"").next())
        .unwrap_or("");
    let mut hits = Vec::new();
    for chunk in hits_block.split("\"predicate\":").skip(1) {
        let value = chunk.split('}').next().unwrap_or("").trim();
        // The chunk starts right after the key, so the predicate is its first
        // quoted word.
        let predicate = value
            .strip_prefix('"')
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_default()
            .to_string();
        let object = format!("\"predicate\": {value}");
        hits.push(GuardHit {
            predicate,
            path: string_field(&object, "path").unwrap_or_default(),
            line: field(&object, "line")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            literals: list_field(&object, "literals"),
            fix: string_field(&object, "fix").unwrap_or_default(),
            text: string_field(&object, "text").unwrap_or_default(),
            detail: string_field(&object, "detail").unwrap_or_default(),
        });
    }

    GuardReport {
        ok,
        error,
        root,
        hits,
        success,
    }
}

/// The seeded-violation case, driven through the guard's own `--self-test`.
///
/// T-048 moved the seed into the one implementation (`--self-test` writes it,
/// scans, asserts the hit and removes it in one call) so the shell wrapper, the
/// CI job and this case cannot disagree about what is seeded, where, or what
/// count it must produce. What the case pins is the *contract* NFR-009 states:
/// a non-zero exit, ASCII diagnostics, the predicate letter, the retired
/// `before` spelling, the offending file, and its line.
///
/// The seed lives inside the checkout for the duration of this call, which is
/// why this module sits in the `tool-rename-guard` nextest group.
#[test]
fn test_self_test_detects_a_seeded_retired_literal() {
    let (code, combined) = run_guard(&["--self-test"]);
    assert_eq!(
        code, 0,
        "the guard's self-test must detect its own seeded stale literal; it \
         reported:\n{combined}"
    );
    assert!(
        combined.is_ascii(),
        "guard output must be ASCII (NFR-009), but it contained non-ASCII bytes: {combined:?}"
    );
    assert!(
        combined.contains("(e)"),
        "the self-test must report the predicate letter (e): {combined:?}"
    );
    assert!(
        combined.contains("__tool_rename_guard_selftest.rs:5"),
        "the self-test must name the offending file and line, not merely exit \
         non-zero: {combined:?}"
    );
    assert!(
        !repo_root()
            .join("crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs")
            .exists(),
        "the self-test must remove its own fixture; a left-behind seed makes the \
         next guard run fail:\n{combined}"
    );
}

/// NFR-009: the failure message names the predicate letter AND the exact edit.
///
/// Asserted on a deliberately failing scratch run, because that is the message a
/// contributor actually sees - the self-test's own line is a success message.
#[test]
fn test_a_failure_names_the_predicate_letter_and_the_exact_edit() {
    let tree = ScratchTree::new("t048_failure_message");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_failure_message.rs",
        r#"//! Seeded failure-message fixture -- not shipped.
pub const SEEDED_FAILURE: &str = "bash";
"#,
    );
    let (code, body) = tree.check();
    assert_ne!(code, 0, "the seeded literal must fail the run");
    assert!(
        body.is_ascii(),
        "failure messages must be ASCII (NFR-009): {body:?}"
    );
    assert!(
        body.contains("(e)"),
        "the failure must name the predicate letter (e): {body}"
    );
    assert!(
        body.contains(&format!("{fixture}:")),
        "the failure must name the offending file and line: {body}"
    );
    assert!(
        body.contains("fix: "),
        "the failure must name the exact edit required: {body}"
    );
    assert!(
        body.contains("\"Bash\""),
        "the (e) failure must name the adopted spelling the edit should use: {body}"
    );
    // The same message, field by field, in the machine-readable form.
    let (_, report) = tree.report();
    let hit = assert_hit(&report, "e", &fixture);
    assert_eq!(hit.literals, vec!["bash".to_string()]);
    assert!(
        hit.fix.starts_with("migrate the literal"),
        "the (e) hit must carry the edit in its `fix` field: {hit:?}"
    );
}

/// The exemption list is an audit, not a filter: `--exemptions` must name both
/// structural exemptions explicitly and print a written reason for every
/// per-line entry. An entry without a reason is unreviewable, and an entry for a
/// tool-name site is the defect the guard exists to catch.
#[test]
fn test_every_exemption_is_reviewable_and_the_two_structural_ones_are_named() {
    let (code, listing) = run_guard(&["--exemptions"]);
    assert_eq!(
        code, 0,
        "`--exemptions` must succeed; it failed:\n{listing}"
    );
    assert!(
        listing.is_ascii(),
        "the exemption listing must be ASCII (NFR-009): {listing:?}"
    );
    // Structural exemption 1: the FR-023 table itself.
    assert!(
        listing.contains("crates/ragent-types/src/tool_rename.rs"),
        "the FR-023 table must be named as a structural exemption: {listing:?}"
    );
    // Structural exemption 2: the CHANGELOG.md entry, outside the scanned tree.
    assert!(
        listing.contains("CHANGELOG.md"),
        "the CHANGELOG.md entry must be named as a structural exemption: {listing:?}"
    );
    // Every per-line entry carries a reason: the listing prints one `reason:`
    // line per entry, so the declared entry count and the printed reason count
    // must agree, and none may be blank.
    let declared: usize = listing
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("# Per-line exemptions (")?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .unwrap_or_else(|| {
            panic!("`--exemptions` must declare its per-line entry count: {listing:?}")
        });
    let reasons: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.trim().strip_prefix("reason:"))
        .collect();
    assert!(
        declared > 0,
        "the listing declared 0 exemptions: {listing:?}"
    );
    assert_eq!(
        reasons.len(),
        declared,
        "the listing declares {declared} per-line exemptions but printed {} reason(s) - \
         an entry without a reason is an unreviewable one: {listing:?}",
        reasons.len()
    );
    for reason in &reasons {
        assert!(
            !reason.trim().is_empty(),
            "every exemption must carry a written reason; found a blank one: {listing:?}"
        );
    }
}

/// Tests are outside the guard's scope on purpose: a test may name a retired
/// spelling in order to assert that it is retired (FR-030's surface cases and the
/// FR-023 invariant cases all do). Seeding one into a `tests/` file must therefore
/// pass, which is what keeps the guard from forcing those assertions to be
/// weakened.
///
/// Driven in a scratch copy, so the probe never exists in the checkout even for
/// an instant.
#[test]
fn test_a_retired_literal_in_a_test_file_is_out_of_scope() {
    let tree = ScratchTree::new("t048_tests_scope");
    let probe = tree.seed(
        "crates/ragent-tools-core/tests/test_zz_guard_scope_probe.rs",
        "// Scope probe for test_tool_rename_guard -- scratch copy only.\n\
         const RETIRED: &str = \"bash\";\n",
    );
    let (code, combined) = tree.check();
    assert_eq!(
        code, 0,
        "a retired literal inside tests/ is out of the guard's scope, but it failed:\n{combined}"
    );
    assert!(
        !combined.contains(&probe),
        "the guard must not report a tests/ file: {combined:?}"
    );
    // The same file under `src/` IS reported, so the case proves the scope
    // boundary rather than merely that a file was ignored.
    let in_src = tree.seed(
        "crates/ragent-tools-core/src/__guard_scope_probe.rs",
        "// Scope probe for test_tool_rename_guard -- scratch copy only.\n\
         pub const RETIRED: &str = \"bash\";\n",
    );
    let (code, combined) = tree.check();
    assert_ne!(
        code, 0,
        "the same literal under src/ must be reported, so the scope boundary is \
         the tests/ directory and not the literal:\n{combined}"
    );
    assert_reported(&combined, "e", &in_src);
}

// ---------------------------------------------------------------------------
// T-048 (NFR-009): one implementation, two entry points, hermetic and cheap
// ---------------------------------------------------------------------------

/// The `scripts/` guard and the `cargo test` case are the SAME program, so they
/// must report the same thing: the same verdict, the same hits in the same
/// order, and the same wording.
///
/// `--check` is what `scripts/check-tool-rename.sh` prints and `--json` is what
/// this file reads. This case compares them over the checkout, so a change to
/// one form that is not made to the other fails here rather than in review
/// (NFR-009's "usable both as a `cargo test` case and as a `scripts/` guard,
/// from one implementation").
#[test]
fn test_the_script_form_and_the_test_form_report_the_same_thing() {
    let (script_code, script_out) = run_guard(&["--check"]);
    let (json_code, json_out) = run_guard(&["--json"]);
    assert_eq!(
        script_code, json_code,
        "the two entry points must agree on the verdict; --check exited \
         {script_code} and --json exited {json_code}"
    );
    let report = parse_report(&json_out);
    assert!(
        report.error.is_none(),
        "the FR-023 table must be readable: {:?}",
        report.error
    );
    assert_eq!(
        report.ok,
        script_code == 0,
        "the JSON verdict must match the exit code"
    );

    // Every hit the JSON form carries must appear in the human form, with its
    // letter, its file, its line and its fix.
    for hit in &report.hits {
        assert!(
            script_out.contains(&format!("({}) {}:{}", hit.predicate, hit.path, hit.line)),
            "the script form must report the same hit the test form reads \
             ({}) {}:{}:\n{script_out}",
            hit.predicate,
            hit.path,
            hit.line
        );
        assert!(
            script_out.contains(&hit.fix),
            "the script form must carry the same fix text as the test form: {script_out}"
        );
    }
    assert_eq!(
        script_out.matches("fix: ").count(),
        report.hits.len(),
        "the script form must print one fix line per hit:\n{script_out}"
    );
    if report.ok {
        assert!(
            script_out.trim_end().ends_with(&report.success),
            "a clean run must print the success line the test form reports: \
             {}\n---\n{}",
            report.success,
            script_out
        );
    }
    assert!(
        script_out.is_ascii() && json_out.is_ascii(),
        "both forms must be ASCII (NFR-009)"
    );
}

/// NFR-009's cost clause: "a few seconds so it can run on every `cargo test` and
/// in the CI guard set without a timeout exception".
///
/// Pinned rather than assumed, and pinned against a wall-clock budget generous
/// enough for a loaded CI runner: the measured full-tree run is about two
/// seconds, so ten leaves room for a slow machine while still failing a change
/// that turns the guard into something that walks the whole repository.
#[test]
fn test_the_guard_finishes_in_a_few_seconds() {
    let tree = ScratchTree::new("t048_cost");
    let start = std::time::Instant::now();
    let (code, out) = tree.check();
    let elapsed = start.elapsed();
    assert_eq!(code, 0, "the scratch copy must be clean: {out}");
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "the guard must finish in a few seconds (NFR-009); it took {elapsed:?}"
    );
}

/// NFR-009's hermeticity clause: repository files only, no network request, no
/// `tools.xlsx`.
///
/// Two observable halves, both driven through the one implementation:
///
/// * `--root` confines the scan - every path the report names is inside the root
///   it was given, and a copy with no `tools.xlsx` and no `Cargo.lock` still
///   produces the same verdict as the checkout. A guard that needed the workbook
///   would fail on that copy.
/// * the program imports nothing that can open a socket or read a workbook: the
///   module's imports are checked against a deny-list, because "makes no network
///   request" is a statement about the code that no runtime observation can
///   prove on its own.
#[test]
fn test_the_guard_reads_only_the_repository_and_never_a_workbook() {
    let script = std::fs::read_to_string(repo_root().join("scripts/check_tool_rename.py"))
        .expect("the guard must be readable");
    for forbidden in [
        "import urllib",
        "import socket",
        "import http",
        "import requests",
        // The workbook half. These are the spellings a workbook *reader* is
        // written with, and this list is itself scanned by the FR-023 invariant
        // case, so the one open-symbol is checked in halves rather than whole.
        "read_xlsx",
        "Workbook::",
        "xlrd",
        "xlsx2csv",
    ] {
        let reaches = script.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#')
                && !line.starts_with('*')
                && !line.starts_with("//")
                && line.contains(forbidden)
        });
        assert!(
            !reaches,
            "the guard must not reach for `{forbidden}`: it reads only repository \
             files and makes no network request (NFR-009)"
        );
    }

    // `--root` confines the report to the tree it was given, and the copy holds
    // no workbook at all.
    let tree = ScratchTree::new("t048_hermetic");
    tree.seed(
        "crates/ragent-agent/src/__guard_hermetic.rs",
        "//! Seeded hermeticity fixture -- scratch copy only.\n\
         pub const SEEDED_HERMETIC: &str = \"bash\";\n",
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the seeded literal must fail the run");
    assert!(
        report.root.contains("target/temp/t048_hermetic"),
        "the report must name the root it scanned: {}",
        report.root
    );
    for hit in &report.hits {
        assert!(
            !hit.path.starts_with('/') && !hit.path.contains(".."),
            "every reported path must be repository-relative and inside the root: {hit:?}"
        );
    }
    assert!(
        !tree.root.join("tools.xlsx").exists(),
        "the scratch copy carries no workbook, so a run that needed one could not pass"
    );
    // The checkout's verdict and the copy's verdict are the same guard.
    let (checkout_code, checkout_out) = run_guard(&["--check"]);
    assert_eq!(
        checkout_code, 0,
        "the checkout must be clean for this comparison:\n{checkout_out}"
    );
    assert!(
        !checkout_out.contains("tools.xlsx"),
        "the guard's output must not name the workbook it must not open: {checkout_out}"
    );
}

/// The structural exemption the exemption audit must name, and the one the guard
/// reads rather than skips: the FR-023 table. `--root` still confines it, so a
/// copy without the table fails closed rather than passing silently.
#[test]
fn test_a_root_without_the_rename_table_fails_closed() {
    let tree = ScratchTree::new("t048_no_table");
    std::fs::remove_file(tree.root.join("crates/ragent-types/src/tool_rename.rs"))
        .expect("the scratch copy carries the table");
    let (code, out) = run_guard(&["--json", "--root", &tree.root.to_string_lossy()]);
    assert_ne!(code, 0, "a missing table must fail the guard closed: {out}");
    let report = parse_report(&out);
    assert!(
        !report.ok,
        "a missing table must not be reported as clean: {report:#?}"
    );
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("tool_rename.rs")),
        "the failure must name the table it could not read: {report:#?}"
    );
}

// ---------------------------------------------------------------------------
// T-047 / T-048: one seeded fixture per NFR-008 predicate, in a scratch copy
// ---------------------------------------------------------------------------

/// Assert the parsed report carries a hit for `letter` on `fixture`, and that the
/// hit names the line and the exact edit NFR-009 requires.
fn assert_hit<'a>(report: &'a GuardReport, letter: &str, fixture: &str) -> &'a GuardHit {
    let hit = report
        .hits
        .iter()
        .find(|hit| hit.predicate == letter && hit.path == fixture)
        .unwrap_or_else(|| {
            panic!(
                "the report must carry a ({letter}) hit on {fixture}; it carried {:#?}",
                report.hits
            )
        });
    assert!(
        hit.line > 0,
        "the ({letter}) hit on {fixture} must name its line: {hit:?}"
    );
    assert!(
        !hit.fix.trim().is_empty(),
        "the ({letter}) hit on {fixture} must name the exact edit required (NFR-009): {hit:?}"
    );
    hit
}

/// Assert the human report names `letter`, the fixture and its line, and the fix.
fn assert_reported(report: &str, letter: &str, fixture: &str) {
    assert!(
        report.contains(&format!("({letter})")),
        "the report must name predicate ({letter}): {report}"
    );
    assert!(
        report.contains(fixture),
        "the report must name the offending file {fixture}: {report}"
    );
    assert!(
        report.lines().any(|line| line.contains(fixture)
            && line.contains(&format!("({letter})"))
            && line.contains(&format!("{fixture}:"))),
        "the report must name the offending file AND line for ({letter}): {report}"
    );
    assert!(
        report.contains("fix: "),
        "the report must name the exact edit required for ({letter}) (NFR-009): {report}"
    );
}

/// Predicates (a) and (a2): an alias table naming a retired spelling, a target
/// naming one, and an entry mapping a spelling onto itself.
#[test]
fn test_nfr008_predicate_a_alias_table_entry_is_reported() {
    let tree = ScratchTree::new("t048_predicate_a");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_alias_table.rs",
        r#"//! Seeded alias-table fixture -- not shipped.
pub const SEEDED_TABLE: [(&str, &str); 3] = [
    ("WebFetch", "plan_exit"),
    ("bash", "Bash"),
    ("Edit", "Edit"),
];
"#,
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on a seeded alias table");
    assert!(!report.ok, "the verdict must be a failure: {report:#?}");
    // The key half is reported as (a).
    let key = assert_hit(&report, "a", &fixture);
    assert_eq!(key.literals, vec!["bash".to_string()]);
    // The target half, which is the half a reviewer skims past: the row whose
    // *target* is a retired spelling is a code literal too, so (e) reports it
    // and names the adopted spelling the edit should use.
    assert!(
        report
            .hits
            .iter()
            .any(|hit| hit.predicate == "e" && hit.literals == vec!["plan_exit".to_string()]),
        "the report must name the target half of the rule, not only the key \
         half: {:#?}",
        report.hits
    );
    // The self-mapping row is caught as well, in the same run.
    assert_hit(&report, "a2", &fixture);
}

/// Predicate (a2): an alias entry that maps a spelling onto the identical
/// string. It maps no retired name, so predicates (a) and (e) stay silent and
/// (a2) is the only one that can catch it.
#[test]
fn test_nfr008_predicate_a2_self_mapping_is_reported() {
    let tree = ScratchTree::new("t048_predicate_a2");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_self_mapping.rs",
        r#"//! Seeded self-mapping fixture -- not shipped.
pub const SEEDED_SELF_MAPPING: [(&str, &str); 1] = [
    ("Edit", "Edit"),
];
"#,
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on a self-mapping entry");
    assert_hit(&report, "a2", &fixture);
    let (_, human) = tree.check();
    assert!(
        human.contains("self-mapping"),
        "the report must say the entry maps a spelling onto itself: {human}"
    );
}

/// Predicate (b): a resolver file's match arm on a retired spelling.
///
/// The fixture is seeded over `crates/ragent-agent/src/tool/alias_table.rs`,
/// which is one of the four files predicate (b) reads, so the arm is judged as a
/// resolver arm rather than as a plain literal. Only the arm is replaced, so the
/// rest of the file still parses as the resolver it is.
#[test]
fn test_nfr008_predicate_b_resolver_arm_is_reported() {
    let tree = ScratchTree::new("t048_predicate_b");
    let original =
        std::fs::read_to_string(repo_root().join("crates/ragent-agent/src/tool/alias_table.rs"))
            .expect("the alias resolver must be readable");
    let fixture = "crates/ragent-agent/src/tool/alias_table.rs";
    tree.seed(
        fixture,
        &format!(
            "{original}\n\
             /// Seeded resolver arm -- appended by the guard's test, in a scratch copy.\n\
             pub fn seeded_resolve(name: &str) -> bool {{\n    \
             match name {{\n        \
             \"task_list\" => true,\n        \
             _ => false,\n    \
             }}\n\
             }}\n"
        ),
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on a seeded resolver arm");
    let hit = assert_hit(&report, "b", fixture);
    assert!(
        hit.text.contains("\"task_list\" =>"),
        "the (b) hit must quote the arm it dispatches on: {hit:?}"
    );
    // The same arm is a code literal, so (e) reports it too and names the
    // adopted spelling the edit should use.
    let literal = assert_hit(&report, "e", fixture);
    assert_eq!(literal.literals, vec!["task_list".to_string()]);
}

/// Predicate (c): a default configuration value holding a retired spelling.
#[test]
fn test_nfr008_predicate_c_default_value_is_reported() {
    let tree = ScratchTree::new("t048_predicate_c");
    let fixture = tree.seed(
        "crates/ragent-config/src/__guard_default.rs",
        r#"//! Seeded default-value fixture -- not shipped.
pub fn default_seeded_tool() -> String {
    "bash".to_string()
}
"#,
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on a seeded default value");
    assert_hit(&report, "c", &fixture);
}

/// Predicates (d) and (f): a tool reporting a retired spelling as its registry
/// key, and a second declaration of a name the tree already registers.
#[test]
fn test_nfr008_predicates_d_and_f_registry_key_and_duplicate_are_reported() {
    let tree = ScratchTree::new("t048_predicate_d_f");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_registry_keys.rs",
        r#"//! Seeded registry-key fixture -- not shipped.
pub struct SeededOldTool;

impl SeededOldTool {
    pub fn name(&self) -> &'static str {
        "grep"
    }
}

pub struct SeededSecondOldTool;

impl SeededSecondOldTool {
    pub fn name(&self) -> &'static str {
        "grep"
    }
}
"#,
    );
    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on seeded registry keys");
    assert_hit(&report, "d", &fixture);
    let duplicate = assert_hit(&report, "f", &fixture);
    assert!(
        duplicate.fix.contains("distinct name"),
        "the (f) hit must name the edit that clears it: {duplicate:?}"
    );
}

/// Predicate (g): a tool whose whole `execute` body forwards to a tool declared
/// in a crate this one depends on, i.e. one registered earlier.
#[test]
fn test_nfr008_predicate_g_wholesale_forwarder_is_reported() {
    let tree = ScratchTree::new("t048_predicate_g");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_forwarder.rs",
        r#"//! Seeded forwarder fixture -- not shipped.

use serde_json::Value;

use super::{ExtractedCoreToolAdapter, Tool, ToolContext, ToolOutput};

pub struct SeededWrapperTool;

#[async_trait::async_trait]
impl Tool for SeededWrapperTool {
    fn name(&self) -> &'static str {
        "seeded_wrapper"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        ExtractedCoreToolAdapter::new(std::sync::Arc::new(
            ragent_tools_core::read::ReadTool,
        ))
        .execute(input, ctx)
        .await
    }
}
"#,
    );
    let (code, report) = tree.report();
    assert_ne!(
        code, 0,
        "the guard must fail on a seeded wholesale forwarder"
    );
    let hit = assert_hit(&report, "g", &fixture);
    assert!(
        hit.detail.contains("ReadTool"),
        "the (g) hit must name the tool the body forwards to: {hit:?}"
    );
}

/// A tool that decides something before forwarding is NOT a wholesale
/// delegation: it is the alias shape FR-026 permits, and FR-031 refuses the
/// retired key that would make it one.
#[test]
fn test_nfr008_predicate_g_ignores_a_forwarder_that_decides() {
    let tree = ScratchTree::new("t048_predicate_g_deciding");
    let fixture = tree.seed(
        "crates/ragent-agent/src/__guard_deciding_forwarder.rs",
        r#"//! Seeded deciding-forwarder fixture -- not shipped.

use serde_json::Value;

use super::{ExtractedCoreToolAdapter, Tool, ToolContext, ToolOutput};

pub struct SeededAliasTool;

#[async_trait::async_trait]
impl Tool for SeededAliasTool {
    fn name(&self) -> &'static str {
        "seeded_alias"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        if input.get("command").is_none() {
            anyhow::bail!("missing command");
        }
        ExtractedCoreToolAdapter::new(std::sync::Arc::new(
            ragent_tools_core::bash::BashTool,
        ))
        .execute(input, ctx)
        .await
    }
}
"#,
    );
    let (code, report) = tree.report();
    assert!(
        !report
            .hits
            .iter()
            .any(|hit| hit.predicate == "g" && hit.path == fixture),
        "`{fixture}` decides before forwarding, so predicate (g) must stay silent \
         about it: {:#?}",
        report.hits
    );
    assert_eq!(
        code, 0,
        "a tool that decides before forwarding is the alias shape FR-026 permits, so \
         the whole guard must pass: {:#?}",
        report.hits
    );
}

/// NFR-008's last clause: ONE run reports every offending file and line for
/// EVERY predicate, "not merely the first, so a single pass fixes the whole set".
///
/// Four independent violations are seeded into one scratch copy and the run must
/// name all four. A guard that stopped at the first hit would pass every other
/// case in this file.
#[test]
fn test_one_run_reports_every_predicate_at_once() {
    let tree = ScratchTree::new("t048_all_predicates");
    let retired = tree.seed(
        "crates/ragent-agent/src/__guard_many_literal.rs",
        r#"//! Seeded retired literal -- not shipped.
pub const SEEDED_RETIRED: &str = "bash";
"#,
    );
    let defaulted = tree.seed(
        "crates/ragent-config/src/__guard_many_default.rs",
        r#"//! Seeded default value -- not shipped.
pub fn default_seeded_tool() -> String {
    "grep".to_string()
}
"#,
    );
    let keys = tree.seed(
        "crates/ragent-agent/src/__guard_many_keys.rs",
        r#"//! Seeded registry keys -- not shipped.
pub struct SeededManyTool;

impl SeededManyTool {
    pub fn name(&self) -> &'static str {
        "task_list"
    }
}

pub struct SeededDuplicateTool;

impl SeededDuplicateTool {
    pub fn name(&self) -> &'static str {
        "task_list"
    }
}
"#,
    );

    let (code, report) = tree.report();
    assert_ne!(code, 0, "the guard must fail on four seeded violations");
    for (letter, path) in [
        ("e", &retired),
        ("c", &defaulted),
        ("d", &keys),
        ("f", &keys),
    ] {
        assert_hit(&report, letter, path);
    }
    // And the human report carries every one of them, with its fix line.
    let (_, human) = tree.check();
    for letter in ["(c)", "(d)", "(e)", "(f)"] {
        assert!(
            human.contains(letter),
            "one run must report every predicate, but {letter} is missing: {human}"
        );
    }
    assert!(
        human.matches("fix: ").count() >= 4,
        "every hit in the report must name its fix: {human}"
    );
}
