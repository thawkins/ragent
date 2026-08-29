//! Integration tests for the scoreboard grade-band and meter-bar helpers
//! (spec `corpusAnalysis`, task T-001, requirements FR-002/FR-003/FR-016).

use ragent_research::scoreboard::{GradeBand, METER_CELLS, render_meter_bar};

#[test]
fn test_grade_band_from_score_boundaries() {
    assert_eq!(GradeBand::from_score(100), GradeBand::A);
    assert_eq!(GradeBand::from_score(80), GradeBand::A);
    assert_eq!(GradeBand::from_score(79), GradeBand::B);
    assert_eq!(GradeBand::from_score(65), GradeBand::B);
    assert_eq!(GradeBand::from_score(64), GradeBand::C);
    assert_eq!(GradeBand::from_score(50), GradeBand::C);
    assert_eq!(GradeBand::from_score(49), GradeBand::D);
    assert_eq!(GradeBand::from_score(0), GradeBand::D);
}

#[test]
fn test_grade_band_from_score_clamps_above_100() {
    assert_eq!(GradeBand::from_score(101), GradeBand::A);
    assert_eq!(GradeBand::from_score(u32::MAX), GradeBand::A);
}

#[test]
fn test_grade_band_display_and_meaning() {
    assert_eq!(GradeBand::A.to_string(), "A");
    assert_eq!(GradeBand::B.to_string(), "B");
    assert_eq!(GradeBand::C.to_string(), "C");
    assert_eq!(GradeBand::D.to_string(), "D");

    assert_eq!(GradeBand::A.meaning(), "Excellent");
    assert_eq!(GradeBand::B.meaning(), "Good");
    assert_eq!(GradeBand::C.meaning(), "Adequate");
    assert_eq!(GradeBand::D.meaning(), "Weak");
}

#[test]
fn test_meter_bar_renders_known_scores() {
    assert_eq!(render_meter_bar(74), "[###############-----]  74/100");
    assert_eq!(render_meter_bar(100), "[####################]  100/100");
    assert_eq!(render_meter_bar(50), "[##########----------]  50/100");
    assert_eq!(render_meter_bar(25), "[#####---------------]  25/100");
    assert_eq!(render_meter_bar(0), "[--------------------]  0/100");
}

#[test]
fn test_meter_bar_clamps_score_above_100() {
    assert_eq!(render_meter_bar(101), "[####################]  100/100");
    assert_eq!(
        render_meter_bar(u32::MAX),
        "[####################]  100/100"
    );
}

#[test]
fn test_meter_bar_width_and_ascii_only() {
    for score in [0u32, 1, 25, 33, 49, 50, 64, 65, 74, 79, 80, 99, 100, 101] {
        let bar = render_meter_bar(score);
        assert!(bar.starts_with('['), "score {score}: missing open bracket");
        assert!(
            bar.as_bytes().get(1 + METER_CELLS) == Some(&b']'),
            "score {score}: bar not exactly {METER_CELLS} cells wide: {bar}"
        );
        let cells = &bar[1..=METER_CELLS];
        assert!(
            cells.chars().all(|c| c == '#' || c == '-'),
            "score {score}: non-cell characters in bar: {cells}"
        );
        assert!(
            cells
                .find('-')
                .map_or(true, |i| cells[..i].chars().all(|c| c == '#')),
            "score {score}: dashes before hashes: {cells}"
        );
        assert!(bar.is_ascii(), "score {score}: non-ASCII output: {bar}");
    }
}

#[test]
fn test_meter_bar_proportionality_matches_tc003_formula() {
    // TC-003: filled cells must equal round(N * 20 / 100), clamped to 0-20.
    for score in 0..=100u32 {
        let expected_filled = ((score as usize * METER_CELLS + 50) / 100).min(METER_CELLS);
        let bar = render_meter_bar(score);
        let cells = &bar[1..=METER_CELLS];
        let filled = cells.matches('#').count();
        assert_eq!(filled, expected_filled, "score {score}: {bar}");
        assert_eq!(
            cells.matches('-').count(),
            METER_CELLS - expected_filled,
            "score {score}: {bar}"
        );
        assert!(
            bar.ends_with(&format!("  {score}/100")),
            "score {score}: {bar}"
        );
    }
}
