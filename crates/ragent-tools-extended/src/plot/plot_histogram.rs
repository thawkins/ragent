//! `plot_histogram` tool — render a histogram on the message window.

use anyhow::Result;
use ratatui_plt::prelude::{Axis, Color};
use ratatui_plt::widgets::histogram::{HistNorm, Histogram};
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, canvas_dimension, error_output,
    parse_bool, parse_floats, parse_title, render_ansi, render_text, success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

/// Render a histogram from a `data` argument.
pub struct PlotHistogramTool;

fn parse_range(input: &Value) -> Result<Option<(f64, f64)>> {
    let v = match input.get("range") {
        None | Some(Value::Null) => return Ok(None),
        Some(v) => v,
    };
    let arr = v
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'range' must be a [min, max] array, got {v}"))?;
    if arr.len() != 2 {
        return Err(anyhow::anyhow!("'range' must have exactly two values"));
    }
    let min = arr[0]
        .as_f64()
        .or_else(|| arr[0].as_u64().map(|u| u as f64))
        .ok_or_else(|| anyhow::anyhow!("'range' values must be numeric"))?;
    let max = arr[1]
        .as_f64()
        .or_else(|| arr[1].as_u64().map(|u| u as f64))
        .ok_or_else(|| anyhow::anyhow!("'range' values must be numeric"))?;
    Ok(Some((min, max)))
}

#[async_trait::async_trait]
impl Tool for PlotHistogramTool {
    fn name(&self) -> &'static str {
        "plot_histogram"
    }

    fn description(&self) -> &'static str {
        "Render a histogram on the message window. Required: 'data' — an array \
         of numbers. Optional: 'bins' (default 20), 'range' ([min, max]), \
         'norm' ('count'|'density'|'probability'), 'color', 'title', \
         'x_label', 'y_label', 'width', 'height', 'cumulative'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "data": { "type": "array", "items": { "type": "number" } },
                "bins": { "type": "integer", "minimum": 1 },
                "range": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
                "norm": { "type": "string", "enum": ["count", "density", "probability"] },
                "color": { "type": "string" },
                "title": { "type": "string" },
                "x_label": { "type": "string" },
                "y_label": { "type": "string" },
                "width": { "type": "integer", "minimum": 1, "maximum": 220 },
                "height": { "type": "integer", "minimum": 1, "maximum": 80 },
                "cumulative": { "type": "boolean" }
            },
            "required": ["data"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "system"
    }

    /// # Errors
    ///
    /// Returns an error output (never a session crash) when arguments are
    /// missing or malformed.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let data_val = match input.get("data") {
            Some(v) => super::coerce_json(v),
            None => return Ok(error_output("missing required parameter 'data'", None)),
        };
        let data = match parse_floats(&data_val) {
            Ok(d) => d,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        if data.is_empty() {
            return Ok(error_output("'data' contains no values", None));
        }

        let width = match canvas_dimension(&input, "width", DEFAULT_WIDTH, MAX_WIDTH) {
            Ok(w) => w,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let height = match canvas_dimension(&input, "height", DEFAULT_HEIGHT, MAX_HEIGHT) {
            Ok(h) => h,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let sample_count = data.len();
        let bins = input
            .get("bins")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .max(1) as usize;
        let cumulative = match parse_bool(&input, "cumulative", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let norm = match input.get("norm").and_then(Value::as_str).unwrap_or("count") {
            "density" => HistNorm::Density,
            "probability" => HistNorm::Probability,
            _ => HistNorm::Count,
        };
        let color =
            super::parse_color(input.get("color").and_then(Value::as_str)).unwrap_or(Color::Cyan);

        let mut hist = Histogram::new(data)
            .bins(bins)
            .norm_mode(norm)
            .color(color)
            .cumulative(cumulative);
        match parse_range(&input) {
            Ok(Some((mn, mx))) => {
                hist = hist.range(mn, mx);
            }
            Ok(None) => {}
            Err(e) => return Ok(error_output(e.to_string(), None)),
        }
        if let Some(t) = parse_title(&input) {
            hist = hist.title(t);
        }
        let mut x_axis = Axis::new();
        if let Some(l) = input.get("x_label").and_then(Value::as_str)
            && !l.trim().is_empty()
        {
            x_axis = x_axis.label(l);
        }
        let mut y_axis = Axis::new();
        if let Some(l) = input.get("y_label").and_then(Value::as_str)
            && !l.trim().is_empty()
        {
            y_axis = y_axis.label(l);
        }
        hist = hist.x_axis(x_axis);
        hist = hist.y_axis(y_axis);
        // Disable the legend for a single-source histogram by default.
        hist = hist.show_legend(false);

        let content = render_text(&hist, width, height);
        let ansi = render_ansi(&hist, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "bins": bins,
            "samples": sample_count,
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "histogram", meta))
    }
}
