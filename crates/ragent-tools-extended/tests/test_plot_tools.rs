//! Tests for the `plot_*` tools (plot_line, plot_scatter, plot_bar,
//! plot_histogram, plot_pie, plot_heatmap).
//!
//! Covers: valid render produces a non-empty canvas with `status: success`;
//! missing/malformed arguments degrade to an error output (`status: error`)
//! without crashing; the tools are registered in the extended registry.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ragent_tools_extended::{Tool, ToolContext, create_extended_registry};
use ragent_types::event::EventBus;
use serde_json::json;

/// Build a minimal `ToolContext` for testing.
fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
    }
}

/// Assert a tool returns a success `status` with non-empty content.
async fn assert_success(tool: &dyn Tool, input: serde_json::Value, name: &str) {
    let out = tool
        .execute(input, &ctx())
        .await
        .unwrap_or_else(|e| panic!("{name}: tool errored unexpectedly: {e}"));
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("success"),
        "{name}: expected success status, got: {:?}",
        out.metadata
    );
    assert!(
        !out.content.trim().is_empty(),
        "{name}: expected non-empty rendered canvas"
    );
    // The TUI renders the plot inline from the `plot` metadata key (the event
    // bus only carries a short preview of `content`), so it must be present
    // and mirror the full canvas.
    let plot = out
        .metadata
        .as_ref()
        .and_then(|m| m.get("plot"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("{name}: expected 'plot' metadata key"));
    assert_eq!(
        plot, out.content,
        "{name}: 'plot' metadata must mirror the rendered content"
    );
    // The ANSI-coloured canvas is also mirrored so the TUI can render the plot
    // with its real per-cell colours (palette series, pie slices, heatmaps).
    let ansi = out
        .metadata
        .as_ref()
        .and_then(|m| m.get("plot_ansi"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("{name}: expected 'plot_ansi' metadata key"));
    assert!(
        !ansi.trim().is_empty(),
        "{name}: expected non-empty ANSI canvas"
    );
    assert!(
        ansi.contains('\u{1b}'),
        "{name}: ANSI canvas should contain SGR escape sequences"
    );
}

/// Assert a tool degrades to an error output instead of crashing.
async fn assert_error(tool: &dyn Tool, input: serde_json::Value, name: &str) {
    let out = tool
        .execute(input, &ctx())
        .await
        .expect("tool should return Ok(error_output) rather than Err");
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("error"),
        "{name}: expected error status, got: {:?}",
        out.metadata
    );
}

#[tokio::test]
async fn test_plot_line_renders() {
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    assert_success(
        &tool,
        json!({
            "series": { "name": "sin", "data": [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]] },
            "title": "Test Plot",
            "x_label": "x",
            "y_label": "y"
        }),
        "plot_line",
    )
    .await;
}

#[tokio::test]
async fn test_plot_line_multiple_series() {
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    assert_success(
        &tool,
        json!({
            "series": [
                { "name": "a", "data": [[0.0, 0.0], [1.0, 1.0]] },
                { "name": "b", "data": [[0.0, 2.0], [1.0, 0.0]] }
            ],
            "height": 12
        }),
        "plot_line_multi",
    )
    .await;
}

#[tokio::test]
async fn test_plot_line_missing_series_errors() {
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    assert_error(&tool, json!({ "title": "x" }), "plot_line_missing").await;
}

#[tokio::test]
async fn test_plot_line_bad_data_errors() {
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    assert_error(
        &tool,
        json!({ "series": { "data": [[0.0], "not-a-point"] } }),
        "plot_line_bad",
    )
    .await;
}

#[tokio::test]
async fn test_plot_scatter_renders() {
    let tool = ragent_tools_extended::plot::plot_scatter::PlotScatterTool;
    assert_success(
        &tool,
        json!({
            "series": [
                { "name": "pts", "data": [[1.0, 2.0], [2.0, 3.0], [3.0, 1.0]], "color": "red" }
            ],
            "title": "Scatter",
            "x_grid": true
        }),
        "plot_scatter",
    )
    .await;
}

#[tokio::test]
async fn test_plot_bar_renders() {
    let tool = ragent_tools_extended::plot::plot_bar::PlotBarTool;
    assert_success(
        &tool,
        json!({
            "categories": ["Q1", "Q2", "Q3", "Q4", "Q5"],
            "datasets": [
                { "name": "Widgets", "data": [120.0, 150.0, 180.0, 140.0, 200.0] },
                { "name": "Gadgets", "data": [90.0, 110.0, 130.0, 160.0, 175.0] }
            ],
            "title": "Sales"
        }),
        "plot_bar",
    )
    .await;
}

#[tokio::test]
async fn test_plot_bar_stacked_horizontal() {
    let tool = ragent_tools_extended::plot::plot_bar::PlotBarTool;
    assert_success(
        &tool,
        json!({
            "categories": ["A", "B"],
            "datasets": { "name": "s", "data": [1.0, 2.0] },
            "stacked": true,
            "horizontal": true,
            "height": 10
        }),
        "plot_bar_stack",
    )
    .await;
}

#[tokio::test]
async fn test_plot_bar_missing_categories_errors() {
    let tool = ragent_tools_extended::plot::plot_bar::PlotBarTool;
    assert_error(
        &tool,
        json!({ "datasets": { "data": [1.0, 2.0] } }),
        "plot_bar_no_cats",
    )
    .await;
}

#[tokio::test]
async fn test_plot_histogram_renders() {
    let tool = ragent_tools_extended::plot::plot_histogram::PlotHistogramTool;
    assert_success(
        &tool,
        json!({ "data": [1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0], "bins": 4 }),
        "plot_histogram",
    )
    .await;
}

#[tokio::test]
async fn test_plot_histogram_norm_density() {
    let tool = ragent_tools_extended::plot::plot_histogram::PlotHistogramTool;
    assert_success(
        &tool,
        json!({ "data": [1.0, 2.0, 2.0, 3.0], "norm": "density", "range": [0.0, 5.0] }),
        "plot_histogram_density",
    )
    .await;
}

#[tokio::test]
async fn test_plot_histogram_empty_errors() {
    let tool = ragent_tools_extended::plot::plot_histogram::PlotHistogramTool;
    assert_error(&tool, json!({ "data": [] }), "plot_histogram_empty").await;
}

#[tokio::test]
async fn test_plot_pie_renders() {
    let tool = ragent_tools_extended::plot::plot_pie::PlotPieTool;
    assert_success(
        &tool,
        json!({
            "slices": [
                { "label": "Rust", "value": 45.0 },
                { "label": "Python", "value": 30.0 },
                { "label": "Other", "value": 25.0 }
            ],
            "title": "Languages",
            "donut": true,
            "percentages": true,
            "labels": false
        }),
        "plot_pie",
    )
    .await;
}

#[tokio::test]
async fn test_plot_pie_missing_slices_errors() {
    let tool = ragent_tools_extended::plot::plot_pie::PlotPieTool;
    assert_error(&tool, json!({ "title": "x" }), "plot_pie_missing").await;
}

#[tokio::test]
async fn test_plot_heatmap_renders() {
    let tool = ragent_tools_extended::plot::plot_heatmap::PlotHeatmapTool;
    assert_success(
        &tool,
        json!({
            "grid": {
                "x": [0.0, 1.0, 2.0],
                "y": [0.0, 1.0],
                "values": [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
            },
            "colormap": "plasma",
            "title": "Heat"
        }),
        "plot_heatmap",
    )
    .await;
}

#[tokio::test]
async fn test_plot_heatmap_default_axes() {
    let tool = ragent_tools_extended::plot::plot_heatmap::PlotHeatmapTool;
    assert_success(
        &tool,
        json!({ "grid": { "values": [[1.0, 2.0], [3.0, 4.0]] }, "colorbar": false }),
        "plot_heatmap_defaults",
    )
    .await;
}

#[tokio::test]
async fn test_plot_heatmap_missing_grid_errors() {
    let tool = ragent_tools_extended::plot::plot_heatmap::PlotHeatmapTool;
    assert_error(
        &tool,
        json!({ "colormap": "viridis" }),
        "plot_heatmap_missing",
    )
    .await;
}

#[tokio::test]
async fn test_plot_tools_registered_in_registry() {
    let registry = create_extended_registry();
    for name in [
        "plot_line",
        "plot_scatter",
        "plot_bar",
        "plot_histogram",
        "plot_pie",
        "plot_heatmap",
    ] {
        assert!(
            registry.get(name).is_some(),
            "expected {name} to be registered"
        );
        assert_eq!(
            registry.get(name).unwrap().permission_category(),
            "system",
            "{name} should be a read-only system tool"
        );
    }
}

#[tokio::test]
async fn test_plot_canvas_bounds_are_enforced() {
    // A huge requested width must be clamped to MAX_WIDTH so tool output stays
    // bounded rather than ballooning the message window.
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    let out = tool
        .execute(
            json!({
                "series": { "data": [[0.0, 0.0], [1.0, 1.0]] },
                "width": 999_999,
                "height": 999_999
            }),
            &ctx(),
        )
        .await
        .expect("tool should not error");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "success");
    assert_eq!(meta["width"], 220, "width must be clamped to MAX_WIDTH");
    assert_eq!(meta["height"], 80, "height must be clamped to MAX_HEIGHT");
}

#[tokio::test]
async fn test_plot_string_encoded_series_is_coerced() {
    // Some LLM tool bridges serialize complex array/object arguments as JSON
    // strings instead of structured values. The series parser must coerce a
    // string-encoded payload back into a structured value rather than failing.
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    let stringified = r#"[{"name": "a", "data": [[0, 0], [1, 1]], "color": "red"},
                          {"name": "b", "data": [[0, 1], [1, 0]], "color": "blue"}]"#;
    assert_success(
        &tool,
        json!({ "series": stringified, "title": "coerced" }),
        "plot_line_string_coerce",
    )
    .await;
}

#[tokio::test]
async fn test_plot_string_encoded_datasets_is_coerced() {
    let tool = ragent_tools_extended::plot::plot_bar::PlotBarTool;
    let stringified = r#"[{"name": "loc", "data": [10, 20], "color": "green"}]"#;
    assert_success(
        &tool,
        json!({ "categories": ["x", "y"], "datasets": stringified }),
        "plot_bar_string_coerce",
    )
    .await;
}

#[tokio::test]
async fn test_plot_string_encoded_data_is_coerced() {
    let tool = ragent_tools_extended::plot::plot_histogram::PlotHistogramTool;
    let out = tool
        .execute(json!({ "data": "[1.0, 2.0, 2.0, 3.0]", "bins": 4 }), &ctx())
        .await
        .expect("tool should not error");
    assert_eq!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("success"),
        "string-encoded histogram data should be coerced"
    );
}

#[tokio::test]
async fn test_plot_string_encoded_slices_is_coerced() {
    let tool = ragent_tools_extended::plot::plot_pie::PlotPieTool;
    let stringified = r#"[{"label": "a", "value": 1, "color": "red"},
                          {"label": "b", "value": 2, "color": "blue"}]"#;
    assert_success(
        &tool,
        json!({ "slices": stringified }),
        "plot_pie_string_coerce",
    )
    .await;
}

#[tokio::test]
async fn test_plot_string_encoded_grid_is_coerced() {
    let tool = ragent_tools_extended::plot::plot_heatmap::PlotHeatmapTool;
    let stringified = r#"{"values": [[1, 2], [3, 4]]}"#;
    assert_success(
        &tool,
        json!({ "grid": stringified, "colormap": "viridis" }),
        "plot_heatmap_string_coerce",
    )
    .await;
}

#[tokio::test]
async fn test_plot_string_encoded_grid_invalid_still_errors() {
    // A string that does NOT parse as JSON must keep the original error path
    // (status: error) rather than silently rendering an empty canvas.
    let tool = ragent_tools_extended::plot::plot_heatmap::PlotHeatmapTool;
    assert_error(
        &tool,
        json!({ "grid": "not-json-at-all" }),
        "plot_heatmap_bad_string",
    )
    .await;
}

#[tokio::test]
async fn test_plot_named_colours_produce_distinct_sgr_runs() {
    // Colour-range regression: explicitly coloured series must be reflected
    // in the ANSI canvas as multiple distinct SGR colour runs, so the TUI
    // renders the series in their requested colours instead of one flat hue.
    let tool = ragent_tools_extended::plot::plot_line::PlotLineTool;
    let out = tool
        .execute(
            json!({
                "series": [
                    { "name": "a", "data": [[0, 0], [1, 1]], "color": "red" },
                    { "name": "b", "data": [[0, 1], [1, 0]], "color": "blue" },
                    { "name": "c", "data": [[0, 2], [1, 2]], "color": "green" }
                ],
                "height": 12
            }),
            &ctx(),
        )
        .await
        .expect("tool should not error");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "success");
    let ansi = meta["plot_ansi"].as_str().expect("plot_ansi present");
    let mut codes = std::collections::BTreeSet::new();
    let mut chars = ansi.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphabetic() {
                    chars.next();
                    break;
                }
                params.push(n);
                chars.next();
            }
            // Only count foreground-setting sequences (31=red, 34=blue, etc.).
            if params == "31" || params == "34" || params == "32" {
                codes.insert(params);
            }
        }
    }
    assert_eq!(
        codes.len(),
        3,
        "expected red+green+blue SGR foreground codes in the ANSI canvas, got {codes:?}"
    );
}
