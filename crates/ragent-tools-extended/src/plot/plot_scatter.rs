//! `plot_scatter` tool — render a scatter plot on the message window.

use anyhow::Result;
use ratatui_plt::prelude::{AspectRatio, Axis, MarkerShape, Theme};
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, build_series, canvas_dimension,
    error_output, parse_bool, parse_series, parse_title, render_ansi, render_text, success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

/// Render a scatter plot from a `series` argument.
pub struct PlotScatterTool;

#[async_trait::async_trait]
impl Tool for PlotScatterTool {
    fn name(&self) -> &'static str {
        "plot_scatter"
    }

    fn description(&self) -> &'static str {
        "Render a scatter plot on the message window. Required parameter: \
         'series' — one object {name?, data: [[x,y],...], color?} or an array \
         of such objects. Optional: 'title', 'x_label', 'y_label', 'width', \
         'height', 'x_grid', 'y_grid', 'show_legend'. Points are drawn with \
         dot markers."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "series": {
                    "oneOf": [
                        { "type": "array", "items": { "$ref": "#/$defs/series" } },
                        { "$ref": "#/$defs/series" }
                    ]
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
            None => return Ok(error_output("missing required parameter 'series'", None)),
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

        let mut plot = ratatui_plt::prelude::ScatterPlot::new()
            .theme(Theme::dark())
            .aspect_ratio(AspectRatio::Auto)
            .show_legend(show_legend);
        if let Some(t) = parse_title(&input) {
            plot = plot.title(t);
        }
        let mut x_axis = Axis::new();
        if let Some(l) = input.get("x_label").and_then(Value::as_str)
            && !l.trim().is_empty()
        {
            x_axis = x_axis.label(l);
        }
        x_axis = x_axis.grid(x_grid);
        let mut y_axis = Axis::new();
        if let Some(l) = input.get("y_label").and_then(Value::as_str)
            && !l.trim().is_empty()
        {
            y_axis = y_axis.label(l);
        }
        y_axis = y_axis.grid(y_grid);
        plot = plot.x_axis(x_axis);
        plot = plot.y_axis(y_axis);
        for spec in &specs {
            plot = plot.series(build_series(spec, Some(MarkerShape::Dot)));
        }

        let content = render_text(&plot, width, height);
        let ansi = render_ansi(&plot, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "series": specs.len(),
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "scatter", meta))
    }
}
