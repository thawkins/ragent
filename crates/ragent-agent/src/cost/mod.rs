//! Per-run cost estimation for agent sessions (T-010).
//!
//! This module provides a lightweight, provider-agnostic price table and a
//! pure function to compute a [`RunCostSummary`] from a sequence of usage
//! records. Built-in prices are supplied for the four major provider families
//! covered by the `openharness` spec: Anthropic, OpenAI, Google Gemini, and
//! Ollama (local, zero cost). Unknown models contribute zero cost rather than
//! failing, which keeps cost tracking informational rather than load-bearing.
//!
//! Prices are stored **per 1M tokens** in USD, matching the `ragent_config::Cost`
//! convention, and converted to per-token rates inside `compute_run_cost`.
//!
//! # Usage
//!
//! ```
//! use ragent_agent::cost::{builtin_prices, compute_run_cost, RunCostSummary, UsageRecord};
//!
//! let prices = builtin_prices();
//! let summary = compute_run_cost(
//!     vec![
//!         UsageRecord { model_id: "gpt-4o".into(), input_tokens: 1_000_000, output_tokens: 500_000 },
//!     ],
//!     &prices,
//! );
//! assert_eq!(summary.total_input_tokens, 1_000_000);
//! assert_eq!(summary.total_output_tokens, 500_000);
//! // $2.50 input + $5.00 output
//! assert!((summary.total_cost_usd - 7.50).abs() < 0.001);
//! ```

use std::collections::HashMap;

/// One row of token usage for a single model.
///
/// The `cost` module intentionally keeps this type flat so callers can
/// accumulate usage from any source (`Event::TokenUsage`, `StreamEvent::Usage`,
/// telemetry records, etc.) without taking a dependency on the full event
/// hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    /// Model identifier as returned by the provider (e.g. `"gpt-4o"`).
    pub model_id: String,
    /// Input/prompt tokens consumed.
    pub input_tokens: u64,
    /// Output/completion tokens produced.
    pub output_tokens: u64,
}

/// Estimated cost summary for a completed agent run.
///
/// This is the value published by `Event::RunCostSummary` and surfaced in the
/// TUI run-complete banner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunCostSummary {
    /// Total input/prompt tokens across all usage records.
    pub total_input_tokens: u64,
    /// Total output/completion tokens across all usage records.
    pub total_output_tokens: u64,
    /// Estimated total cost in USD.
    pub total_cost_usd: f64,
}

impl Default for RunCostSummary {
    fn default() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
        }
    }
}

/// Price table mapping model identifiers to `(input_usd_per_1m, output_usd_per_1m)`.
///
/// Keys use `String` so user-supplied overrides from `ragent.json` can be
/// inserted without leaking. Built-in entries are inserted from `&'static str`
/// literals which convert to `String` without allocation overhead concerns.
/// Values are in **USD per 1,000,000 tokens**, the same unit used
/// in `ragent_config::Cost`.
pub type PriceTable = HashMap<String, (f64, f64)>;

/// Returns the built-in price table used when no user overrides are configured.
///
/// Prices are approximate published rates as of mid-2025. They are intended for
/// spend estimation, not billing. Ollama is priced at `(0.0, 0.0)` because it
/// runs locally.
#[must_use]
pub fn builtin_prices() -> PriceTable {
    let mut table = PriceTable::with_capacity(16);

    // Anthropic — approximate rates per 1M tokens.
    table.insert("claude-sonnet-4-20250514".to_string(), (3.0, 15.0));
    table.insert("claude-sonnet-4-20250601".to_string(), (3.0, 15.0));
    table.insert("claude-opus-4-20250514".to_string(), (15.0, 75.0));
    table.insert("claude-opus-4-20250601".to_string(), (15.0, 75.0));
    table.insert("claude-haiku-4-20250514".to_string(), (0.25, 1.25));

    // OpenAI — approximate rates per 1M tokens.
    table.insert("gpt-4o".to_string(), (2.50, 10.0));
    table.insert("gpt-4o-mini".to_string(), (0.15, 0.60));
    table.insert("gpt-4.1".to_string(), (2.0, 8.0));
    table.insert("gpt-4.1-mini".to_string(), (0.40, 1.60));
    table.insert("o3".to_string(), (10.0, 40.0));
    table.insert("o4-mini".to_string(), (1.10, 4.40));
    table.insert("o3-mini".to_string(), (1.10, 4.40));

    // Google Gemini — approximate rates per 1M tokens.
    table.insert("gemini-2.5-flash-preview-05-20".to_string(), (0.15, 0.60));
    table.insert("gemini-2.5-pro-preview-05-06".to_string(), (1.25, 10.0));
    table.insert("gemini-2.0-flash".to_string(), (0.10, 0.40));
    table.insert("gemini-2.0-flash-lite".to_string(), (0.075, 0.30));
    table.insert("gemini-1.5-flash".to_string(), (0.075, 0.30));
    table.insert("gemini-1.5-pro".to_string(), (1.25, 5.0));

    // Ollama — local inference, no hosted cost.
    table.insert("ollama".to_string(), (0.0, 0.0));

    table
}

/// Build a merged price table from the built-in entries and user-supplied
/// overrides from `ragent.json` (FR-011).
///
/// Each `PriceEntry` whose `model` field matches a built-in key replaces that
/// entry. Entries for models not in the built-in table are inserted as new
/// keys. This lets operators correct stale prices or add custom models without
/// modifying source code.
#[must_use]
pub fn merged_prices(overrides: &[ragent_config::PriceEntry]) -> PriceTable {
    let mut table = builtin_prices();
    for entry in overrides {
        table.insert(
            entry.model.clone(),
            (entry.input_per_1m, entry.output_per_1m),
        );
    }
    table
}

/// Computes a [`RunCostSummary`] from a sequence of [`UsageRecord`]s and a
/// [`PriceTable`].
///
/// Models not present in `prices` contribute zero cost but still count toward
/// token totals. This is intentional: cost estimation should never block a
/// run because a provider returned an unfamiliar model identifier.
///
/// # Type parameters
///
/// * `I` — any iterator yielding `UsageRecord` (e.g. `Vec`, slice, `Iterator`).
///
/// # Examples
///
/// ```
/// use ragent_agent::cost::{builtin_prices, compute_run_cost, UsageRecord};
///
/// let summary = compute_run_cost(
///     vec![UsageRecord {
///         model_id: "claude-sonnet-4-20250514".into(),
///         input_tokens: 2_000_000,
///         output_tokens: 1_000_000,
///     }],
///     &builtin_prices(),
/// );
/// assert_eq!(summary.total_cost_usd, 21.0); // $6 input + $15 output
/// ```
#[must_use]
pub fn compute_run_cost<I>(usage_log: I, prices: &PriceTable) -> RunCostSummary
where
    I: IntoIterator<Item = UsageRecord>,
{
    let mut summary = RunCostSummary::default();

    for record in usage_log {
        summary.total_input_tokens += record.input_tokens;
        summary.total_output_tokens += record.output_tokens;

        if let Some((input_per_1m, output_per_1m)) = prices.get(record.model_id.as_str()) {
            let input_rate = input_per_1m / 1_000_000.0;
            let output_rate = output_per_1m / 1_000_000.0;
            summary.total_cost_usd += (record.output_tokens as f64)
                .mul_add(output_rate, (record.input_tokens as f64) * input_rate);
        }
    }

    // Guard against negative zero from floating-point arithmetic.
    if summary.total_cost_usd.abs() < f64::EPSILON {
        summary.total_cost_usd = 0.0;
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_usage_log_returns_zero_summary() {
        let summary = compute_run_cost(Vec::<UsageRecord>::new(), &builtin_prices());
        assert_eq!(summary, RunCostSummary::default());
    }

    #[test]
    fn test_known_model_computes_expected_cost() {
        let summary = compute_run_cost(
            vec![UsageRecord {
                model_id: "gpt-4o".into(),
                input_tokens: 1_000_000,
                output_tokens: 500_000,
            }],
            &builtin_prices(),
        );
        assert_eq!(summary.total_input_tokens, 1_000_000);
        assert_eq!(summary.total_output_tokens, 500_000);
        // $2.50 + $5.00
        assert!((summary.total_cost_usd - 7.50).abs() < 0.0001);
    }

    #[test]
    fn test_multiple_records_accumulate() {
        let summary = compute_run_cost(
            vec![
                UsageRecord {
                    model_id: "gemini-2.0-flash".into(),
                    input_tokens: 2_000_000,
                    output_tokens: 1_000_000,
                },
                UsageRecord {
                    model_id: "gpt-4o-mini".into(),
                    input_tokens: 1_000_000,
                    output_tokens: 1_000_000,
                },
            ],
            &builtin_prices(),
        );
        assert_eq!(summary.total_input_tokens, 3_000_000);
        assert_eq!(summary.total_output_tokens, 2_000_000);
        // Gemini: 2M * $0.10 + 1M * $0.40 = $0.60
        // OpenAI: 1M * $0.15 + 1M * $0.60 = $0.75
        assert!((summary.total_cost_usd - 1.35).abs() < 0.0001);
    }

    #[test]
    fn test_unknown_model_counts_tokens_with_zero_cost() {
        let summary = compute_run_cost(
            vec![UsageRecord {
                model_id: "unknown-model-xyz".into(),
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
            }],
            &builtin_prices(),
        );
        assert_eq!(summary.total_input_tokens, 1_000_000);
        assert_eq!(summary.total_output_tokens, 1_000_000);
        assert!(summary.total_cost_usd.abs() < f64::EPSILON);
    }

    #[test]
    fn test_ollama_is_zero_cost() {
        let summary = compute_run_cost(
            vec![UsageRecord {
                model_id: "ollama".into(),
                input_tokens: 10_000_000,
                output_tokens: 10_000_000,
            }],
            &builtin_prices(),
        );
        assert_eq!(summary.total_input_tokens, 10_000_000);
        assert_eq!(summary.total_output_tokens, 10_000_000);
        assert!(summary.total_cost_usd.abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_price_table_overrides_builtin() {
        let mut prices = PriceTable::new();
        prices.insert("custom-model".to_string(), (1.0, 4.0));

        let summary = compute_run_cost(
            vec![UsageRecord {
                model_id: "custom-model".into(),
                input_tokens: 2_000_000,
                output_tokens: 500_000,
            }],
            &prices,
        );
        assert!((summary.total_cost_usd - 4.0).abs() < 0.0001);
    }
}
