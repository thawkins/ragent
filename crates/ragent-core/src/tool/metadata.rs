//! Metadata builder for standardized tool output metadata.
//!
//! This module provides a fluent API for building consistent metadata
//! across all tools. It enforces standard field names and provides
//! validation for common metadata patterns.
//!
//! # Example
//!
//! ```
//! use ragent_core::tool::metadata::MetadataBuilder;
//!
//! let metadata = MetadataBuilder::new()
//!     .path("/path/to/file.txt")
//!     .line_count(42)
//!     .byte_count(1024)
//!     .build();
//! ```

use serde::Serialize;
use serde_json::{Map, Value};

/// Builder for creating standardized tool output metadata.
///
/// The builder follows a fluent API pattern and ensures consistent
/// field naming across all tools. Fields are added to a JSON object
/// that can be attached to [`ToolOutput`](crate::tool::ToolOutput).
#[derive(Debug, Default)]
pub struct MetadataBuilder {
    inner: Map<String, Value>,
}

impl MetadataBuilder {
    /// Create a new empty metadata builder.
    pub fn new() -> Self {
        Self { inner: Map::new() }
    }

    /// Add a path field to the metadata.
    ///
    /// Use this for tools that operate on a single file or directory.
    #[must_use]
    pub fn path(mut self, path: impl AsRef<str>) -> Self {
        self.inner
            .insert("path".to_string(), Value::String(path.as_ref().to_string()));
        self
    }

    /// Add a line count field to the metadata.
    ///
    /// Use this for tools that return content with line counts.
    #[must_use]
    pub fn line_count(mut self, count: usize) -> Self {
        self.inner
            .insert("line_count".to_string(), Value::Number(count.into()));
        self
    }

    /// Add a total lines field (used when content is truncated).
    ///
    /// Use this together with `line_count` to indicate the total
    /// number of lines in the source before truncation.
    #[must_use]
    pub fn total_lines(mut self, count: usize) -> Self {
        self.inner
            .insert("total_lines".to_string(), Value::Number(count.into()));
        self
    }

    /// Add a flag indicating whether content was truncated.
    #[must_use]
    pub fn summarized(mut self, is_summarized: bool) -> Self {
        self.inner
            .insert("summarized".to_string(), Value::Bool(is_summarized));
        self
    }

    /// Add a flag indicating whether content was truncated.
    /// Alias for `summarized` for API consistency.
    #[must_use]
    pub fn truncated(mut self, is_truncated: bool) -> Self {
        self.inner
            .insert("truncated".to_string(), Value::Bool(is_truncated));
        self
    }

    /// Add a byte count field to the metadata.
    ///
    /// Use this for tools that write or read file content.
    #[must_use]
    pub fn byte_count(mut self, count: usize) -> Self {
        self.inner
            .insert("byte_count".to_string(), Value::Number(count.into()));
        self
    }

    /// Add an exit code field to the metadata.
    ///
    /// Use this for execution tools like bash or execute_python.
    #[must_use]
    pub fn exit_code(mut self, code: i32) -> Self {
        self.inner
            .insert("exit_code".to_string(), Value::Number(code.into()));
        self
    }

    /// Add a duration field to the metadata in milliseconds.
    ///
    /// Use this for tools that measure execution time.
    #[must_use]
    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.inner
            .insert("duration_ms".to_string(), Value::Number(duration.into()));
        self
    }

    /// Add a timed out flag to the metadata.
    #[must_use]
    pub fn timed_out(mut self, timed_out: bool) -> Self {
        self.inner
            .insert("timed_out".to_string(), Value::Bool(timed_out));
        self
    }

    /// Add a count field for generic counting.
    ///
    /// Use this for search results, file listings, etc.
    #[must_use]
    pub fn count(mut self, count: usize) -> Self {
        self.inner
            .insert("count".to_string(), Value::Number(count.into()));
        self
    }

    /// Add a file count field to the metadata.
    ///
    /// Use this for tools that operate on multiple files.
    #[must_use]
    pub fn file_count(mut self, count: usize) -> Self {
        self.inner
            .insert("file_count".to_string(), Value::Number(count.into()));
        self
    }

    /// Add an entries count field to the metadata.
    ///
    /// Use this for tools that return collections of items.
    #[must_use]
    pub fn entries(mut self, count: usize) -> Self {
        self.inner
            .insert("entries".to_string(), Value::Number(count.into()));
        self
    }

    /// Add a matches count field to the metadata.
    ///
    /// Use this for search tools like grep.
    #[must_use]
    pub fn matches(mut self, count: usize) -> Self {
        self.inner
            .insert("matches".to_string(), Value::Number(count.into()));
        self
    }

    /// Add a status code field to the metadata.
    ///
    /// Use this for HTTP request tools.
    #[must_use]
    pub fn status_code(mut self, code: u16) -> Self {
        self.inner
            .insert("status_code".to_string(), Value::Number(code.into()));
        self
    }

    /// Add old and new line counts for edit operations.
    ///
    /// Use this for tools that modify file content.
    #[must_use]
    pub fn edit_lines(mut self, old: usize, new: usize) -> Self {
        self.inner
            .insert("old_lines".to_string(), Value::Number(old.into()));
        self.inner
            .insert("new_lines".to_string(), Value::Number(new.into()));
        self
    }

    /// Add a task ID field to the metadata.
    ///
    /// Use this for task management tools.
    #[must_use]
    pub fn task_id(mut self, task_id: impl AsRef<str>) -> Self {
        self.inner.insert(
            "task_id".to_string(),
            Value::String(task_id.as_ref().to_string()),
        );
        self
    }

    /// Add a custom field to the metadata.
    ///
    /// Use this sparingly for tool-specific fields that don't fit
    /// the standard fields.
    #[must_use]
    pub fn custom(mut self, key: impl AsRef<str>, value: impl Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(value) => {
                self.inner.insert(key.as_ref().to_string(), value);
            }
            Err(_) => {
                // Silently skip invalid values
            }
        }
        self
    }

    /// Build the metadata into a JSON [`Value`].
    ///
    /// Returns `Some(Value)` if there are fields, `None` if empty.
    pub fn build(self) -> Option<Value> {
        if self.inner.is_empty() {
            None
        } else {
            Some(Value::Object(self.inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_builder_returns_none() {
        let metadata = MetadataBuilder::new().build();
        assert!(metadata.is_none());
    }

    #[test]
    fn test_single_field() {
        let metadata = MetadataBuilder::new().path("/test/file.txt").build();
        assert!(metadata.is_some());
        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("path").unwrap().as_str().unwrap(), "/test/file.txt");
    }

    #[test]
    fn test_multiple_fields() {
        let metadata = MetadataBuilder::new()
            .path("/test/file.txt")
            .line_count(42)
            .byte_count(1024)
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("path").unwrap().as_str().unwrap(), "/test/file.txt");
        assert_eq!(obj.get("line_count").unwrap().as_u64().unwrap(), 42);
        assert_eq!(obj.get("byte_count").unwrap().as_u64().unwrap(), 1024);
    }

    #[test]
    fn test_chaining() {
        let metadata = MetadataBuilder::new()
            .exit_code(0)
            .duration_ms(150)
            .timed_out(false)
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("exit_code").unwrap().as_i64().unwrap(), 0);
        assert_eq!(obj.get("duration_ms").unwrap().as_u64().unwrap(), 150);
        assert_eq!(obj.get("timed_out").unwrap().as_bool().unwrap(), false);
    }

    #[test]
    fn test_edit_lines() {
        let metadata = MetadataBuilder::new()
            .path("/test/file.txt")
            .edit_lines(10, 5)
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("old_lines").unwrap().as_u64().unwrap(), 10);
        assert_eq!(obj.get("new_lines").unwrap().as_u64().unwrap(), 5);
    }

    #[test]
    fn test_summarized() {
        let metadata = MetadataBuilder::new()
            .line_count(100)
            .total_lines(500)
            .summarized(true)
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("line_count").unwrap().as_u64().unwrap(), 100);
        assert_eq!(obj.get("total_lines").unwrap().as_u64().unwrap(), 500);
        assert_eq!(obj.get("summarized").unwrap().as_bool().unwrap(), true);
    }

    #[test]
    fn test_custom_field() {
        let metadata = MetadataBuilder::new()
            .path("/test/file.txt")
            .custom("custom_key", "custom_value")
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(
            obj.get("custom_key").unwrap().as_str().unwrap(),
            "custom_value"
        );
    }

    #[test]
    fn test_count_fields() {
        let metadata = MetadataBuilder::new()
            .count(42)
            .file_count(5)
            .entries(10)
            .matches(3)
            .build();

        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("count").unwrap().as_u64().unwrap(), 42);
        assert_eq!(obj.get("file_count").unwrap().as_u64().unwrap(), 5);
        assert_eq!(obj.get("entries").unwrap().as_u64().unwrap(), 10);
        assert_eq!(obj.get("matches").unwrap().as_u64().unwrap(), 3);
    }

    #[test]
    fn test_task_id() {
        let metadata = MetadataBuilder::new().task_id("task-001").build();
        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("task_id").unwrap().as_str().unwrap(), "task-001");
    }

    #[test]
    fn test_status_code() {
        let metadata = MetadataBuilder::new().status_code(200).build();
        let obj = metadata.unwrap().as_object().unwrap().clone();
        assert_eq!(obj.get("status_code").unwrap().as_u64().unwrap(), 200);
    }
}
