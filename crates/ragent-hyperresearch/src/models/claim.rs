//! Claim model and verification state (FR-018).

use serde::{Deserialize, Serialize};

/// Verification state of a claim against the current vault.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    /// Not yet checked.
    #[default]
    Unverified,
    /// The supporting note and source resolve.
    Supported,
    /// The supporting note exists but its source no longer resolves.
    StaleSource,
    /// The supporting note cited by the claim does not exist.
    Dangling,
}

/// One claim extracted from a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// Identity of the claim within its note.
    pub id: String,
    /// The claim text.
    pub text: String,
    /// Note id that supports the claim.
    pub note_id: String,
    /// Source URL recorded for the supporting note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Current verification state.
    #[serde(default)]
    pub state: VerificationState,
}
