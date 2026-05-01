//! Shared XLSX writing helpers used by office tools and benchmark exports.
//!
//! This module centralizes the workbook writer so every XLSX-producing surface
//! in the workspace uses the same `serde_json` sheet/row representation.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Write an XLSX workbook from JSON sheet data.
///
/// Expected shape:
/// ```json
/// {
///   "sheets": [
///     { "name": "Sheet1", "rows": [["A1", "B1"], [1, true]] }
///   ]
/// }
/// ```
///
/// # Errors
///
/// Returns an error when the content shape is invalid or the workbook cannot be
/// written to disk.
pub fn write_xlsx(path: &Path, content: &Value) -> Result<()> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();

    let sheets = content["sheets"]
        .as_array()
        .context("Missing 'sheets' array in xlsx content")?;

    for sheet_def in sheets {
        let sheet_name = sheet_def["name"].as_str().unwrap_or("Sheet1");

        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(sheet_name)
            .map_err(|e| anyhow::anyhow!("Failed to set sheet name: {e}"))?;

        if let Some(rows) = sheet_def["rows"].as_array() {
            for (row_idx, row) in rows.iter().enumerate() {
                if let Some(cells) = row.as_array() {
                    for (col_idx, cell) in cells.iter().enumerate() {
                        let row_num = row_idx as u32;
                        let col_num = col_idx as u16;

                        match cell {
                            Value::Number(n) => {
                                if let Some(f) = n.as_f64() {
                                    worksheet.write_number(row_num, col_num, f).map_err(|e| {
                                        anyhow::anyhow!("Failed to write number: {e}")
                                    })?;
                                }
                            }
                            Value::Bool(b) => {
                                worksheet
                                    .write_boolean(row_num, col_num, *b)
                                    .map_err(|e| anyhow::anyhow!("Failed to write boolean: {e}"))?;
                            }
                            Value::String(s) => {
                                worksheet
                                    .write_string(row_num, col_num, s)
                                    .map_err(|e| anyhow::anyhow!("Failed to write string: {e}"))?;
                            }
                            Value::Null => {}
                            _ => {
                                let s = cell.to_string();
                                worksheet
                                    .write_string(row_num, col_num, &s)
                                    .map_err(|e| anyhow::anyhow!("Failed to write value: {e}"))?;
                            }
                        }
                    }
                }
            }
        }
    }

    workbook
        .save(path)
        .map_err(|e| anyhow::anyhow!("Failed to save xlsx: {e}"))?;

    Ok(())
}
