//! Integration tests for the `ragent-tools-core` `DateTimeTool`.
//!
//! `DateTimeTool` is public, so no `#[path]` re-import is needed. The
//! assertions are relative (the reported instant lies between two readings of
//! the same clock) so they hold whatever the wall clock says when they run.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use chrono::Utc;
use ragent_tools_core::datetime::DateTimeTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn make_ctx() -> ToolContext {
    let dir = PathBuf::from("target/temp");
    ToolContext {
        session_id: "datetime-test".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

/// Run the tool with `input` and return its metadata.
async fn run(input: serde_json::Value) -> serde_json::Value {
    DateTimeTool
        .execute(input, &make_ctx())
        .await
        .expect("DateTime execute")
        .metadata
        .expect("DateTime metadata")
}

#[tokio::test]
async fn test_default_offset_is_utc_and_instant_is_now() {
    let before = Utc::now().timestamp();
    let metadata = run(json!({})).await;
    let after = Utc::now().timestamp();

    assert_eq!(metadata["timezone_offset_minutes"], 0);
    assert_eq!(metadata["timezone_offset"], "UTC+00:00");

    let seconds = metadata["unix_seconds"]
        .as_i64()
        .expect("unix_seconds is an integer");
    assert!(
        seconds >= before && seconds <= after,
        "reported {seconds} outside the [{before}, {after}] bracket"
    );
    assert_eq!(
        metadata["date"],
        metadata["local"]
            .as_str()
            .map_or(String::new(), |iso| iso[..10].to_string())
    );
    assert!(
        metadata["utc"].as_str().is_some_and(|s| s.ends_with('Z')),
        "RFC 3339 UTC rendering must end in Z: {}",
        metadata["utc"]
    );
}

#[tokio::test]
async fn test_offset_shifts_the_local_rendering() {
    let utc = run(json!({})).await;
    let plus_five_thirty = run(json!({ "timezone_offset_minutes": 330 })).await;

    assert_eq!(plus_five_thirty["timezone_offset"], "UTC+05:30");
    assert_eq!(plus_five_thirty["timezone_offset_minutes"], 330);

    let shift = plus_five_thirty["unix_seconds"]
        .as_i64()
        .expect("unix_seconds")
        - utc["unix_seconds"].as_i64().expect("unix_seconds");
    // Two separate clock readings, so allow the second boundary to move.
    assert!(
        (-1..=1).contains(&shift),
        "the same instant must be reported under both offsets, shifted by at most a second: {shift}"
    );

    let to_seconds = |metadata: &serde_json::Value| -> i64 {
        let date = metadata["date"].as_str().expect("date");
        let time = metadata["time"].as_str().expect("time");
        chrono::NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M:%S")
            .expect("date and time parse")
            .and_utc()
            .timestamp()
    };
    let delta = to_seconds(&plus_five_thirty) - to_seconds(&utc);
    assert!(
        (330 * 60 - 1..=330 * 60 + 1).contains(&delta),
        "local rendering at UTC+05:30 should be 330 minutes ahead, got {delta}s"
    );
}

#[tokio::test]
async fn test_negative_offset_renders_with_a_minus_sign() {
    let metadata = run(json!({ "timezone_offset_minutes": -300 })).await;
    assert_eq!(metadata["timezone_offset"], "UTC-05:00");
    assert_eq!(metadata["timezone_offset_minutes"], -300);
}

#[tokio::test]
async fn test_custom_format_is_echoed_in_metadata() {
    let metadata = run(json!({
        "timezone_offset_minutes": 0,
        "format": "%Y/%m/%d",
    }))
    .await;

    let formatted = metadata["formatted"].as_str().expect("formatted");
    let date = metadata["date"].as_str().expect("date");
    assert_eq!(formatted, date.replace('-', "/"));
}

#[tokio::test]
async fn test_field_metadata_matches_the_local_rendering() {
    let metadata = run(json!({ "timezone_offset_minutes": 120 })).await;

    let date = metadata["date"].as_str().expect("date");
    let time = metadata["time"].as_str().expect("time");
    assert_eq!(date.len(), 10, "date is YYYY-MM-DD: {date}");
    assert_eq!(time.len(), 8, "time is HH:MM:SS: {time}");
    assert_eq!(
        date,
        format!(
            "{}-{}-{}",
            metadata["year"].as_str().expect("year"),
            metadata["month"].as_str().expect("month"),
            metadata["day"].as_str().expect("day"),
        )
    );
    assert!(
        metadata["day_of_week"]
            .as_str()
            .is_some_and(|d| !d.is_empty()),
        "day_of_week must be named"
    );
}

#[tokio::test]
async fn test_out_of_range_offset_is_rejected() {
    let err = DateTimeTool
        .execute(json!({ "timezone_offset_minutes": 100_000 }), &make_ctx())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("must be between -1080 and 1080"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_non_integer_offset_is_rejected() {
    let err = DateTimeTool
        .execute(json!({ "timezone_offset_minutes": "noon" }), &make_ctx())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("must be an integer number of minutes"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn test_permission_category_is_none() {
    // The tool reads the clock only; a permission prompt would be friction
    // with no security value.
    assert_eq!(DateTimeTool.permission_category(), "none");
}
