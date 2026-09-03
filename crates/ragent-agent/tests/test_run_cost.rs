#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/cost/mod.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::cost::*;

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
