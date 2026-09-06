//! `plot_heatmap` tool — render a 2D heatmap on the message window.

use anyhow::Result;
use ratatui_plt::prelude::{AspectRatio, Axis, Theme};
use ratatui_plt::widgets::heatmap::Heatmap;
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, canvas_dimension, error_output,
    parse_bool, parse_title, render_ansi, render_text, success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

/// Set the heatmap colormap by name. Returns `None` for an unknown name so the
/// widget's default (viridis) is used.
fn resolve_colormap(name: Option<&str>) -> Option<Box<dyn ratatui_plt::prelude::Colormap>> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    let cm = ratatui_plt::prelude::get_colormap(name)?;
    Some(cm)
}

#[async_trait::async_trait]
impl Tool for PlotHeatmapTool {
    fn name(&self) -> &'static str {
        "plot_heatmap"
    }

    fn description(&self) -> &'static str {
        "Render a 2D heatmap on the message window. Required: 'grid' — an \
         object {x: [values], y: [values], values: [[row-rows...]]} or simply \
         {values: [[...]]}. Optional: 'colormap' (e.g. viridis, plasma, \
         inferno, magma, coolwarm), 'title', 'x_label', 'y_label', \
         'colorbar' (bool, default true), 'show_values' (bool, default \
         false), 'width', 'height'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "grid": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "array", "items": { "type": "number" } },
                        "y": { "type": "array", "items": { "type": "number" } },
                        "values": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "number" } }
                        }
                    },
                    "required": ["values"],
                    "additionalProperties": false
                },
                "colormap": { "type": "string" },
                "title": { "type": "string" },
                "x_label": { "type": "string" },
                "y_label": { "type": "string" },
                "colorbar": { "type": "boolean" },
                "show_values": { "type": "boolean" },
                "width": { "type": "integer", "minimum": 1, "maximum": 220 },
                "height": { "type": "integer", "minimum": 1, "maximum": 80 }
            },
            "required": ["grid"],
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
        let grid_val = match input.get("grid") {
            Some(v) => super::coerce_json(v),
            None => return Ok(error_output("missing required parameter 'grid'", None)),
        };
        let obj = match grid_val.as_object() {
            Some(o) => o,
            None => {
                return Ok(error_output(
                    "'grid' must be a JSON object of the form {\"values\": [[...]]}",
                    None,
                ));
            }
        };

        // Extract the matrix of values; x/y coordinate axes are optional and
        // default to the row/column index.
        let values_val = obj
            .get("values")
            .ok_or_else(|| anyhow::anyhow!("grid missing required 'values'"))?;
        let mut rows: Vec<Vec<f64>> = Vec::new();
        if let Some(arr) = values_val.as_array() {
            for row in arr {
                match super::parse_floats(row) {
                    Ok(r) => rows.push(r),
                    Err(e) => return Ok(error_output(e.to_string(), None)),
                }
            }
        }
        if rows.is_empty() || rows[0].is_empty() {
            return Ok(error_output(
                "'grid.values' must be a non-empty 2D array",
                None,
            ));
        }
        let row_count = rows.len();
        let col_count = rows[0].len();

        // Default axes to indices when not supplied.
        let x: Vec<f64> = match obj.get("x") {
            Some(xv) => match super::parse_floats(xv) {
                Ok(x) => x,
                Err(e) => return Ok(error_output(e.to_string(), None)),
            },
            None => (0..col_count).map(|i| i as f64).collect(),
        };
        let y: Vec<f64> = match obj.get("y") {
            Some(yv) => match super::parse_floats(yv) {
                Ok(y) => y,
                Err(e) => return Ok(error_output(e.to_string(), None)),
            },
            None => (0..row_count).map(|i| i as f64).collect(),
        };
        let grid = ratatui_plt::series::GridData::new(x, y, rows);

        let width = match canvas_dimension(&input, "width", DEFAULT_WIDTH, MAX_WIDTH) {
            Ok(w) => w,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let height = match canvas_dimension(&input, "height", DEFAULT_HEIGHT, MAX_HEIGHT) {
            Ok(h) => h,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let colorbar = match parse_bool(&input, "colorbar", true) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let show_values = match parse_bool(&input, "show_values", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };

        let mut heat = Heatmap::new(grid)
            .theme(Theme::dark())
            .aspect_ratio(AspectRatio::Auto)
            .show_colorbar(colorbar)
            .show_values(show_values);
        let cmap = resolve_colormap(input.get("colormap").and_then(Value::as_str));
        if let Some(cm) = cmap {
            heat = heat.colormap(cm);
        }
        if let Some(t) = parse_title(&input) {
            heat = heat.title(t);
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
        heat = heat.x_axis(x_axis);
        heat = heat.y_axis(y_axis);

        let content = render_text(&heat, width, height);
        let ansi = render_ansi(&heat, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "rows": row_count,
            "cols": col_count,
            "colormap": input.get("colormap").and_then(Value::as_str).unwrap_or("viridis"),
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "heatmap", meta))
    }
}

/// Render a heatmap from a `grid` argument.
pub struct PlotHeatmapTool;
