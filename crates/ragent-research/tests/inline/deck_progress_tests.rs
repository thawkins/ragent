//! Inline tests for [`crate::deck_progress`] (spec `diffdeck` FR-031).

use super::*;

#[test]
fn test_deck_stage_run_order_is_ordinal() {
    for (index, stage) in DeckStage::ALL.iter().enumerate() {
        assert_eq!(stage.ordinal(), index + 1, "{:?}", stage.label());
    }
}

#[test]
fn test_deck_stage_labels_are_distinct() {
    let mut labels: Vec<&str> = DeckStage::ALL.iter().map(|s| s.label()).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(labels.len(), before, "stage labels must be distinct");
}

#[test]
fn test_started_event_carries_no_unit() {
    let event = DeckEvent::started(DeckStage::Select, "looking up");
    assert_eq!(event.status, DeckStepStatus::Started);
    assert_eq!(event.unit(), None);
    assert_eq!(event.completed, 0);
}

#[test]
fn test_progress_event_reports_unit() {
    let event = DeckEvent::progress(DeckStage::EnrichFindings, 2, 5, "finding two");
    assert_eq!(event.unit(), Some((2, 5)));
    assert_eq!(event.status, DeckStepStatus::Started);
}

#[test]
fn test_skipped_event_status() {
    let event = DeckEvent::skipped(DeckStage::Export, "no sink");
    assert_eq!(event.status, DeckStepStatus::Skipped);
    assert_eq!(event.status.as_str(), "skipped");
    assert_eq!(event.unit(), None);
}

#[test]
fn test_report_without_sink_is_a_no_op() {
    // The orchestration must be able to run with no surface at all; `report`
    // taking an `Option` is what makes every call site free of an `if let`.
    report(None, DeckEvent::started(DeckStage::Render, "rendering"));
    report(
        Some(&NoopDeckProgress),
        DeckEvent::started(DeckStage::Render, "rendering"),
    );
}

#[test]
fn test_report_forwards_to_sink() {
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        seen: Mutex<Vec<DeckEvent>>,
    }
    impl DeckProgress for Recorder {
        fn report(&self, event: &DeckEvent) {
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(event.clone());
            }
        }
    }

    let recorder = Recorder::default();
    report(
        Some(&recorder),
        DeckEvent::done(DeckStage::Diff, "3 elements"),
    );
    let seen = recorder.seen.lock().expect("recorder lock");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].stage, DeckStage::Diff);
    assert_eq!(seen[0].status, DeckStepStatus::Done);
}

#[test]
fn test_element_label_falls_back_to_deck() {
    assert_eq!(element_label(None), "deck");
    assert_eq!(
        element_label(Some(crate::deck::DeckElement::Findings)),
        "Findings"
    );
}
