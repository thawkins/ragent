//! Model-picker table-row cache (FR-003).
//!
//! The `SelectModel` picker dialog renders a table of available models with
//! columns for name, context window, cost tier, thinking levels, and feature
//! indicators.  Building the `Row` widgets from `ModelPickerEntry` data
//! requires multiple `format!()` calls per model per frame.
//!
//! This cache stores the pre-built `Vec<Row<'static>>` alongside a cheap
//! signature (model count + first model ID).  When the model list hasn't
//! changed since the last render, the cached rows are reused directly,
//! avoiding the per-frame `format!()` churn.

use ratatui::widgets::Row;

/// Cached model-picker table rows and the signature they were built from.
#[derive(Clone)]
pub struct ModelPickerRowsCache {
    /// Pre-built table rows for the model picker.
    pub rows: Vec<Row<'static>>,
    /// Number of models when the rows were built.
    pub model_count: usize,
    /// ID of the first model when the rows were built (cheap identity check).
    pub first_model_id: Option<String>,
}

impl ModelPickerRowsCache {
    /// Returns `true` when the cache is valid for the given model list.
    pub fn matches(&self, model_count: usize, first_model_id: Option<&str>) -> bool {
        self.model_count == model_count && self.first_model_id.as_deref() == first_model_id
    }
}
