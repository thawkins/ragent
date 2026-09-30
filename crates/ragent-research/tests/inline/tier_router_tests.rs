//! Inline tests for `tier_router.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::run_config::Tier;

#[test]
fn router_defaults_to_full_tier() {
    let router = TierRouter::new("run-1", "glp1", "GLP-1 outcomes", Tier::default());
    assert_eq!(router.tier(), Tier::Full);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
    assert!(steps.contains(&RunStep::ContradictionGraph));
    assert!(steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn light_router_skips_adversarial_steps() {
    let router = TierRouter::new("run-1", "demo", "demo topic", Tier::Light);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
    assert!(steps.contains(&RunStep::Decompose));
    assert!(steps.contains(&RunStep::WidthSweep));
    assert!(steps.contains(&RunStep::Polish));
    assert!(!steps.contains(&RunStep::ContradictionGraph));
    assert!(!steps.contains(&RunStep::LociAnalysis));
    assert!(!steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn full_router_includes_all_steps() {
    let router = TierRouter::new("run-1", "demo", "demo topic", Tier::Full);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
    assert!(steps.contains(&RunStep::ContradictionGraph));
    assert!(steps.contains(&RunStep::Critics));
    assert!(steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn dissertation_router_starts_with_chapter_partition() {
    let router = TierRouter::new("run-1", "demo", "demo topic", Tier::Dissertation);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
    assert_eq!(steps[0], RunStep::ChapterPartition);
    assert!(steps.contains(&RunStep::Synthesize));
}

#[test]
fn walk_remaining_runs_steps_in_order() {
    let mut router = TierRouter::new("run-1", "demo", "demo topic", Tier::Light);
    let observer = CollectingTierRouterObserver::new();
    let steps_to_run: Vec<RunStep> = router.manifest().steps.iter().map(|s| s.step).collect();
    router
        .walk_remaining(&observer, |r, step, obs| {
            if steps_to_run.contains(&step) {
                r.finish_step(step, obs);
            }
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap();
    assert!(router.is_complete());
    let events = observer.events();
    let completed = events
        .iter()
        .filter(|(_, s, _)| *s == StepStatus::Completed)
        .count();
    assert_eq!(completed, RunStep::steps_for_tier(Tier::Light).len());
}

#[test]
fn skip_remaining_marks_pending_steps_skipped() {
    let mut router = TierRouter::new("run-1", "demo", "demo topic", Tier::Full);
    let observer = CollectingTierRouterObserver::new();
    router.start_step(RunStep::Decompose, &observer);
    router.finish_step(RunStep::Decompose, &observer);
    router.skip_remaining("early termination", &observer);
    let events = observer.events();
    let skipped = events
        .iter()
        .filter(|(_, s, _)| *s == StepStatus::Skipped)
        .count();
    assert_eq!(skipped, RunStep::steps_for_tier(Tier::Full).len() - 1);
}

#[test]
fn supervisor_router_uses_graph_steps() {
    let router = TierRouter::new_with_mode(
        "run-sv",
        "demo",
        "demo topic",
        Tier::Full,
        ResearchMode::Supervisor,
    );
    assert_eq!(router.mode(), ResearchMode::Supervisor);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
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
fn competitive_router_uses_graph_steps() {
    let router = TierRouter::new_with_mode(
        "run-comp",
        "demo",
        "demo topic",
        Tier::Full,
        ResearchMode::Competitive,
    );
    assert_eq!(router.mode(), ResearchMode::Competitive);
    let steps: Vec<_> = router.manifest().steps.iter().map(|s| s.step).collect();
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
fn supervisor_router_walks_graph_steps() {
    let mut router = TierRouter::new_with_mode(
        "run-sv",
        "demo",
        "demo topic",
        Tier::Full,
        ResearchMode::Supervisor,
    );
    let observer = CollectingTierRouterObserver::new();
    router
        .walk_remaining(&observer, |r, step, obs| {
            r.finish_step(step, obs);
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap();
    assert!(router.is_complete());
    let events = observer.events();
    let completed = events
        .iter()
        .filter(|(_, s, _)| *s == StepStatus::Completed)
        .count();
    assert_eq!(completed, 4);
}
