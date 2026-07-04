//! System-prompt and tool-reference builders for agent sessions.
//!
//! These functions assemble the per-turn system prompt sections that are
//! injected into every [`crate::session::processor::SessionProcessor`] turn:
//! the codebase-index guidance (active vs. disabled), the compact and detailed
//! tool-reference listings, and the universal tool-calling directive.

/// Universal tool-calling guidance injected into every session's system prompt.
///
/// Previously this was an Ollama-specific constant (`OLLAMA_TOOL_GUIDANCE`),
/// but the directives are universally useful — all providers benefit from clear
/// "call tools immediately, don't narrate" instructions.  The file-reading
/// guidance that used to be here has been removed because it is already built
/// into the base system prompt (`build_system_prompt_with_storage`), so
/// duplicating it here was redundant and created a maintenance drift risk.
pub const TOOL_CALLING_GUIDANCE: &str = "\n## Tool Use — Critical Instructions\n\n\
    IMPORTANT: When you need to take any action, call the appropriate tool IMMEDIATELY.\n\
    Do NOT write text describing what you are going to do — just call the tool.\n\
    Do NOT say 'Let me explore...' or 'I will analyze...' — instead, call the relevant tool now.\n\n\
    Rule: every response where you need information or need to act MUST start with a tool call.\n\n";

/// Build a system-prompt section describing the codebase index tools.
///
/// Uses strong directive language to steer the LLM away from `grep`/`search`
/// for any query that involves code symbols, types, or structure.
pub(crate) fn build_codeindex_guidance_section_active() -> String {
    "\n## Code Intelligence — Codebase Index Tools\n\n\
            A **codebase index** is active for this project. It provides fast, structured \
            search across all indexed source files — symbols, references, dependencies, \
            and documentation.\n\n\
            **MANDATORY — You MUST use codeindex tools instead of grep for code symbol queries.**\n\
            When the index is active, `grep` is the WRONG choice for finding \
            functions, types, structs, enums, traits, or any named code entity. The index \
            is faster, returns structured results with file/line/signature, and understands \
            symbol kinds.\n\n\
            **Decision flow — which tool to use:**\n\
            - \"Where is function X defined?\" → `codeindex_search` (NOT grep)\n\
            - \"Find all structs matching Y\" → `codeindex_symbols` with kind=struct (NOT grep)\n\
            - \"Who calls function Z?\" → `codeindex_references` (NOT grep)\n\
            - \"What does file A import?\" → `codeindex_dependencies` (NOT grep for imports)\n\
            - \"List all functions in file B\" → `codeindex_symbols` with file_path (NOT grep)\n\
            - \"Is the index working?\" → `codeindex_status`\n\
            - \"Re-index after bulk edits\" → `codeindex_reindex`\n\n\
            **When grep IS appropriate:**\n\
            - Searching for arbitrary text strings, comments, or prose (not symbols)\n\
            - Finding TODO/FIXME/HACK comments\n\
            - Searching config files, markdown, or non-code text\n\
            - Pattern matching across many files for non-structural content\n\n\
            **Rule of thumb:** If you are looking for a named code entity (function, type, \
            variable, import), use codeindex. If you are searching for a text pattern that \
            is NOT a code symbol, use grep with the `pattern` parameter.\n\n\
            **CRITICAL — grep parameter requirement:**\n\
            The `grep` tool requires the `pattern` parameter. This is the ONLY required field. \
            Do NOT omit it. Example: `grep(pattern: \"fn main\", path: \"src\")`\n\n"
        .to_string()
}

/// Build a system-prompt section for when the codebase index is NOT active.
///
/// Informs the LLM that codeindex tools will return "not available" and
/// that grep/search should be used as fallback. Suggests enabling the index.
pub(crate) fn build_codeindex_guidance_section_disabled() -> String {
    "\n## Code Intelligence — Codebase Index Tools\n\n\
            The codebase index is **not active** for this project. Code index tools \
            (`codeindex_search`, `codeindex_symbols`, `codeindex_references`, \
            `codeindex_dependencies`) will return \"not available\" if called.\n\n\
            Use `grep` with the `pattern` parameter for code lookups in the meantime. You can suggest \
            the user enable the index with `/codeindex on` for faster, structured \
            symbol search.\n\n\
            **CRITICAL — grep parameter requirement:**\n\
            The `grep` tool requires the `pattern` parameter. This is the ONLY required field. \
            Do NOT omit it. Example: `grep(pattern: \"fn main\", path: \"src\")`\n\n"
        .to_string()
}

/// Build a concise system-prompt section listing every registered tool by name and description.
///
/// Injected into every session's system prompt so the model always knows the exact tool names
/// available. This prevents hallucinated tool names (e.g. calling "search" instead of "grep").
pub(crate) fn build_tool_reference_section(registry: &crate::tool::ToolRegistry) -> String {
    let defs = registry.definitions();
    if defs.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "## Available Tools\n\nYou have access to the following tools. \
          Use ONLY these exact tool names — do not invent or guess tool names.\n\n",
    );
    for def in &defs {
        // Truncate long descriptions to keep the prompt compact.
        let desc = ragent_types::truncate_bytes(&def.description, 120);
        section.push_str(&format!("- `{}` — {}\n", def.name, desc));
    }
    section.push('\n');
    section
}

/// Build a detailed tool-reference section for sub-agent sessions.
///
/// Unlike [`build_tool_reference_section`], this includes each tool's full
/// description and a compact rendering of its JSON Schema parameters
/// (property name, type, whether it is required, and description).  This
/// gives sub-agents the same level of tool documentation that the primary
/// agent's system prompt carries, without relying on the model to infer
/// parameter names or required fields from the API schema alone.
pub fn build_detailed_tool_reference_section(registry: &crate::tool::ToolRegistry) -> String {
    let defs = registry.definitions();
    if defs.is_empty() {
        return String::new();
    }

    let mut section = String::from(
        "## Available Tools\n\n\
        You have access to the following tools. Use ONLY these exact tool \
        names — do not invent or guess tool names. Each tool's parameters are \
        listed with their type and whether they are required (required).\n\n",
    );

    for def in &defs {
        let desc = def.description.replace('\n', " ").replace('\r', "");
        let desc = ragent_types::truncate_bytes(&desc, 400);
        section.push_str(&format!("### `{}`\n{}\n\n", def.name, desc));

        if let Some(properties) = def.parameters.get("properties").and_then(|v| v.as_object()) {
            let required: std::collections::HashSet<&str> = def
                .parameters
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            for (param_name, schema) in properties {
                let param_type = schema
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        if schema.get("anyOf").is_some() || schema.get("oneOf").is_some() {
                            "anyOf"
                        } else {
                            "any"
                        }
                    });

                let param_desc = schema
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .replace('\n', " ")
                    .replace('\r', "");
                let param_desc = ragent_types::truncate_bytes(&param_desc, 140);

                let req_marker = if required.contains(param_name.as_str()) {
                    " (required)"
                } else {
                    ""
                };

                section.push_str(&format!(
                    "- `{}` (`{}`{}) — {}\n",
                    param_name, param_type, req_marker, param_desc
                ));
            }
        }

        section.push('\n');
    }

    section
}
