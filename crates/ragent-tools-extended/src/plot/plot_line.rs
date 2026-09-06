//! `plot_line` tool — render a line (XY) plot on the message window.
//!
//! Draws one or more series of `[x, y]` points as a line chart using
//! `ratatui-plt`, rendered off-screen to a plain-text canvas. Useful for
//! showing trends, time series, functions, and benchmarks.

use anyhow::Result;
use ratatui_plt::prelude::{AspectRatio, Axis, LinePlot, Theme};
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, build_series, canvas_dimension,
    error_output, parse_bool, parse_series, parse_title, render_ansi, render_text, success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

/// Render a line plot from a `series` argument.
pub struct PlotLineTool;

fn axis(label: Option<&str>, grid: bool) -> Axis {
    let mut a = Axis::new();
    if let Some(l) = label {
        if !l.trim().is_empty() {
            a = a.label(l);
        }
    }
    a = a.grid(grid);
    a
}

#[async_trait::async_trait]
impl Tool for PlotLineTool {
    fn name(&self) -> &'static str {
        "plot_line"
    }

    fn description(&self) -> &'static str {
        "Render a line (XY) plot on the message window. Required parameter: \
         'series' — either one object {name?, data: [[x,y],...], color?} or an \
         array of such objects. Optional: 'title', 'x_label', 'y_label', \
         'width' (default 80), 'height' (default 20, max 80), 'x_grid', \
         'y_grid', 'show_legend'. Returns the plot as terminal text."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "series": {
                    "oneOf": [
                        { "type": "array", "items": { "$ref": "#/$defs/series" } },
                        { "$ref": "#/$defs/series" }
                    ],
                    "description": "One or more named series ({name, data: [[x,y]], color})"
                },
                "title": { "type": "string" },
                "x_label": { "type": "string" },
                "y_label": { "type": "string" },
                "width": { "type": "integer", "minimum": 1, "maximum": 220 },
                "height": { "type": "integer", "minimum": 1, "maximum": 80 },
                "x_grid": { "type": "boolean" },
                "y_grid": { "type": "boolean" },
                "show_legend": { "type": "boolean" }
            },
            "$defs": {
                "series": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "data": {
                            "type": "array",
                            "items": {
                                "type": "array",
                                "items": { "type": "number" },
                                "minItems": 2,
                                "maxItems": 2
                            }
                        },
                        "color": { "type": "string" }
                    },
                    "required": ["data"],
                    "additionalProperties": false
                }
            },
            "required": ["series"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "system"
    }

    /// # Errors
    ///
    /// Returns an error output (never a session crash) when the `series`
    /// argument is missing or malformed.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let series_val = match input.get("series") {
            Some(v) => super::coerce_json(v),
            None => {
                return Ok(error_output("missing required parameter 'series'", None));
            }
        };
        let specs = match parse_series(&series_val) {
            Ok(s) => s,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        if specs.is_empty() {
            return Ok(error_output("'series' contains no data series", None));
        }

        let width = match canvas_dimension(&input, "width", DEFAULT_WIDTH, MAX_WIDTH) {
            Ok(w) => w,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let height = match canvas_dimension(&input, "height", DEFAULT_HEIGHT, MAX_HEIGHT) {
            Ok(h) => h,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let x_grid = match parse_bool(&input, "x_grid", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let y_grid = match parse_bool(&input, "y_grid", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let show_legend = match parse_bool(&input, "show_legend", true) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };

        let mut plot = LinePlot::new()
            .theme(Theme::dark())
            .aspect_ratio(AspectRatio::Auto)
            .show_legend(show_legend);
        if let Some(t) = parse_title(&input) {
            plot = plot.title(t);
        }
        plot = plot.x_axis(axis(input.get("x_label").and_then(Value::as_str), x_grid));
        plot = plot.y_axis(axis(input.get("y_label").and_then(Value::as_str), y_grid));
        for spec in &specs {
            plot = plot.series(build_series(spec, None));
        }

        let content = render_text(&plot, width, height);
        let ansi = render_ansi(&plot, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "series": specs.len(),
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "line", meta))
    }
}
