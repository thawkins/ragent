//! Inline tests for `run_manifest.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn light_steps_skip_adversarial_pipeline() {
    let steps = RunStep::steps_for_tier(Tier::Light);
    assert!(steps.contains(&RunStep::Decompose));
    assert!(steps.contains(&RunStep::Synthesize));
    assert!(!steps.contains(&RunStep::ContradictionGraph));
    assert!(!steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn full_steps_include_all_sixteen() {
    let steps = RunStep::steps_for_tier(Tier::Full);
    assert!(steps.contains(&RunStep::ContradictionGraph));
    assert!(steps.contains(&RunStep::LociAnalysis));
    assert!(steps.contains(&RunStep::Critics));
    assert!(steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn dissertation_steps_start_with_chapter_partition() {
    let steps = RunStep::steps_for_tier(Tier::Dissertation);
    assert_eq!(steps[0], RunStep::ChapterPartition);
    assert!(steps.contains(&RunStep::Synthesize));
}

#[test]
fn manifest_tracks_step_lifecycle() {
    let mut m = RunManifest::new("run-1", "glp1", "GLP-1 outcomes", Tier::Full);
    assert_eq!(m.next_pending_step(), Some(RunStep::Decompose));

    assert!(m.start_step(RunStep::Decompose));
    assert_eq!(m.steps[0].status, StepStatus::InProgress);

    assert!(m.complete_step(RunStep::Decompose));
    assert_eq!(m.steps[0].status, StepStatus::Completed);
    assert_eq!(m.next_pending_step(), Some(RunStep::WidthSweep));

    m.fail_step(RunStep::WidthSweep, Some("network".into()));
    assert_eq!(m.steps[1].status, StepStatus::Failed);
    assert!(m.next_pending_step().is_some());
}

#[test]
fn manifest_serializes_and_deserializes() {
    let mut m = RunManifest::new("run-2", "demo", "demo topic", Tier::Light);
    m.start_step(RunStep::Decompose);
    m.complete_step(RunStep::Decompose);
    m.skip_step(RunStep::WidthSweep, Some("not needed".into()));

    let json = m.to_json().unwrap();
    let restored = RunManifest::from_json(&json).unwrap();
    assert_eq!(m, restored);
}

#[test]
fn resume_restarts_from_first_pending_or_in_progress() {
    let mut m = RunManifest::new("run-3", "demo", "demo topic", Tier::Full);
    m.start_step(RunStep::Decompose);
    m.complete_step(RunStep::Decompose);
    m.complete_step(RunStep::WidthSweep);
    m.fail_step(RunStep::ContradictionGraph, None);

    m.resume();
    assert!(m.started_at.is_some());
    assert_eq!(m.completed_at, None);
    // The failed contradiction_graph step is the first non-completed step,
    // so the resume cursor should point at it (the run may attempt to redo
    // it or skip it depending on policy).
    assert_eq!(m.next_pending_step(), Some(RunStep::ContradictionGraph));
}

#[test]
fn supervisor_steps_use_graph_pipeline() {
    let steps = RunStep::steps_for_mode(ResearchMode::Supervisor, Tier::Full);
    assert_eq!(
        steps,
        vec![
            RunStep::SupervisorPlan,
            RunStep::SupervisorDelegate,
            RunStep::SupervisorSynthesize,
            RunStep::SupervisorFinalize,
        ]
    );
}

#[test]
fn competitive_steps_use_graph_pipeline() {
    let steps = RunStep::steps_for_mode(ResearchMode::Competitive, Tier::Full);
    assert_eq!(
        steps,
        vec![
            RunStep::SupervisorPlan,
            RunStep::SupervisorDelegate,
            RunStep::SupervisorSynthesize,
            RunStep::SupervisorFinalize,
        ]
    );
}

#[test]
fn tiered_mode_falls_back_to_tier_steps() {
    let tiered = RunStep::steps_for_mode(ResearchMode::Tiered, Tier::Full);
    assert_eq!(tiered, RunStep::steps_for_tier(Tier::Full));
}

#[test]
fn manifest_with_mode_tracks_supervisor_steps() {
    let mut m = RunManifest::new_with_mode(
        "run-sv",
        "demo",
        "demo topic",
        Tier::Full,
        ResearchMode::Supervisor,
    );
    assert_eq!(m.mode, ResearchMode::Supervisor);
    assert_eq!(m.next_pending_step(), Some(RunStep::SupervisorPlan));

    assert!(m.start_step(RunStep::SupervisorPlan));
    assert_eq!(m.steps[0].status, StepStatus::InProgress);

    assert!(m.complete_step(RunStep::SupervisorPlan));
    assert_eq!(m.next_pending_step(), Some(RunStep::SupervisorDelegate));
}

#[test]
fn manifest_supervisor_round_trips_json() {
    let mut m = RunManifest::new_with_mode(
        "run-sv",
        "demo",
        "demo topic",
        Tier::Full,
        ResearchMode::Supervisor,
    );
    m.start_step(RunStep::SupervisorPlan);
    m.complete_step(RunStep::SupervisorPlan);

    let json = m.to_json().unwrap();
    let restored = RunManifest::from_json(&json).unwrap();
    assert_eq!(m, restored);
    assert_eq!(restored.mode, ResearchMode::Supervisor);
}
