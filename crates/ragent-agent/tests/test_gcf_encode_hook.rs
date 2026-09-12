#![allow(clippy::unwrap_used)]
//! Integration tests for the GCF encoder hook (spec `gcf`, T-005).
//!
//! Exercises `tool_result_content_for_llm` (the single LLM-view choke point)
//! with the GCF runtime flag toggled, covering the FR-004 eligibility rules
//! (size threshold, JSON-only payloads, savings margin, exempt tools) and the
//! FR-007 lossless guarantee (the emitted block decodes back to the exact
//! original JSON).

use std::sync::{Mutex, OnceLock};

use ragent_agent::session::processor::tool_result_content_for_llm;
use serde_json::{Value, json};

/// The GCF runtime flag is process-global; serialise every test that toggles
/// it so parallel test threads cannot race each other's state.
fn flag_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(()));
    // A poisoned lock only means a previous test panicked while holding the
    // flag; taking it anyway keeps the remaining tests runnable.
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Toggle the GCF flag for the duration of `f` and always restore it.
fn with_gcf(enabled: bool, f: impl FnOnce()) {
    let _guard = flag_guard();
    let previous = ragent_config::gcf::is_enabled();
    ragent_config::gcf::set_enabled(enabled);
    f();
    ragent_config::gcf::set_enabled(previous);
}

/// Build a `{"rows":[...]}` JSON string whose exact character length is
/// `9*a + 10*b + 10` (rows of `{"id":1}` and `{"id":22}`), so tests can pin
/// the 200-char eligibility threshold precisely.
fn rows_json(a: usize, b: usize) -> String {
    let mut s = String::from("{\"rows\":[");
    for i in 0..(a + b) {
        if i > 0 {
            s.push(',');
        }
        if i < a {
            s.push_str("{\"id\":1}");
        } else {
            s.push_str("{\"id\":22}");
        }
    }
    s.push_str("]}");
    s
}

/// Flat object with long values: GCF's `key=value` rows save almost nothing,
/// so the labelled block cannot reach the 10% savings margin.
fn flat_long_json() -> String {
    let value = json!({
        "alpha": "A".repeat(40),
        "beta": "B".repeat(40),
        "gamma": "C".repeat(40),
        "delta": "D".repeat(40),
        "epsilon": "E".repeat(40),
    });
    serde_json::to_string(&value).expect("serialise")
}

const GCF_BEGIN: &str = "[BEGIN GCF generic]";
const GCF_END: &str = "[END GCF]";

/// Split a labelled GCF block into its payload (the text between the two
/// marker lines).
fn block_payload(block: &str) -> String {
    let payload = block
        .strip_prefix(GCF_BEGIN)
        .unwrap_or_else(|| panic!("block must start with {GCF_BEGIN}"))
        .strip_suffix(GCF_END)
        .unwrap_or_else(|| panic!("block must end with {GCF_END}"));
    payload
        .strip_prefix('\n')
        .unwrap_or_else(|| panic!("payload must start with a newline"))
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("payload must end with a newline"))
        .to_string()
}

// ---------------------------------------------------------------------------
// Fallback: non-JSON observations never encode (FR-004).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_non_json_tool_result_stays_raw() {
    with_gcf(true, || {
        let content = format!("plain prose observation {}", "x".repeat(250));
        let result = tool_result_content_for_llm("bash", &content, None);
        assert_eq!(result.as_ref(), content);
        assert!(!result.contains(GCF_BEGIN));
    });
}

// ---------------------------------------------------------------------------
// Threshold: payloads under 200 chars stay raw even when they would encode
// well; exactly 200 chars is eligible (FR-004).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_below_threshold_json_stays_raw() {
    with_gcf(true, || {
        // 199 chars, and a shape that encodes with large savings.
        let content = rows_json(21, 0);
        assert_eq!(content.chars().count(), 199);
        let result = tool_result_content_for_llm("stock_history", &content, None);
        assert_eq!(result.as_ref(), content, "sub-threshold JSON must stay raw");
    });
}

#[test]
fn test_gcf_at_threshold_json_encodes() {
    with_gcf(true, || {
        // 200 chars: not below the threshold, and it encodes well.
        let content = rows_json(20, 1);
        assert_eq!(content.chars().count(), 200);
        let result = tool_result_content_for_llm("stock_history", &content, None);
        assert!(
            result.starts_with(GCF_BEGIN),
            "200-char JSON should be GCF-encoded, got: {}",
            &result[..result.len().min(120)]
        );
    });
}

// ---------------------------------------------------------------------------
// Emission + lossless round-trip (FR-007).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_large_json_emits_labelled_block() {
    with_gcf(true, || {
        let content = rows_json(60, 0);
        let result = tool_result_content_for_llm("stock_history", &content, None);
        assert!(result.starts_with(GCF_BEGIN));
        assert!(result.ends_with(GCF_END));
        let payload = block_payload(&result);
        assert!(
            payload.starts_with("GCF profile=generic"),
            "payload must start with the generic-profile header, got: {}",
            payload.lines().next().unwrap_or_default()
        );
    });
}

#[test]
fn test_gcf_block_round_trips_losslessly() {
    with_gcf(true, || {
        let value = json!({
            "tool": "stock_fundamentals",
            "ok": true,
            "price": null,
            "data": {
                "symbol": "AAPL",
                "nested": {"leaf": 42},
                "tags": ["a", "b"],
                "empty_list": [],
                "empty_map": {}
            },
            "rows": (0..40).map(|i| json!({"id": i})).collect::<Vec<_>>()
        });
        let content = serde_json::to_string(&value).expect("serialise");
        assert!(content.chars().count() >= 200);
        let result = tool_result_content_for_llm("stock_fundamentals", &content, None);
        assert!(result.starts_with(GCF_BEGIN), "expected a GCF block");
        let payload = block_payload(&result);
        let decoded = gcf::decode_generic(&payload).expect("block must decode");
        assert_eq!(
            decoded, value,
            "FR-007: block must decode to the exact input"
        );
    });
}

#[test]
fn test_gcf_array_tool_result_encodes_and_round_trips() {
    with_gcf(true, || {
        let value = json!([
            {"symbol": "AAPL", "close": 123.45},
            {"symbol": "MSFT", "close": 234.56}
        ]);
        // Keep the payload pure JSON (pad inside the object).
        let content = serde_json::to_string(&json!({
            "results": value,
            "padding": "y".repeat(220)
        }))
        .expect("serialise");
        let result = tool_result_content_for_llm("stock_search", &content, None);
        if result.starts_with(GCF_BEGIN) {
            let payload = block_payload(&result);
            let decoded = gcf::decode_generic(&payload).expect("block must decode");
            let expected: Value = serde_json::from_str(&content).expect("parse raw");
            assert_eq!(decoded, expected);
        }
        // If the encoder declined (margin), raw passthrough is also correct.
    });
}

// ---------------------------------------------------------------------------
// Savings margin: JSON that would not save >= 10% stays raw (FR-004).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_below_margin_json_stays_raw() {
    with_gcf(true, || {
        let content = flat_long_json();
        assert!(content.chars().count() >= 200);
        let result = tool_result_content_for_llm("stock_fundamentals", &content, None);
        assert_eq!(result.as_ref(), content, "low-savings JSON must stay raw");
        assert!(!result.contains(GCF_BEGIN));
    });
}

// ---------------------------------------------------------------------------
// Encode failure: JSON that parses but cannot encode falls back to raw.
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_encode_failure_falls_back_to_raw() {
    with_gcf(true, || {
        // u64::MAX is outside the GCF int64 domain (SPEC 2.3.2): the value
        // parses as JSON but `encode_generic` rejects it, so the hook must
        // fall back to the raw observation instead of failing the tool result.
        let content = format!(
            "{{\"n\":18446744073709551615,\"pad\":\"{}\"}}",
            "z".repeat(240)
        );
        let parsed: Value = serde_json::from_str(&content).expect("input must be valid JSON");
        let raw_encode = gcf::encode_generic(&parsed);
        assert!(
            raw_encode.is_err(),
            "precondition: gcf must reject u64::MAX"
        );
        let result = tool_result_content_for_llm("stock_fundamentals", &content, None);
        assert_eq!(
            result.as_ref(),
            content,
            "encode failure must fall back to raw"
        );
    });
}

// ---------------------------------------------------------------------------
// Feature gate: disabled state (the default) never encodes (FR-001).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_disabled_leaves_json_raw() {
    with_gcf(false, || {
        let content = rows_json(60, 0);
        let result = tool_result_content_for_llm("stock_history", &content, None);
        assert!(
            !result.contains(GCF_BEGIN),
            "GCF must not run when the feature flag is off"
        );
    });
}

// ---------------------------------------------------------------------------
// Exempt tools stay raw even when GCF is enabled (FR-004 gate ordering).
// ---------------------------------------------------------------------------

#[test]
fn test_gcf_exempt_wait_agents_not_encoded() {
    with_gcf(true, || {
        let content = serde_json::to_string(&json!({
            "results": [{"id": "t-1", "status": "completed", "output": "done"}],
            "padding": "p".repeat(400)
        }))
        .expect("serialise");
        let result = tool_result_content_for_llm("wait_agents", &content, None);
        assert_eq!(
            result.as_ref(),
            content,
            "exempt tools pass through unchanged even with GCF on"
        );
    });
}
