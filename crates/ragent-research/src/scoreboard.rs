//! Corpus Quality Scoreboard display helpers for research documents.
//!
//! Implements the deterministic, LLM-free display primitives defined by the
//! `corpusAnalysis` spec (`specs/corpusAnalysis/SPEC.md`):
//!
//! - [`GradeBand`] and [`GradeBand::from_score`] - the FR-002 letter grade
//!   (A/B/C/D) plus its meaning word for an overall 0-100 quality score.
//! - [`render_meter_bar`] - the FR-003 proportional ASCII meter bar.
//!
//! All output is plain ASCII (FR-016): the bar uses only `#`, `-`, `[`, `]`,
//! spaces, and digits. These helpers are pure formatting; they never compute
//! or mutate any corpus quality score (FR-015).

/// Width, in cells, of the meter bar rendered by [`render_meter_bar`] (FR-003).
pub const METER_CELLS: usize = 20;

/// Letter grade for an overall corpus quality score (FR-002).
///
/// Bands align with the synthesis-audit thresholds (80/50) already used by
/// `render_data_quality_summary`:
///
/// - [`GradeBand::A`] - 80-100, Excellent - proceed
/// - [`GradeBand::B`] - 65-79, Good
/// - [`GradeBand::C`] - 50-64, Adequate - caution
/// - [`GradeBand::D`] - 0-49, Weak - revise
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradeBand {
    /// 80-100: Excellent - proceed.
    A,
    /// 65-79: Good.
    B,
    /// 50-64: Adequate - caution.
    C,
    /// 0-49: Weak - revise.
    D,
}

impl GradeBand {
    /// Returns the band for an overall 0-100 quality score (FR-002).
    ///
    /// Scores above 100 are treated as 100 so the helper stays total for any
    /// deserialized `CorpusCriticReport` or `SynthesisAudit` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_research::scoreboard::GradeBand;
    ///
    /// assert_eq!(GradeBand::from_score(100), GradeBand::A);
    /// assert_eq!(GradeBand::from_score(80), GradeBand::A);
    /// assert_eq!(GradeBand::from_score(79), GradeBand::B);
    /// assert_eq!(GradeBand::from_score(65), GradeBand::B);
    /// assert_eq!(GradeBand::from_score(64), GradeBand::C);
    /// assert_eq!(GradeBand::from_score(50), GradeBand::C);
    /// assert_eq!(GradeBand::from_score(49), GradeBand::D);
    /// assert_eq!(GradeBand::from_score(0), GradeBand::D);
    /// ```
    #[must_use]
    pub fn from_score(score: u32) -> Self {
        let clamped = score.min(100);
        if clamped >= 80 {
            Self::A
        } else if clamped >= 65 {
            Self::B
        } else if clamped >= 50 {
            Self::C
        } else {
            Self::D
        }
    }

    /// Returns the meaning word displayed next to the letter (FR-002):
    /// Excellent, Good, Adequate, or Weak.
    #[must_use]
    pub fn meaning(self) -> &'static str {
        match self {
            Self::A => "Excellent",
            Self::B => "Good",
            Self::C => "Adequate",
            Self::D => "Weak",
        }
    }
}

impl std::fmt::Display for GradeBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The grade letter is the band's Display form: "A".."D".
        let letter = match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        };
        f.write_str(letter)
    }
}

/// Renders the FR-003 proportional ASCII meter bar for a 0-100 score.
///
/// The bar is exactly [`METER_CELLS`] (20) cells wide between square
/// brackets, filled left-to-right with `#` and padded with `-`, followed by
/// two spaces and a `score/100` suffix. The filled-cell count is
/// `round(score * METER_CELLS / 100)`, matching the TC-003 verification
/// formula. Scores above 100 are clamped to 100. The output is pure ASCII
/// (FR-016).
///
/// # Examples
///
/// ```
/// use ragent_research::scoreboard::render_meter_bar;
///
/// assert_eq!(render_meter_bar(74), "[###############-----]  74/100");
/// assert_eq!(render_meter_bar(100), "[####################]  100/100");
/// assert_eq!(render_meter_bar(0), "[--------------------]  0/100");
/// ```
#[must_use]
pub fn render_meter_bar(score: u32) -> String {
    let clamped = score.min(100);
    let filled = (clamped as usize * METER_CELLS + 50) / 100;
    let empty = METER_CELLS - filled;
    format!(
        "[{}{}]  {clamped}/100",
        "#".repeat(filled),
        "-".repeat(empty)
    )
}
