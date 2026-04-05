//! Alias tools that map commonly hallucinated tool names to canonical implementations.
//!
//! Many LLMs emit tool names that differ from ragent's canonical names — either
//! because they have been trained on different coding-agent frameworks or because
//! they extrapolate plausible-sounding names from the task context.  Rather than
//! returning "Unknown tool" errors, each alias tool normalises its parameter names
//! and delegates to the canonical implementation.
//!
//! ## Aliases provided
//!
//! | Alias name          | Canonical tool | Notes                              |
//! |---------------------|----------------|------------------------------------|
//! | `view_file`         | `read`         | `path` pass-through                |
//! | `read_file`         | `read`         | `path` pass-through                |
//! | `get_file_contents` | `read`         | `start`/`end` → `start_line`/`end_line` |
//! | `list_files`        | `list`         | `path` pass-through                |
//! | `list_directory`    | `list`         | `directory` → `path`              |
//! | `find_files`        | `glob`         | `pattern`/`path` pass-through     |
//! | `search_in_repo`    | `search`       | `query`/`path` pass-through       |
//! | `file_search`       | `search`       | `query`/`path` pass-through       |
//! | `replace_in_file`   | `edit`         | `old`→`old_str`, `new`→`new_str`  |
//! | `update_file`       | `write`        | `content` pass-through            |
//! | `run_shell_command` | `bash`         | `command` pass-through            |
//! | `run_terminal_cmd`  | `bash`         | `command` pass-through            |
//! | `execute_bash`      | `bash`         | `command` pass-through            |
//! | `execute_code`      | `bash`         | `code` → `command`                |
//! | `run_code`          | `bash`         | `code` → `command`                |

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use super::{bash, edit, glob, list, read, search, write};

// ---------------------------------------------------------------------------
// Helper: build a normalised input Value and delegate to a canonical tool
// ---------------------------------------------------------------------------

async fn delegate(
    tool: &(impl Tool + ?Sized),
    input: Value,
    ctx: &ToolContext,
) -> Result<ToolOutput> {
    tool.execute(input, ctx).await
}

/// Extract a shell command from an input Value, trying multiple common parameter names.
/// Models emit `command`, `code`, or `cmd` (sometimes as an array like `["bash","-c","..."]`).
fn extract_command(input: &mut Value) -> Option<String> {
    // Try `command` first (canonical)
    if let Some(s) = input["command"].as_str() {
        return Some(s.to_string());
    }
    // Then `code`
    if let Some(s) = input["code"].as_str().map(|s| s.to_string()) {
        input["command"] = Value::String(s.clone());
        return Some(s);
    }
    // Then `cmd` — may be a string or an array
    match &input["cmd"] {
        Value::String(s) => {
            let cmd = s.clone();
            input["command"] = Value::String(cmd.clone());
            return Some(cmd);
        }
        Value::Array(arr) => {
            // Join array elements as a shell command via `bash -c`
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !parts.is_empty() {
                let cmd = parts.join(" ");
                input["command"] = Value::String(cmd.clone());
                return Some(cmd);
            }
        }
        _ => {}
    }
    None
}

/// Alias for `read`. Accepts `path` and optional `start_line`/`end_line`.
pub struct ViewFileTool;

#[async_trait::async_trait]
impl Tool for ViewFileTool {
    fn name(&self) -> &'static str {
        "view_file"
    }

    fn description(&self) -> &'static str {
        "Read and display the contents of a file. Alias for 'read'. \
         Use 'path' to specify the file, and optionally 'start_line'/'end_line' \
         to view a specific range."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to read" },
                "start_line": { "type": "integer", "description": "First line to include (1-based)" },
                "end_line": { "type": "integer", "description": "Last line to include (1-based, inclusive)" }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&read::ReadTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// read_file  →  read
// ---------------------------------------------------------------------------

/// Alias for `read`. Identical parameter set.
pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file. Alias for 'read'. \
         Supports optional 'start_line'/'end_line' for range reading."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to read" },
                "start_line": { "type": "integer", "description": "First line to include (1-based)" },
                "end_line": { "type": "integer", "description": "Last line to include (1-based, inclusive)" }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&read::ReadTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// get_file_contents  →  read  (maps start/end → start_line/end_line)
// ---------------------------------------------------------------------------

/// Alias for `read`. Accepts `start`/`end` line parameters in addition to
/// the canonical `start_line`/`end_line` names.
pub struct GetFileContentsTool;

#[async_trait::async_trait]
impl Tool for GetFileContentsTool {
    fn name(&self) -> &'static str {
        "get_file_contents"
    }

    fn description(&self) -> &'static str {
        "Retrieve the contents of a file. Alias for 'read'. \
         Use 'path', and optionally 'start'/'end' (or 'start_line'/'end_line') \
         for a line range."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "start": { "type": "integer", "description": "First line to include (1-based)" },
                "end":   { "type": "integer", "description": "Last line to include (1-based, inclusive)" },
                "start_line": { "type": "integer", "description": "First line (canonical name)" },
                "end_line":   { "type": "integer", "description": "Last line (canonical name)" }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Promote start/end → start_line/end_line if the canonical keys are absent
        if input.get("start_line").is_none() {
            if let Some(v) = input.get("start").cloned() {
                input["start_line"] = v;
            }
        }
        if input.get("end_line").is_none() {
            if let Some(v) = input.get("end").cloned() {
                input["end_line"] = v;
            }
        }
        delegate(&read::ReadTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// list_files  →  list
// ---------------------------------------------------------------------------

/// Alias for `list`. Accepts `path`.
pub struct ListFilesTool;

#[async_trait::async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn description(&self) -> &'static str {
        "List files and directories at a given path. Alias for 'list'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list (default: working directory)" }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&list::ListTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// list_directory  →  list  (maps directory → path)
// ---------------------------------------------------------------------------

/// Alias for `list`. Accepts `directory` or `path`.
pub struct ListDirectoryTool;

#[async_trait::async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory. Alias for 'list'. \
         Use 'directory' or 'path' to specify the target."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "directory": { "type": "string", "description": "Directory path to list" },
                "path":      { "type": "string", "description": "Directory path (alternative parameter name)" }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Promote directory → path if path is absent
        if input.get("path").is_none() {
            if let Some(v) = input.get("directory").cloned() {
                input["path"] = v;
            }
        }
        delegate(&list::ListTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// find_files  →  glob
// ---------------------------------------------------------------------------

/// Alias for `glob`. Accepts `pattern` and optional `path`.
pub struct FindFilesTool;

#[async_trait::async_trait]
impl Tool for FindFilesTool {
    fn name(&self) -> &'static str {
        "find_files"
    }

    fn description(&self) -> &'static str {
        "Find files matching a glob pattern. Alias for 'glob'. \
         Use 'pattern' (e.g. '**/*.rs') and optional 'path' to constrain the search root."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern (e.g. '**/*.rs')" },
                "path":    { "type": "string", "description": "Root directory to search in" }
            },
            "required": ["pattern"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&glob::GlobTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// search_in_repo  →  search
// ---------------------------------------------------------------------------

/// Alias for `search`. Accepts `query` and optional `path`.
pub struct SearchInRepoTool;

#[async_trait::async_trait]
impl Tool for SearchInRepoTool {
    fn name(&self) -> &'static str {
        "search_in_repo"
    }

    fn description(&self) -> &'static str {
        "Search for text or code in the repository. Alias for 'search'. \
         Use 'query' for the pattern and optional 'path' to limit the search scope."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query":            { "type": "string", "description": "Text or regex pattern to search for" },
                "path":             { "type": "string", "description": "Directory or file to search in" },
                "include":          { "type": "string", "description": "Glob filter (e.g. '*.rs')" },
                "case_insensitive": { "type": "boolean", "description": "Case-insensitive search" },
                "max_results":      { "type": "integer", "description": "Max matches to return" }
            },
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&search::SearchTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// file_search  →  search
// ---------------------------------------------------------------------------

/// Alias for `search`. OpenAI Agents SDK naming convention.
pub struct FileSearchTool;

#[async_trait::async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &'static str {
        "file_search"
    }

    fn description(&self) -> &'static str {
        "Search for text patterns in files. Alias for 'search'. \
         Use 'query' for the search term and optional 'path' to scope the search."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query":            { "type": "string", "description": "Text or regex pattern to search for" },
                "path":             { "type": "string", "description": "Directory or file to search in" },
                "include":          { "type": "string", "description": "Glob filter (e.g. '*.py')" },
                "case_insensitive": { "type": "boolean", "description": "Case-insensitive search" },
                "max_results":      { "type": "integer", "description": "Max matches to return" }
            },
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&search::SearchTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// replace_in_file  →  edit  (maps old/new → old_str/new_str)
// ---------------------------------------------------------------------------

/// Alias for `edit`. Accepts `old`/`new` in addition to canonical `old_str`/`new_str`.
pub struct ReplaceInFileTool;

#[async_trait::async_trait]
impl Tool for ReplaceInFileTool {
    fn name(&self) -> &'static str {
        "replace_in_file"
    }

    fn description(&self) -> &'static str {
        "Replace a specific string in a file. Alias for 'edit'. \
         Provide 'path', 'old' (text to find) and 'new' (replacement text). \
         The replacement is exact — include enough context to make it unique."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Path to the file to edit" },
                "old":     { "type": "string", "description": "Exact text to find and replace" },
                "new":     { "type": "string", "description": "Replacement text" },
                "old_str": { "type": "string", "description": "Exact text to find (canonical name)" },
                "new_str": { "type": "string", "description": "Replacement text (canonical name)" }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Promote old/new → old_str/new_str if the canonical keys are absent
        if input.get("old_str").is_none() {
            if let Some(v) = input.get("old").cloned() {
                input["old_str"] = v;
            }
        }
        if input.get("new_str").is_none() {
            if let Some(v) = input.get("new").cloned() {
                input["new_str"] = v;
            }
        }
        delegate(&edit::EditTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// update_file  →  write
// ---------------------------------------------------------------------------

/// Alias for `write`. Accepts `path` and `content`.
pub struct UpdateFileTool;

#[async_trait::async_trait]
impl Tool for UpdateFileTool {
    fn name(&self) -> &'static str {
        "update_file"
    }

    fn description(&self) -> &'static str {
        "Write new content to an existing file, replacing its current contents. \
         Alias for 'write'. Use 'path' and 'content'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Path to the file to update" },
                "content": { "type": "string", "description": "New content to write" }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        delegate(&write::WriteTool, input, ctx).await
    }
}

// ---------------------------------------------------------------------------
// Bash execution aliases
// ---------------------------------------------------------------------------

/// Alias for `bash`. Accepts `command` — same parameter as canonical.
pub struct RunShellCommandTool;

#[async_trait::async_trait]
impl Tool for RunShellCommandTool {
    fn name(&self) -> &'static str {
        "run_shell_command"
    }

    fn description(&self) -> &'static str {
        "Run a shell command and return its output. Alias for 'bash'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command to execute" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            },
            "required": ["command"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}

/// Alias for `bash`. Accepts `command`.
pub struct RunTerminalCmdTool;

#[async_trait::async_trait]
impl Tool for RunTerminalCmdTool {
    fn name(&self) -> &'static str {
        "run_terminal_cmd"
    }

    fn description(&self) -> &'static str {
        "Execute a terminal command. Alias for 'bash'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Terminal command to execute" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            },
            "required": ["command"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}

/// Alias for `bash`. Accepts `command`.
pub struct ExecuteBashTool;

#[async_trait::async_trait]
impl Tool for ExecuteBashTool {
    fn name(&self) -> &'static str {
        "execute_bash"
    }

    fn description(&self) -> &'static str {
        "Execute a bash command. Alias for 'bash'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Bash command to run" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            },
            "required": ["command"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}

/// Alias for `bash`. Accepts `code` (maps to `command`).
pub struct ExecuteCodeTool;

#[async_trait::async_trait]
impl Tool for ExecuteCodeTool {
    fn name(&self) -> &'static str {
        "execute_code"
    }

    fn description(&self) -> &'static str {
        "Execute a code snippet via the shell. Alias for 'bash'. \
         Use 'code' to provide the snippet, or 'command' for a direct shell command."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code":    { "type": "string", "description": "Code snippet to execute" },
                "command": { "type": "string", "description": "Shell command (alternative to 'code')" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}

/// Alias for `bash`. Accepts `code` (maps to `command`).
pub struct RunCodeTool;

#[async_trait::async_trait]
impl Tool for RunCodeTool {
    fn name(&self) -> &'static str {
        "run_code"
    }

    fn description(&self) -> &'static str {
        "Run a code snippet. Alias for 'bash'. \
         Provide the snippet via 'code' or 'command'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code":    { "type": "string", "description": "Code or command to run" },
                "command": { "type": "string", "description": "Shell command (alternative to 'code')" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120)" }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "bash:execute"
    }

    async fn execute(&self, mut input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if extract_command(&mut input).is_none() {
            anyhow::bail!("Missing required 'command', 'code', or 'cmd' parameter");
        }
        delegate(&bash::BashTool, input, ctx).await
    }
}
