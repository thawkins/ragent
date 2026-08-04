//! Codebase index full-text search tool.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use crate::codeindex_utils::{busy_output, with_retry};

/// Search the codebase index for symbols, functions, types, and documentation
/// using full-text search with optional structured filters.
pub struct CodeIndexSearchTool;

#[async_trait::async_trait]
impl Tool for CodeIndexSearchTool {
    fn name(&self) -> &'static str {
        "codeindex_search"
    }

    fn description(&self) -> &'static str {
        "Search the codebase index for symbols, functions, types, and documentation. \
         Required parameter: 'query' (symbol name, keyword, or phrase). Optional \
         filters: 'kind' (function/struct/enum/trait/etc.), 'language' (e.g. rust), \
         'file_pattern' (path substring), and 'max_results' (default 20, max 100). \
         USE THIS instead of `grep` or `search` when looking for named code entities; \
         use `grep` only for arbitrary text patterns, comments, or non-symbol content."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — symbol name, keyword, or phrase to find in the codebase"
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by symbol kind: function, struct, enum, trait, impl, const, static, type_alias, module, macro, field, variant, interface, class, method",
                    "enum": ["function", "struct", "enum", "trait", "impl", "const", "static", "type_alias", "module", "macro", "field", "variant", "interface", "class", "method"]
                },
                "language": {
                    "type": "string",
                    "description": "Filter by programming language (e.g. 'rust', 'python', 'typescript')"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Filter by file path substring (e.g. 'src/parser' or '.rs')"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 20, max: 100)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => {
                return Ok(codeindex_not_available(
                    "Use `grep` or `glob` as fallback tools for code search.",
                    &["grep", "glob"],
                ));
            }
        };

        let query_str = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let kind = input["kind"]
            .as_str()
            .and_then(|k| k.parse::<ragent_codeindex::types::SymbolKind>().ok());
        let language = input["language"].as_str().map(String::from);
        let file_pattern = input["file_pattern"].as_str().map(String::from);
        let max_results = input["max_results"]
            .as_u64()
            .map_or(20, |n| n.min(100) as usize);

        let search_query = ragent_codeindex::types::SearchQuery {
            query: query_str.to_string(),
            kind,
            language,
            file_pattern,
            max_results,
            include_body: false,
        };

        let results = match with_retry(|| idx.try_search(&search_query)).await? {
            Some(r) => r,
            None => return Ok(busy_output("codeindex_search")),
        };

        if results.is_empty() {
            return Ok(ToolOutput {
                content: format!("No results found for '{query_str}'."),
                metadata: Some(json!({"total_results": 0})),
            });
        }

        let mut output = String::new();
        for (i, r) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. {} `{}` — {}:{}\n",
                i + 1,
                r.kind,
                r.symbol_name,
                r.file_path,
                r.line,
            ));
            if !r.signature.is_empty() {
                output.push_str(&format!("   {}\n", r.signature));
            }
            if !r.doc_snippet.is_empty() {
                let doc = ragent_types::truncate_bytes(&r.doc_snippet, 120);
                output.push_str(&format!("   /// {doc}\n"));
            }
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({"total_results": results.len()})),
        })
    }
}
