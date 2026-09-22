//! Tests for the opt-in `/plugins stores --check` availability probe (spec
//! `pluginstores` FR-031 `--check`).
//!
//! The probe contacts each store's effective endpoint through the injectable
//! [`StoreIndexFetcher`] seam, so these tests are deterministic and offline: the
//! `available` cases use the compiled-default [`FixtureStoreFetcher`] (which
//! serves an in-memory index for both defaults) and the `unavailable` cases use
//! a fetcher with no registered endpoint, so no live store is ever contacted
//! (NFR-003).
//!
//! The plain `/plugins stores` report is asserted to carry **no** probe suffix,
//! so the default output is unchanged.

use ragent_config::PluginStoresConfig;
use ragent_plugins::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, FixtureStoreFetcher, StoreProbe,
    probe_stores, render_stores_report, render_stores_report_with_probes, stores_check_requested,
};

/// A fixture fetcher that serves an index for both compiled default endpoints.
fn available_fetcher() -> FixtureStoreFetcher {
    FixtureStoreFetcher::new().with_default_endpoints()
}

// ── Flag parsing ────────────────────────────────────────────────────────────

#[test]
fn check_flag_is_recognised_anywhere_in_the_argument_text() {
    for args in ["--check", "  --check  ", "extra --check", "--check extra"] {
        assert!(
            stores_check_requested(args),
            "`{args}` must request a check"
        );
    }
    for args in ["", "check", "--refresh", "--checked", "stores"] {
        assert!(
            !stores_check_requested(args),
            "`{args}` must not request a check"
        );
    }
}

// ── Probing (FR-031 --check) ────────────────────────────────────────────────

#[test]
fn probe_reports_both_default_stores_available_with_their_entry_counts() {
    let stores = PluginStoresConfig::default();
    let probes = probe_stores(&stores, &available_fetcher());

    assert_eq!(
        probes.len(),
        2,
        "one probe per store (aligned with StoreKind::ALL)"
    );
    for probe in &probes {
        match probe {
            StoreProbe::Available { count } => {
                assert!(*count >= 1, "a served fixture advertises its entries");
            }
            other => panic!("the compiled default must be reachable offline: {other:?}"),
        }
    }
}

#[test]
fn probe_marks_an_unconfigured_fixture_endpoint_unavailable_with_a_reason() {
    // No endpoint registered: the fetcher returns a contained error, never a
    // network request (NFR-003). The probe surfaces it as `Unavailable`.
    let stores = PluginStoresConfig::default();
    let empty = FixtureStoreFetcher::new();
    let probes = probe_stores(&stores, &empty);

    for probe in &probes {
        match probe {
            StoreProbe::Unavailable { detail } => {
                assert!(
                    detail.contains("no offline fixture"),
                    "the reason names the missing fixture: {detail}"
                );
            }
            other => panic!("an unregistered fixture must be unavailable: {other:?}"),
        }
    }
}

#[test]
fn probe_is_aligned_with_the_store_order_codex_then_claude() {
    // Only the Codex default is registered, so Codex is available and Claude is
    // not -- proving the two probe slots map to the two stores in order.
    let stores = PluginStoresConfig::default();
    let fetcher = FixtureStoreFetcher::new().with_index(
        DEFAULT_CODEX_STORE_URL,
        br#"{"plugins":[{"id":"x","name":"X","version":"1","source":"https://e/x.zip"}]}"#,
    );
    let probes = probe_stores(&stores, &fetcher);

    assert_eq!(
        probes[0],
        StoreProbe::Available { count: 1 },
        "codex is the first probe slot"
    );
    assert!(
        matches!(probes[1], StoreProbe::Unavailable { .. }),
        "claude has no registered fixture: {:?}",
        probes[1]
    );
    // The compiled default endpoints were the ones probed.
    assert_eq!(fetcher.endpoints(), vec![DEFAULT_CODEX_STORE_URL]);
}

// ── Report rendering with probes ────────────────────────────────────────────

#[test]
fn report_appends_an_available_marker_and_count_per_store() {
    let stores = PluginStoresConfig::default();
    let probes = probe_stores(&stores, &available_fetcher());
    let report = render_stores_report_with_probes(&stores, Some(&probes));

    for token in ["codex", "claude"] {
        assert!(
            report.contains(&format!("- {token}: [default]")),
            "the provenance tag is preserved: {report}"
        );
        assert!(
            report.contains("[ok] available,"),
            "the availability marker is present: {report}"
        );
        assert!(
            report.contains("plugins"),
            "the count is labelled: {report}"
        );
    }
    assert!(report.contains(DEFAULT_CODEX_STORE_URL), "{report}");
    assert!(report.contains(DEFAULT_CLAUDE_STORE_URL), "{report}");
    assert!(
        report.is_ascii(),
        "the report must stay ASCII only: {report}"
    );
}

#[test]
fn report_marks_an_unavailable_store_with_the_reason() {
    let stores = PluginStoresConfig::default();
    let probes = probe_stores(&stores, &FixtureStoreFetcher::new());
    let report = render_stores_report_with_probes(&stores, Some(&probes));

    assert!(report.contains("[err] unavailable:"), "{report}");
    assert!(
        report.contains("no offline fixture"),
        "the reason is surfaced inline: {report}"
    );
}

#[test]
fn a_short_probe_slice_leaves_the_remaining_lines_plain() {
    // Defensive: a probe slice shorter than the store list must not panic; the
    // un-probed store keeps the plain config line.
    let stores = PluginStoresConfig::default();
    let report =
        render_stores_report_with_probes(&stores, Some(&[StoreProbe::Available { count: 3 }]));

    assert!(report.contains("- codex: [default]"), "{report}");
    assert!(report.contains("[ok] available, 3 plugins"), "{report}");
    let claude_line = report
        .lines()
        .find(|line| line.contains("claude"))
        .expect("the claude line is present");
    assert_eq!(
        claude_line,
        format!("- claude: [default] {DEFAULT_CLAUDE_STORE_URL}"),
        "the un-probed store keeps the plain config line"
    );
}

#[test]
fn the_default_report_carries_no_probe_suffix() {
    // The plain `/plugins stores` output is unchanged by the `--check` addition.
    let report = render_stores_report(&PluginStoresConfig::default());
    assert!(!report.contains("available"), "{report}");
    assert!(!report.contains("unavailable"), "{report}");
    for token in ["codex", "claude"] {
        assert!(
            report.contains(&format!("- {token}: [default]")),
            "{report}"
        );
    }
    // `render_stores_report` and the `None` probe form agree exactly.
    assert_eq!(
        report,
        render_stores_report_with_probes(&PluginStoresConfig::default(), None)
    );
}

// ── Seam sanity ─────────────────────────────────────────────────────────────

#[test]
fn the_probe_uses_the_injected_seam_not_the_network() {
    // A bespoke closure seam records the endpoints it was asked for, proving the
    // probe routes through the injectable fetcher rather than the HTTPS one.
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = seen.clone();
    let fetcher = move |_kind: ragent_plugins::StoreKind,
                        endpoint: &ragent_plugins::StoreEndpoint,
                        _limits: &ragent_plugins::FetchLimits|
          -> Result<ragent_plugins::StoreIndex, ragent_plugins::StoreError> {
        recorder
            .lock()
            .expect("recorder lock")
            .push(endpoint.as_str().to_string());
        Ok(ragent_plugins::StoreIndex {
            store: None,
            entries: Vec::new(),
            skipped: 0,
        })
    };

    let probes = probe_stores(&PluginStoresConfig::default(), &fetcher);

    assert_eq!(
        probes,
        vec![
            StoreProbe::Available { count: 0 },
            StoreProbe::Available { count: 0 }
        ]
    );
    let recorded = seen.lock().expect("recorder lock").clone();
    assert_eq!(
        recorded,
        vec![DEFAULT_CODEX_STORE_URL, DEFAULT_CLAUDE_STORE_URL],
        "both compiled defaults were probed, in order"
    );
}
