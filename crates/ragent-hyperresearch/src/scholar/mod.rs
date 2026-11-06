//! Scholarly providers, registry, and cross-provider deduplication.
//!
//! Requirements served here: FR-012 (provider registry), FR-013
//! (deduplication by DOI then normalised title plus year), FR-014 (open-access
//! resolution with recovery).

pub mod dedup;
pub mod open_access;
pub mod registry;
