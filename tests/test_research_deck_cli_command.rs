//! Integration tests for the `ragent research deck` CLI arm (spec `diffdeck`
//! T-011: FR-001, FR-010, FR-013, FR-014, FR-017, FR-021).
//!
//! Each test spawns the built `ragent` binary inside a fresh temp working
//! directory with an isolated `XDG_CONFIG_HOME`/`HOME`, writes two dated
//! reports into `research/<name>/`, and asserts the two-part output the CLI
//! contract promises: the machine-readable `ragent-research: {...}` line and
//! the human summary beneath it.
//!
//! The orchestration itself is covered by `ragent-research`'s
//! `test_deck_orchestrate` suite; these tests assert the CLI *wires* to it -
//! that the deck lands on disk, that the counts in both output forms are the
//! written deck's own, and that a refusal fails the process rather than
//! printing a success line.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::MutexGuard;

/// A temp working directory plus an isolated config/home root.
///
/// Holds the cwd lock for its lifetime and restores the previous cwd on drop, so
/// a panicking test cannot leave the process cwd inside a deleted tempdir.
struct Fixture {
    temp: tempfile::TempDir,
    prev: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let lock = crate::global_env_lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(temp.path()).expect("set cwd");
        Self {
            temp,
            prev,
            _lock: lock,
        }
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }

    /// Run `ragent` with `args` against this fixture's isolated environment.
    fn ragent(&self, args: &[&str]) -> Output {
        let config = self.temp.path().join("config");
        let home = self.temp.path().join("home");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::create_dir_all(&home).expect("home dir");
        Command::new(env!("CARGO_BIN_EXE_ragent"))
            .args(args)
            .env("XDG_CONFIG_HOME", &config)
            .env("HOME", &home)
            .output()
            .expect("spawn ragent")
    }

    /// Write one dated report for `name`, using the stamp as its date.
    fn write_report(&self, name: &str, stamp: &str, body: &str) -> PathBuf {
        let dir = self.temp.path().join("research").join(name);
        std::fs::create_dir_all(&dir).expect("item dir");
        let path = dir.join(format!("RESEARCH-{stamp}.md"));
        std::fs::write(&path, body).expect("report");
        path
    }

    /// The item directory for `name`.
    fn item_dir(&self, name: &str) -> PathBuf {
        self.temp.path().join("research").join(name)
    }

    /// Write the standard two-report fixture: an `OLDER` baseline and a `NEWER`
    /// report that adds one concept and rewords Finding 1's headline.
    fn fixture(&self) -> PathBuf {
        let older = "\
# Title: Test Deck Item

## Executive Summary
A stable summary paragraph.

## Concepts

### 1. Alpha Effect

**Definition:** The effect the first arm measured.

## Findings
### **Finding 1** - Alpha effect observed
Body of finding one.

## In-Project Cross-References
- ragent-research: deck.rs
";
        let newer = "\
# Title: Test Deck Item

## Executive Summary
A stable summary paragraph.

## Concepts

### 1. Alpha Effect

**Definition:** The effect the first arm measured.

### 2. Arm Measurement

**Definition:** What the new arm measures.

## Findings
### **Finding 1** - Alpha effect confirmed
Body of finding one.

## In-Project Cross-References
- ragent-research: deck.rs
";
        self.write_report("deckitem", "2025-01-10", older);
        self.write_report("deckitem", "2025-02-14", newer)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The single `ragent-research: {...}` JSON line in `text`, parsed.
///
/// Fails when there is not exactly one such line, which is the contract: a deck
/// run reports itself once, so a consumer can read the outcome without
/// guessing which line is the summary.
fn deck_event(text: &str) -> serde_json::Value {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("ragent-research: {"))
        .collect();
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("ragent-research: "))
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect();
    let decks: Vec<&serde_json::Value> = parsed
        .iter()
        .filter(|value| value["kind"] == "deck")
        .collect();
    assert_eq!(
        decks.len(),
        1,
        "expected exactly one deck event line, got: {text}"
    );
    decks[0].clone()
}

#[test]
fn test_research_deck_cli_writes_deck_and_prints_event_and_summary() {
    let fx = Fixture::new();
    fx.fixture();
    let out = fx.ragent(&[
        "research",
        "deck",
        "deckitem",
        "--from",
        "2025-01-10",
        "--to",
        "2025-02-14",
    ]);
    assert!(
        out.status.success(),
        "deck must succeed: stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);

    // The deck file exists where the default name says it should.
    let decks: Vec<PathBuf> = std::fs::read_dir(fx.item_dir("deckitem"))
        .expect("item dir readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("DECK-") && name.to_lowercase().ends_with(".md")
                })
        })
        .collect();
    assert_eq!(decks.len(), 1, "exactly one deck file must be written");

    // The machine-readable line carries the written deck's own counts.
    let event = deck_event(&text);
    let payload = &event["payload"];
    assert_eq!(payload["name"], "deckitem");
    assert_eq!(payload["from"], "2025-01-10");
    assert_eq!(payload["to"], "2025-02-14");
    assert_eq!(payload["no_changes"], false);
    assert_eq!(
        payload["path"].as_str().map(str::to_string),
        Some(decks[0].display().to_string()),
        "the event must name the file that was written"
    );
    // FR-009: a reworded finding headline is NEW, because the headline is the
    // finding's identity - so the pair yields two NEW and three OLD items (the
    // unchanged summary, the concept the newer report kept, and the unchanged
    // cross-reference).
    assert_eq!(payload["new"], 2);
    assert_eq!(payload["added"], 0);
    assert_eq!(payload["update"], 0);
    assert_eq!(payload["old"], 3);
    assert_eq!(payload["changes"], 2);
    assert_eq!(
        payload["element_slides"],
        serde_json::json!(["Concepts", "Findings"])
    );

    // The human summary repeats the same totals in the non-TUI spelling.
    assert!(
        text.contains("ragent-research: wrote `"),
        "summary must carry the CLI attribution: {text}"
    );
    assert!(
        text.contains("NEW 2, ADDED 0, UPDATE 0, OLD 3"),
        "summary must carry the per-status counts: {text}"
    );
    assert!(
        text.contains("2 change(s) across 2 element slide(s)"),
        "summary must carry the change and slide totals: {text}"
    );
    assert!(
        !text.contains("From: /research deck"),
        "the CLI must not print the TUI attribution: {text}"
    );
}

#[test]
fn test_research_deck_cli_elements_filter_narrows_the_deck() {
    let fx = Fixture::new();
    fx.fixture();
    let out = fx.ragent(&[
        "research",
        "deck",
        "deckitem",
        "--elements",
        "concepts",
        "--out",
        "only-concepts.md",
    ]);
    assert!(
        out.status.success(),
        "filtered deck must succeed: stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);
    let event = deck_event(&text);
    let payload = &event["payload"];
    // FR-014: the filter runs before rendering, so the counts cover only the
    // elements the deck wrote - two concept slides, one NEW against one OLD.
    assert_eq!(payload["new"], 1);
    assert_eq!(payload["old"], 1);
    assert_eq!(payload["changes"], 1);
    assert_eq!(payload["element_slides"], serde_json::json!(["Concepts"]));
    assert!(
        text.contains("1 change(s) across 1 element slide(s)"),
        "summary must report the narrowed totals: {text}"
    );
    assert!(
        fx.path().join("only-concepts.md").exists(),
        "--out must write the deck where it was asked to"
    );
}

#[test]
fn test_research_deck_cli_identical_reports_report_no_changes() {
    let fx = Fixture::new();
    let body = "\
# Title: Stable Item

## Executive Summary
Nothing changes here.

## Findings
### **Finding 1** - Unchanged headline
Unchanged body.
";
    fx.write_report("stable", "2025-01-10", body);
    fx.write_report("stable", "2025-02-14", body);
    let out = fx.ragent(&["research", "deck", "stable"]);
    assert!(
        out.status.success(),
        "deck must succeed: stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
    let text = stdout(&out);
    let event = deck_event(&text);
    assert_eq!(event["payload"]["no_changes"], true);
    assert_eq!(
        event["payload"]["element_slides"],
        serde_json::json!([]),
        "a no-change deck renders no element slide"
    );
    // FR-013: the summary says so explicitly rather than reporting zeroes.
    assert!(
        text.contains("no changes detected between the two reports"),
        "summary must name the no-change case: {text}"
    );
    assert!(
        !text.contains("NEW 0, ADDED 0, UPDATE 0, OLD 0"),
        "a no-change deck must not read as a deck full of zeroes: {text}"
    );
}

#[test]
fn test_research_deck_cli_refuses_a_missing_item_without_writing() {
    let fx = Fixture::new();
    fx.fixture();
    let out = fx.ragent(&["research", "deck", "no-such-item"]);
    assert!(
        !out.status.success(),
        "a missing item must fail the process: stdout={}",
        stdout(&out)
    );
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        combined.contains("no-such-item"),
        "the refusal must name the item: {combined}"
    );
    assert!(
        combined.contains("not found"),
        "the refusal must say the item was not found: {combined}"
    );
    assert!(
        !fx.item_dir("no-such-item").exists(),
        "FR-018: a refused run must create no directory"
    );
    assert!(
        !combined.contains("ragent-research: wrote `"),
        "a refused run must not print a success summary: {combined}"
    );
}

#[test]
fn test_research_deck_cli_refuses_an_existing_deck_without_force() {
    let fx = Fixture::new();
    fx.fixture();
    let out_path = fx.path().join("existing.md");
    std::fs::write(&out_path, "sentinel\n").expect("sentinel");

    let refused = fx.ragent(&[
        "research",
        "deck",
        "deckitem",
        "--out",
        "existing.md",
        "--from",
        "2025-01-10",
        "--to",
        "2025-02-14",
    ]);
    assert!(
        !refused.status.success(),
        "an existing deck without --force must fail: stdout={}",
        stdout(&refused)
    );
    let combined = format!("{}{}", stdout(&refused), stderr(&refused));
    assert!(
        combined.contains("--force"),
        "the refusal must name the overwriting flag: {combined}"
    );
    assert_eq!(
        std::fs::read_to_string(&out_path).expect("sentinel readable"),
        "sentinel\n",
        "FR-017: a refused run must leave the existing file byte-identical"
    );

    // With --force the same invocation overwrites it.
    let forced = fx.ragent(&[
        "research",
        "deck",
        "deckitem",
        "--out",
        "existing.md",
        "--force",
    ]);
    assert!(
        forced.status.success(),
        "--force must overwrite: stdout={} stderr={}",
        stdout(&forced),
        stderr(&forced)
    );
    let written = std::fs::read_to_string(&out_path).expect("deck readable");
    assert!(
        written.contains("# Test Deck Item - Research Diff Deck"),
        "the overwritten file must be the deck: {written}"
    );
}

#[test]
fn test_research_deck_cli_rejects_an_unknown_element_name() {
    let fx = Fixture::new();
    fx.fixture();
    let out = fx.ragent(&[
        "research",
        "deck",
        "deckitem",
        "--elements",
        "not-an-element",
    ]);
    assert!(
        !out.status.success(),
        "an unknown element name must fail: stdout={}",
        stdout(&out)
    );
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        combined.contains("not-an-element"),
        "the refusal must name the offending value: {combined}"
    );
    assert!(
        combined.contains("Executive Summary"),
        "the refusal must list the accepted values: {combined}"
    );
    // The refusal happens before any run, so no deck is written.
    assert!(
        !fx.item_dir("deckitem").join("DECK-today.md").exists(),
        "a rejected filter must write nothing"
    );
}

#[test]
fn test_research_deck_cli_twice_in_a_day_is_byte_identical() {
    let fx = Fixture::new();
    fx.fixture();
    let first = fx.ragent(&["research", "deck", "deckitem"]);
    assert!(first.status.success(), "first run must succeed");
    let deck = std::fs::read_dir(fx.item_dir("deckitem"))
        .expect("item dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("DECK-"))
        })
        .expect("a deck file");
    let before = std::fs::read_to_string(&deck).expect("deck readable");

    // FR-021: a second run in the same day writes the same bytes under --force.
    let second = fx.ragent(&["research", "deck", "deckitem", "--force"]);
    assert!(
        second.status.success(),
        "second run must succeed: stderr={}",
        stderr(&second)
    );
    let after = std::fs::read_to_string(&deck).expect("deck readable");
    assert_eq!(before, after, "the same pair must render identically");
}

// -- stage progress on stderr (FR-031) --------------------------------------

/// A deck run narrates its stages on stderr, leaving stdout one JSON record.
///
/// The CLI is a presentation surface, so a run that spends real time reports
/// itself rather than going quiet. The reports must go to **stderr** for exactly
/// this reason: `stdout` carries the machine-readable `ragent-research: {...}`
/// line, and a progress line mixed into it would make a piped consumer parse a
/// record it did not ask for. Every stage of a model-free run still reports - the
/// enrichment stages as skipped - so a reader never has to infer "no provider"
/// from an absence.
#[test]
fn test_deck_run_reports_stage_progress_on_stderr() {
    let fixture = Fixture::new();
    fixture.fixture();
    let out = fixture.ragent(&["research", "deck", "deckitem"]);
    assert!(
        out.status.success(),
        "the fixture pair produces a deck: {}",
        stderr(&out)
    );

    let err = stderr(&out);
    for stage in ["select", "read", "diff", "destination", "render"] {
        assert!(
            err.contains(&format!("ragent-research: deck {stage} ...")),
            "stage `{stage}` must announce itself on stderr; stderr was:\n{err}"
        );
        assert!(
            err.contains(&format!("ragent-research: deck {stage} done")),
            "stage `{stage}` must report completion; stderr was:\n{err}"
        );
    }
    // The countable stages report whether they ran or were skipped, and never
    // in silence: an active default model makes the enrichment stage run, and a
    // run with no presentation server reports the export as skipped with its
    // reason. Either way the stage is named.
    assert!(
        err.contains("deck enrich findings ...") || err.contains("deck enrich findings skipped"),
        "the enrichment stage must report; stderr was:\n{err}"
    );
    assert!(
        err.contains("deck export skipped"),
        "the export must report why it did not run; stderr was:\n{err}"
    );

    // The progress lines must not leak into stdout.
    let out_text = stdout(&out);
    assert!(
        !out_text.contains("ragent-research: deck "),
        "progress belongs on stderr, not stdout; stdout was:\n{out_text}"
    );
    // stdout still carries exactly one parseable deck record.
    let event = deck_event(&out_text);
    assert_eq!(event["kind"], "deck");
}
