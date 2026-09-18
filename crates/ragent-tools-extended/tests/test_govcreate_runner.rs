//! Tests for the `/spec govcreate` orchestration runner (spec `govdoc` T-012;
//! FR-006, FR-011, FR-012, FR-013, FR-014, FR-019).
//!
//! The runner is pure orchestration over injected [`GovCreateStages`]; the
//! suite drives it with in-memory fakes so every ordering and
//! failure-containment rule is exercised without the network, the filesystem,
//! or an LLM. The one real side effect is the T-005 scaffold stage, which
//! runs against `tempfile::TempDir` targets per its own suite's convention.

use std::sync::{Arc, Mutex};

use ragent_tools_extended::archdoc::govcreate_run::{
    AuthoredSpec, CancellationToken, CompletedStage, GovCreateOutcome, GovCreateRun,
    GovCreateRunError, GovCreateRunReport, GovCreateStages, run_govcreate,
};
use ragent_tools_extended::archdoc::{
    AcquisitionStats, ArchitectureStructure, Component, ContentRef, GatheredCorpus, GatheredSource,
};
use ragent_tools_extended::masterfetch::PageType;
use ragent_tools_extended::project_scaffold::{ScaffoldRequest, parse_flags};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn scaffold_request() -> ScaffoldRequest {
    parse_flags(&["--language", "rust", "--type", "cmdline"]).expect("valid flags")
}

fn sample_corpus(reference: &str) -> GatheredCorpus {
    GatheredCorpus {
        reference: reference.to_owned(),
        sources: vec![GatheredSource {
            url: "mem://doc".to_owned(),
            summary: "Doc".to_owned(),
            page_type: PageType::Article,
            content: "Payment Gateway authorises payments. Ledger records postings.".to_owned(),
        }],
        excluded: Vec::new(),
        text: "Payment Gateway authorises payments. Ledger records postings.".to_owned(),
        stats: AcquisitionStats::default(),
        budget_reached: None,
    }
}

fn sample_structure() -> ArchitectureStructure {
    ArchitectureStructure {
        components: vec![Component {
            name: "Payment Gateway".to_owned(),
            responsibilities: vec!["authorise payments".to_owned()],
        }],
        interfaces: Vec::new(),
        data_stores: Vec::new(),
        external_dependencies: Vec::new(),
        relationships: Vec::new(),
        from_fallback: false,
    }
}

/// Injected stage effects recorded for later assertion.
#[derive(Debug, Default)]
struct StageLog {
    calls: Vec<&'static str>,
}

/// A fully succeed-at-everything fake. Each stage records its call and
/// returns a fixed, shaped value; individual tests swap in failures by
/// overriding the relevant field before handing the fake to the runner.
struct FakeStages {
    log: Arc<Mutex<StageLog>>,
    acquire: Result<GatheredCorpus, GovCreateRunError>,
    extract: Result<ArchitectureStructure, GovCreateRunError>,
    author: Result<AuthoredSpec, GovCreateRunError>,
    write: Result<(), GovCreateRunError>,
}

impl Default for FakeStages {
    fn default() -> Self {
        Self {
            log: Arc::default(),
            acquire: Ok(sample_corpus("mem://arch")),
            extract: Ok(sample_structure()),
            author: Ok(AuthoredSpec {
                spec_md: "SPEC".to_owned(),
                plan_md: "PLAN".to_owned(),
                testplan_md: "TESTPLAN".to_owned(),
            }),
            write: Ok(()),
        }
    }
}

impl FakeStages {
    fn calls(&self) -> Vec<&'static str> {
        self.log.lock().expect("stage log lock").calls.clone()
    }
}

impl GovCreateStages for FakeStages {
    async fn acquire(&self, _reference: &ContentRef) -> Result<GatheredCorpus, GovCreateRunError> {
        self.log.lock().expect("lock").calls.push("acquire");
        self.acquire.clone()
    }

    async fn extract(
        &self,
        _corpus: &GatheredCorpus,
    ) -> Result<ArchitectureStructure, GovCreateRunError> {
        self.log.lock().expect("lock").calls.push("extract");
        self.extract.clone()
    }

    async fn author(
        &self,
        _structure: &ArchitectureStructure,
        _run: &GovCreateRun,
    ) -> Result<AuthoredSpec, GovCreateRunError> {
        self.log.lock().expect("lock").calls.push("author");
        self.author.clone()
    }

    async fn write_spec(
        &self,
        _run: &GovCreateRun,
        _authored: &AuthoredSpec,
    ) -> Result<(), GovCreateRunError> {
        self.log.lock().expect("lock").calls.push("write");
        self.write.clone()
    }
}

/// Build a run rooted in a fresh tempdir whose content ref is a small
/// markdown file inside the same dir (passes both classification and
/// validation).
fn make_run() -> (tempfile::TempDir, GovCreateRun) {
    let temp = tempfile::tempdir().expect("tempdir");
    let doc = temp.path().join("arch.md");
    std::fs::write(
        &doc,
        "# Architecture\n\nPayment Gateway authorises payments.\n",
    )
    .expect("write fixture doc");
    let run = GovCreateRun {
        spec_id: "payments-arch".to_owned(),
        content_ref: "arch.md".to_owned(),
        target_folder: temp.path().join("target-svc"),
        scaffold: scaffold_request(),
        force: false,
        invoking_root: temp.path().to_path_buf(),
    };
    (temp, run)
}

fn completed_ids(report: &GovCreateRunReport) -> Vec<&'static str> {
    report.completed.iter().map(|s| s.id).collect()
}

// ---------------------------------------------------------------------------
// FR-012 ordering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_reports_stages_in_fr012_order() {
    let (_temp, run) = make_run();
    let stages = FakeStages::default();
    let cancel = CancellationToken::none();

    let report = run_govcreate(&run, &stages, &cancel).await;

    assert!(report.succeeded(), "run succeeds: {:?}", report.outcome);
    assert_eq!(
        completed_ids(&report),
        vec!["guard", "scaffold", "acquire", "extract", "author", "write"],
        "FR-012 order",
    );
    assert_eq!(
        stages.calls(),
        vec!["acquire", "extract", "author", "write"],
        "injected stages ran in FR-012 order after the real scaffold",
    );

    match &report.outcome {
        GovCreateOutcome::Completed { spec_dir, .. } => {
            assert_eq!(
                *spec_dir,
                run.target_folder.join("specs").join("payments-arch"),
                "spec dir is <target>/specs/<specid> (FR-009)"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// FR-006 pre-scaffold validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_fr006_bad_content_ref_leaves_target_unchanged() {
    let temp = tempfile::tempdir().expect("tempdir");
    let target = temp.path().join("untouched");
    let run = GovCreateRun {
        spec_id: "payments-arch".to_owned(),
        content_ref: "does-not-exist.md".to_owned(),
        target_folder: target.clone(),
        scaffold: scaffold_request(),
        force: false,
        invoking_root: temp.path().to_path_buf(),
    };

    let stages = FakeStages::default();
    let report = run_govcreate(&run, &stages, &CancellationToken::none()).await;

    assert_eq!(
        completed_ids(&report),
        Vec::<&'static str>::new(),
        "no stages ran: classification failed pre-guard",
    );
    match &report.outcome {
        GovCreateOutcome::NoUsableContent { cause } => {
            assert!(
                cause.contains("path not found") || cause.contains("not found"),
                "FR-006 cause names the path: {cause}"
            );
        }
        other => panic!("expected NoUsableContent, got {other:?}"),
    }
    assert!(
        !target.exists(),
        "FR-006: target folder unchanged when the content ref is invalid",
    );
    assert!(
        stages.calls().is_empty(),
        "no injected stage ran for an invalid content ref"
    );
}

#[tokio::test]
async fn test_govcreate_runner_fr006_empty_corpus_aborts_after_acquisition() {
    let (_temp, run) = make_run();
    let mut empty = sample_corpus(&run.content_ref);
    empty.sources.clear();
    empty.text.clear();
    let stages = FakeStages {
        acquire: Ok(empty),
        ..Default::default()
    };

    let report = run_govcreate(&run, &stages, &CancellationToken::none()).await;

    assert_eq!(
        completed_ids(&report),
        vec!["guard", "scaffold", "acquire"],
        "empty corpus aborts after acquisition (guard+scaffold completed)",
    );
    match &report.outcome {
        GovCreateOutcome::NoUsableContent { cause } => {
            assert!(cause.contains("empty corpus"), "{cause}");
        }
        other => panic!("expected NoUsableContent, got {other:?}"),
    }
    assert_eq!(
        stages.calls(),
        vec!["acquire"],
        "extraction and authoring never ran",
    );
}

// ---------------------------------------------------------------------------
// FR-011 empty-target guard
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_fr011_non_empty_target_names_blocking_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let target = temp.path().join("busy");
    std::fs::create_dir_all(&target).expect("mkdir");
    std::fs::write(target.join("keepme.txt"), b"occupied").expect("seed");
    let doc = temp.path().join("arch.md");
    std::fs::write(&doc, "content").expect("doc");

    let run = GovCreateRun {
        spec_id: "payments-arch".to_owned(),
        content_ref: "arch.md".to_owned(),
        target_folder: target.clone(),
        scaffold: scaffold_request(),
        force: false,
        invoking_root: temp.path().to_path_buf(),
    };

    let stages = FakeStages::default();
    let report = run_govcreate(&run, &stages, &CancellationToken::none()).await;

    assert_eq!(
        completed_ids(&report),
        vec!["guard"],
        "guard passed, scaffold refused: only the guard stage completed"
    );
    match &report.outcome {
        GovCreateOutcome::NonEmptyTarget { blocking } => {
            assert!(
                blocking.iter().any(|b| b.contains("keepme.txt")),
                "names the blocking entry: {blocking:?}"
            );
        }
        other => panic!("expected NonEmptyTarget, got {other:?}"),
    }
    assert!(
        stages.calls().is_empty(),
        "FR-011: no extraction, generation, or write follows the guard refusal"
    );
    // Nothing extra was written to the target.
    assert_eq!(
        std::fs::read_dir(&target).expect("read").count(),
        1,
        "only keepme.txt remains"
    );
}

// ---------------------------------------------------------------------------
// FR-013 failure containment and partial-result report
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_fr013_post_scaffold_failure_preserves_scaffold() {
    let (_temp, run) = make_run();
    let stages = FakeStages {
        extract: Err(GovCreateRunError::Extract("model timeout".to_owned())),
        ..Default::default()
    };

    let report = run_govcreate(&run, &stages, &CancellationToken::none()).await;

    // The scaffold stage really ran: the project directory exists.
    assert!(
        run.target_folder.join("Cargo.toml").is_file(),
        "scaffold remains on disk"
    );
    assert_eq!(
        completed_ids(&report),
        vec!["guard", "scaffold", "acquire"],
        "failed stage is not recorded as completed"
    );
    match &report.outcome {
        GovCreateOutcome::StageFailed { stage, cause } => {
            assert_eq!(*stage, "extract");
            assert_eq!(cause, "model timeout");
        }
        other => panic!("expected StageFailed, got {other:?}"),
    }
    // FR-013 contract: no spec was generated.
    assert!(
        !run.target_folder.join("specs/payments-arch").exists(),
        "no spec files were written"
    );
    assert_eq!(stages.calls(), vec!["acquire", "extract"]);
}

// ---------------------------------------------------------------------------
// FR-014 scaffold failure leaves no spec
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_fr014_scaffold_failure_blocks_downstream_stages() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Guard-refusing target triggers the `Guard` variant; to force the
    // `Emit`-style failure instead we hand the scaffold a target whose parent
    // component is not creatable: a file where the target folder needs to be
    // a directory.
    let target = temp.path().join("blocked");
    std::fs::write(&target, b"i am a file").expect("seed a file");
    std::fs::write(temp.path().join("arch.md"), "content").expect("doc");

    let run = GovCreateRun {
        spec_id: "payments-arch".to_owned(),
        content_ref: "arch.md".to_owned(),
        target_folder: target.clone(), // create_dir_all fails here
        scaffold: scaffold_request(),
        force: false,
        invoking_root: temp.path().to_path_buf(),
    };
    let stages = FakeStages::default();
    let report = run_govcreate(&run, &stages, &CancellationToken::none()).await;

    assert_eq!(
        completed_ids(&report),
        vec!["guard"],
        "only the content-ref guard completed before the scaffold failure",
    );
    // The target-creation failure maps to `ScaffoldFailed`, not
    // `NonEmptyTarget`. FR-014's contract is what matters here: no
    // acquisition, no authoring, no partial spec.
    match &report.outcome {
        GovCreateOutcome::ScaffoldFailed { cause } => {
            assert!(
                cause.contains("cannot create target folder") || cause.contains("target folder"),
                "cause describes the scaffold failure: {cause}"
            );
        }
        other => panic!("expected ScaffoldFailed, got {other:?}"),
    }
    assert!(
        stages.calls().is_empty(),
        "FR-014: acquisition never runs when the scaffold fails",
    );
}

// ---------------------------------------------------------------------------
// FR-019 cancellation at stage boundaries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_govcreate_runner_fr019_cancel_before_any_stage() {
    let (_temp, run) = make_run();
    let cancel = CancellationToken::none();
    cancel.cancel();

    let stages = FakeStages::default();
    let report = run_govcreate(&run, &stages, &cancel).await;

    assert_eq!(
        completed_ids(&report),
        Vec::<&'static str>::new(),
        "nothing ran: cancel landed before the guard",
    );
    assert!(
        matches!(
            report.outcome,
            GovCreateOutcome::Cancelled {
                stopped_before: "guard"
            }
        ),
        "{:?}",
        report.outcome
    );
    assert!(
        !run.target_folder.exists(),
        "no scaffold writes after cancel"
    );
    assert!(stages.calls().is_empty());
}

#[tokio::test]
async fn test_govcreate_runner_fr019_cancel_at_write_boundary_leaves_no_spec() {
    let (_temp, run) = make_run();
    let cancel = CancellationToken::none();

    // Cancel flips *after* author so the write boundary sees it: the runner
    // checks cancellation between stages, never inside one.
    struct CancellingStages {
        inner: FakeStages,
        token: CancellationToken,
    }
    impl GovCreateStages for CancellingStages {
        async fn acquire(
            &self,
            reference: &ContentRef,
        ) -> Result<GatheredCorpus, GovCreateRunError> {
            self.inner.acquire(reference).await
        }
        async fn extract(
            &self,
            corpus: &GatheredCorpus,
        ) -> Result<ArchitectureStructure, GovCreateRunError> {
            self.inner.extract(corpus).await
        }
        async fn author(
            &self,
            structure: &ArchitectureStructure,
            run: &GovCreateRun,
        ) -> Result<AuthoredSpec, GovCreateRunError> {
            let out = self.inner.author(structure, run).await;
            // User pressed Escape while author was in flight.
            self.token.cancel();
            out
        }
        async fn write_spec(
            &self,
            run: &GovCreateRun,
            authored: &AuthoredSpec,
        ) -> Result<(), GovCreateRunError> {
            self.inner.write_spec(run, authored).await
        }
    }

    let stages = CancellingStages {
        inner: FakeStages::default(),
        token: cancel.clone(),
    };
    let report = run_govcreate(&run, &stages, &cancel).await;

    assert_eq!(
        completed_ids(&report),
        vec!["guard", "scaffold", "acquire", "extract", "author"],
    );
    assert!(
        matches!(
            report.outcome,
            GovCreateOutcome::Cancelled {
                stopped_before: "write"
            }
        ),
        "{:?}",
        report.outcome
    );
    // The write stage never ran: the cancelled run leaves no spec files.
    assert!(
        !run.target_folder.join("specs/payments-arch").exists(),
        "FR-019: no spec written after the cancel point"
    );
    // Scaffold persisted - cancellation does not roll back what completed.
    assert!(run.target_folder.join("Cargo.toml").is_file());
}

// ---------------------------------------------------------------------------
// FR-013 report rendering (NFR-005)
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_runner_report_render_lists_completed_stages_and_cause() {
    let report = GovCreateRunReport {
        completed: vec![
            CompletedStage {
                id: "guard",
                note: "content reference validated".to_owned(),
            },
            CompletedStage {
                id: "scaffold",
                note: "scaffolded 'payments-svc'".to_owned(),
            },
        ],
        outcome: GovCreateOutcome::StageFailed {
            stage: "extract",
            cause: "model timeout".to_owned(),
        },
    };
    let rendered = report.render();
    for needle in [
        "[ ok ] guard",
        "[ ok ] scaffold",
        "[fail] extract: model timeout",
        "no spec was generated",
    ] {
        assert!(
            rendered.contains(needle),
            "render contains {needle}: {rendered}"
        );
    }
}

#[test]
fn test_govcreate_runner_render_cancelled_mentions_boundary_and_no_spec() {
    let report = GovCreateRunReport {
        completed: vec![CompletedStage {
            id: "guard",
            note: String::new(),
        }],
        outcome: GovCreateOutcome::Cancelled {
            stopped_before: "acquire",
        },
    };
    let rendered = report.render();
    assert!(
        rendered.contains("cancelled before the acquire stage"),
        "{rendered}"
    );
    assert!(rendered.contains("no spec was written"), "{rendered}");
}

// ---------------------------------------------------------------------------
// Cancellation flag plumbing
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_cancellation_token_clone_shares_flag() {
    let a = CancellationToken::none();
    let b = a.clone();
    assert!(!a.is_cancelled());
    a.cancel();
    assert!(b.is_cancelled(), "clones observe the shared flag");
}

// Compile-time shape check: the runner must accept a non-'static stages
// reference so tests can build fakes inline.
#[allow(dead_code)]
fn _assert_runner_is_flexible<'a, S: GovCreateStages>(
    run: &'a GovCreateRun,
    stages: &'a S,
    token: &'a CancellationToken,
) -> impl std::future::Future<Output = GovCreateRunReport> + 'a {
    run_govcreate(run, stages, token)
}

// ---------------------------------------------------------------------------
// FR-015 staged-progress streaming (T-013)
// ---------------------------------------------------------------------------

use ragent_tools_extended::archdoc::govcreate_run::{
    GovCreateProgress, run_govcreate_with_progress, stage_label,
};

/// Drain the events a full happy-path run emits and check the FR-015 ordering
/// contract: `Started(id)` precedes that stage's `StageCompleted`, one
/// `Terminal` event lands last, every stage appears exactly once.
#[tokio::test]
async fn test_govcreate_progress_stream_happy_path_order() {
    let (_temp, run) = make_run();
    let stages = FakeStages::default();
    let cancel = CancellationToken::none();

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_sink = Arc::clone(&events);
    let report = run_govcreate_with_progress(&run, &stages, &cancel, &mut |event| {
        events_sink
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event)
    })
    .await;
    let events = events.lock().unwrap_or_else(|p| p.into_inner()).clone();

    assert!(report.succeeded(), "happy path completes: {:?}", report);

    let stage_ids = ["guard", "scaffold", "acquire", "extract", "author", "write"];
    let mut last_pos = 0usize;
    for id in stage_ids {
        let started = events
            .iter()
            .position(|e| matches!(e, GovCreateProgress::Started { id: s } if *s == id))
            .unwrap_or_else(|| panic!("started {id}: {events:?}"));
        let completed_at = events
            .iter()
            .position(|e| matches!(e, GovCreateProgress::StageCompleted(s) if s.id == id))
            .unwrap_or_else(|| panic!("completed {id}: {events:?}"));
        assert!(started < completed_at, "started {id} before its completion");
        assert!(started >= last_pos, "stage {id} maintains stage order");
        last_pos = completed_at;
    }
    match events.last() {
        Some(GovCreateProgress::Terminal(GovCreateOutcome::Completed { .. })) => {}
        other => panic!("terminal event is Completed, got {other:?}"),
    }
}

/// Every failure path emits exactly one Terminal event and no `Started` for
/// the stages that never ran (FR-015 + FR-013 containment).
#[tokio::test]
async fn test_govcreate_progress_stream_fr013_failure_terminal_only() {
    let (_temp, run) = make_run();
    let stages = FakeStages {
        extract: Err(GovCreateRunError::Extract("model timeout".to_owned())),
        ..Default::default()
    };
    let cancel = CancellationToken::none();

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_sink = Arc::clone(&events);
    let report = run_govcreate_with_progress(&run, &stages, &cancel, &mut |event| {
        events_sink
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event)
    })
    .await;
    let events = events.lock().unwrap_or_else(|p| p.into_inner()).clone();

    assert!(
        matches!(
            report.outcome,
            GovCreateOutcome::StageFailed {
                stage: "extract",
                ..
            }
        ),
        "{:?}",
        report.outcome
    );
    let terminals = events
        .iter()
        .filter(|e| matches!(e, GovCreateProgress::Terminal(_)))
        .count();
    assert_eq!(terminals, 1, "exactly one terminal event: {events:?}");
    assert!(
        !events.iter().any(|e| matches!(
            e,
            GovCreateProgress::Started {
                id: "author" | "write"
            }
        )),
        "author/write never start after the extract failure: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GovCreateProgress::Started { id: "extract" })),
        "extract started before failing: {events:?}"
    );
}

/// FR-019 path: cancellation before the guard still emits the terminal event
/// so the TUI panel always reaches its final state.
#[tokio::test]
async fn test_govcreate_progress_stream_fr019_cancel_emits_terminal() {
    let (_temp, run) = make_run();
    let cancel = CancellationToken::none();
    cancel.cancel();
    let stages = FakeStages::default();

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_sink = Arc::clone(&events);
    let _report = run_govcreate_with_progress(&run, &stages, &cancel, &mut |event| {
        events_sink
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event)
    })
    .await;
    let events = events.lock().unwrap_or_else(|p| p.into_inner()).clone();

    assert_eq!(events.len(), 1, "exactly the terminal event: {events:?}");
    assert!(
        matches!(
            events.first(),
            Some(GovCreateProgress::Terminal(GovCreateOutcome::Cancelled {
                stopped_before: "guard"
            }))
        ),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, GovCreateProgress::Started { .. })),
        "no stage started: {events:?}"
    );
}

/// NFR-005 render contract: lines stay in the `[ .. ]` / `[ ok ]` / `[fail]`
/// / `[note]` ASCII vocabulary the rest of the TUI panels use.
#[test]
fn test_govcreate_progress_render_nfr005_shapes() {
    let started = GovCreateProgress::Started { id: "acquire" };
    assert_eq!(started.render(), "[ .. ] acquire");

    let done = GovCreateProgress::StageCompleted(CompletedStage {
        id: "scaffold",
        note: "scaffolded 'demo'".to_owned(),
    });
    assert_eq!(done.render(), "[ ok ] scaffold - scaffolded 'demo'");

    let terminal_ok = GovCreateProgress::Terminal(GovCreateOutcome::Completed {
        spec_dir: std::path::PathBuf::from("x/specs/y"),
        budget_note: None,
    });
    assert_eq!(terminal_ok.render(), "[ ok ] complete");

    let cancelled = GovCreateProgress::Terminal(GovCreateOutcome::Cancelled {
        stopped_before: "author",
    });
    assert_eq!(
        cancelled.render(),
        "[note] cancelled before the author stage"
    );

    let failed = GovCreateProgress::Terminal(GovCreateOutcome::StageFailed {
        stage: "extract",
        cause: "m".to_owned(),
    });
    assert_eq!(failed.render(), "[fail] extract");

    // Stage labels exist for every id the runner can emit.
    for id in ["guard", "scaffold", "acquire", "extract", "author", "write"] {
        assert!(!stage_label(id).is_empty());
    }
}
