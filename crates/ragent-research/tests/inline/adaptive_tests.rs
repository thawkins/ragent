//! Inline tests for `adaptive.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn stopper_continues_within_budget() {
    let mut s = AdaptiveStopper::new(3);
    assert_eq!(s.decide(1, Some(50), false), StopDecision::Continue);
}

#[test]
fn stopper_halts_at_max_iterations() {
    let mut s = AdaptiveStopper::new(2);
    s.decide(1, Some(50), false);
    s.decide(2, Some(60), false);
    assert_eq!(s.decide(3, Some(60), false), StopDecision::MaxIterations);
}

#[test]
fn stopper_halts_on_no_improvement() {
    let mut s = AdaptiveStopper::new(5);
    s.decide(1, Some(50), false);
    s.decide(2, Some(60), false);
    s.decide(3, Some(60), false);
    assert_eq!(s.decide(4, Some(60), false), StopDecision::NoImprovement);
}

#[test]
fn stopper_continues_when_improving() {
    let mut s = AdaptiveStopper::new(5);
    s.decide(1, Some(50), false);
    s.decide(2, Some(60), false);
    assert_eq!(s.decide(3, Some(70), false), StopDecision::Continue);
}

#[test]
fn stopper_respects_force_deeper() {
    let mut s = AdaptiveStopper::new(5).with_force_deeper(true);
    s.decide(1, Some(50), false);
    s.decide(2, Some(60), false);
    assert_eq!(s.decide(3, Some(60), false), StopDecision::Continue);
}

#[test]
fn stopper_stops_on_complete() {
    let mut s = AdaptiveStopper::new(10);
    assert_eq!(s.decide(1, None, true), StopDecision::Complete);
}
