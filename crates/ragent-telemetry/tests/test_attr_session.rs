//! FR-025 `session.id` metric attribute helper (ANTIPAT M5.2, MEDIUM-6).
//!
//! `InstrumentRegistry::attr_session` is the sanctioned source of the dynamic
//! `session.id` metric attribute. It is marked `#[doc(hidden)]` because no
//! production recorder applies it yet - the session-scoped recorders do not
//! receive a session id, and the call sites that hold one live in the
//! `ragent-agent` crate - so wiring it through would require a cross-crate API
//! change outside this crate's cleanup scope.
//!
//! These tests pin the attribute's contract so the follow-up wiring has a
//! stable behaviour to build on:
//!
//! * the key is exactly `session.id` (so exporters and dashboards agree);
//! * ordinary session ids pass through unchanged;
//! * a sensitive-looking value is redacted (FR-034).

#![cfg(feature = "telemetry")]

use ragent_telemetry::InstrumentRegistry;
use ragent_telemetry::sensitive::REDACTED;

/// The attribute key is the canonical `session.id`, matching the otel spec.
#[test]
fn test_attr_session_key_is_session_id() {
    let kv = InstrumentRegistry::attr_session("sess-abc-123");
    assert_eq!(kv.key.as_str(), "session.id");
}

/// A normal uuid-shaped session id passes through unchanged.
#[test]
fn test_attr_session_passes_plain_id() {
    let kv = InstrumentRegistry::attr_session("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(kv.key.as_str(), "session.id");
    assert_eq!(kv.value.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}

/// A multi-line value is redacted rather than exported as a session id
/// (FR-034 sensitive-data guard).
#[test]
fn test_attr_session_redacts_multiline_value() {
    let kv = InstrumentRegistry::attr_session("line1\nline2\nline3");
    assert_eq!(kv.key.as_str(), "session.id");
    assert_eq!(kv.value.as_str(), REDACTED);
}
