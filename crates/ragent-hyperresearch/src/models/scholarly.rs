//! Shared scholarly-record model (FR-012) and its evidence attributes (FR-017).

use serde::{Deserialize, Serialize};

/// Study design, weakest evidence first; used by the search evidence filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyDesign {
    /// Anecdote, editorial, or other non-study material.
    Other,
    /// Case report.
    CaseReport,
    /// Qualitative study.
    Qualitative,
    /// Cross-sectional study.
    CrossSectional,
    /// Cohort study.
    Cohort,
    /// Case-control study.
    CaseControl,
    /// Randomised controlled trial.
    Rct,
    /// Meta-analysis or systematic review.
    MetaAnalysis,
}

impl StudyDesign {
    /// Default source-quality score in `0..=100` for this design.
    pub fn source_quality(self) -> u8 {
        match self {
            Self::Other => 10,
            Self::CaseReport => 30,
            Self::Qualitative => 35,
            Self::CrossSectional => 50,
            Self::Cohort => 65,
            Self::CaseControl => 60,
            Self::Rct => 90,
            Self::MetaAnalysis => 95,
        }
    }
}

/// Outcome of open-access resolution for one record (FR-014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// An open-access copy was found directly.
    Resolved,
    /// An alternate open-access source supplied the copy.
    AlternateSource,
    /// Fell back to the publisher landing page.
    LandingPage,
    /// No open-access copy could be found.
    NotFound,
}

/// One normalised scholarly record, however many providers contributed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScholarlyRecord {
    /// DOI, when the record has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    /// Title, already normalised for comparison.
    pub title: String,
    /// Publication year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Abstract or summary, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,
    /// Landing page URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Resolved open-access URL, once resolution has run (FR-014).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_access_url: Option<String>,
    /// Study design, when the record states one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_design: Option<StudyDesign>,
    /// Every provider that contributed this record (FR-013).
    #[serde(default)]
    pub provenance: Vec<String>,
    /// Resolution outcome reported per record without failing the batch (FR-014).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
}
