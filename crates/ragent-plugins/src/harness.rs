//! `/plugins test` isolated harness (spec `plugins` T-015; FR-013, FR-026).
//!
//! [`test_plugin`] exercises one installed plugin end-to-end **without touching
//! the live session**:
//!
//! 1. discovery — locate the plugin directory in the configured stores
//!    (manifest parse only, FR-023);
//! 2. manifest validation — the parse outcome from discovery;
//! 3. version check — refuse a plugin declaring a newer host-API version
//!    (FR-019);
//! 4. entry execution — checkout a **fresh** sandbox under the entry budget
//!    (FR-017) and evaluate the entry point with the host API installed against
//!    a captured [`HostCalls`] sink (a stubbed host API: registrations and
//!    messages are collected, never wired into any registry);
//! 5. per-tool sample invocation — invoke every contributed tool once with
//!    schema-valid sample arguments generated from the tool's JSON schema.
//!
//! Every step is reported as one `[ ok ]`/`[fail]` line with its wall-clock
//! time; a failure carries its cause and aborts the remaining steps (FR-013).
//! The sandbox context is dropped when [`test_plugin`] returns, so nothing the
//! plugin registered survives the run (FR-026). No store state (ledger, enable
//! flag) is written.
//!
//! The harness owns its [`RuntimePool`] and never borrows a
//! [`PluginManager`](crate::lifecycle::PluginManager), so it cannot observe or
//! mutate anything the live session loaded.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde_json::Value as JsonValue;

use crate::error::PluginError;
use crate::help::attribution;
use crate::host_api::{HostApiInstall, HostCalls, PluginMessage};
use crate::lifecycle::scan_id;
use crate::manifest::{HOST_API_VERSION, ParsedManifest, PluginToolDecl, check_api_version};
use crate::runtime::{RuntimePool, SandboxBudget, SandboxContext};
use crate::store::{StoreDirs, scan_dirs};
use crate::tool_adapter::dispatch_sandbox;

/// Outcome of one harness step (FR-013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step completed successfully.
    Pass,
    /// The step failed; the string is the cause (SPEC error-handling policy).
    Fail(String),
}

/// One reported harness step: a name, its outcome, the wall-clock time it took,
/// and an optional detail line (contributed names, generated sample arguments,
/// invocation result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessStep {
    /// Step label (`discovery`, `manifest validation`, `version check`,
    /// `entry execution`, `sample invocation <tool>`).
    pub name: String,
    /// Pass/fail plus the cause on failure.
    pub outcome: StepOutcome,
    /// Wall-clock time spent in this step.
    pub elapsed: Duration,
    /// Optional human-readable detail appended to the rendered line.
    pub detail: Option<String>,
}

/// Full result of a `/plugins test` run (FR-013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessReport {
    /// Plugin id under test.
    pub plugin_id: String,
    /// Ordered steps that ran; the first failure ends the sequence.
    pub steps: Vec<HarnessStep>,
    /// Messages the plugin emitted through `ragent.message.*` (FR-013 captured
    /// sink).
    pub messages: Vec<PluginMessage>,
    /// Unsupported-capability labels from the manifest, reported per FR-025.
    pub unsupported: Vec<String>,
}

impl HarnessReport {
    /// Whether every executed step passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.steps
            .iter()
            .all(|step| step.outcome == StepOutcome::Pass)
    }
}

/// Run the isolated harness over the plugin with `plugin_id` discovered in
/// `dirs` (FR-013). All failures are contained as step outcomes; this function
/// never panics and never touches the live session or the store ledger
/// (FR-026).
#[must_use]
pub fn test_plugin(
    dirs: StoreDirs,
    config: &ragent_config::PluginsConfig,
    plugin_id: &str,
) -> HarnessReport {
    let found = scan_dirs(dirs)
        .into_iter()
        .find(|plugin| scan_id(plugin) == plugin_id);

    let Some(plugin) = found else {
        return HarnessReport {
            plugin_id: plugin_id.to_string(),
            steps: vec![step(
                "discovery",
                StepOutcome::Fail(format!("unknown plugin: {plugin_id}")),
                Duration::ZERO,
                None,
            )],
            messages: Vec::new(),
            unsupported: Vec::new(),
        };
    };

    let mut report = HarnessReport {
        plugin_id: plugin_id.to_string(),
        steps: vec![step(
            "discovery",
            StepOutcome::Pass,
            Duration::ZERO,
            Some(plugin.dir.display().to_string()),
        )],
        messages: Vec::new(),
        unsupported: Vec::new(),
    };

    let parsed = match &plugin.outcome {
        Ok(parsed) => parsed,
        Err(failure) => {
            report.steps.push(step(
                "manifest validation",
                StepOutcome::Fail(failure.error.to_string()),
                Duration::ZERO,
                None,
            ));
            return report;
        }
    };
    report.unsupported = parsed.descriptor.unsupported_capabilities.clone();

    report.steps.push(step(
        "manifest validation",
        StepOutcome::Pass,
        Duration::ZERO,
        Some(format!(
            "id {}, dialect {}, version {}; agents {}, hooks {}",
            parsed.descriptor.id,
            parsed.descriptor.dialect,
            parsed.descriptor.version,
            parsed.agents.len(),
            parsed.hooks.len()
        )),
    ));

    if let Err(mismatch) = check_api_version(parsed.descriptor.api_version, HOST_API_VERSION) {
        report.steps.push(step(
            "version check",
            StepOutcome::Fail(mismatch.to_string()),
            Duration::ZERO,
            None,
        ));
        return report;
    }
    report.steps.push(step(
        "version check",
        StepOutcome::Pass,
        Duration::ZERO,
        Some(format!(
            "declared v{}, host v{HOST_API_VERSION}",
            parsed.descriptor.api_version
        )),
    ));

    // Entry execution against a fresh sandbox and a captured sink.
    let budget = SandboxBudget::from_config(config)
        .with_deadline(Duration::from_millis(config.max_entry_ms.max(1)));
    let calls = HostCalls::new();

    let started = Instant::now();
    let context = match run_entry(parsed, budget, &calls) {
        Ok(context) => context,
        Err(cause) => {
            report.steps.push(step(
                "entry execution",
                StepOutcome::Fail(cause),
                started.elapsed(),
                None,
            ));
            report.messages = calls.messages();
            return report;
        }
    };
    // A non-JS plugin (skill-only / MCP-only) has no entry point to execute:
    // the step passes with a note and there are no tools to sample-invoke.
    let Some(context) = context else {
        report.steps.push(step(
            "entry execution",
            StepOutcome::Pass,
            started.elapsed(),
            Some("no entry point (non-JS plugin)".to_string()),
        ));
        report.messages = calls.messages();
        return report;
    };
    report.steps.push(step(
        "entry execution",
        StepOutcome::Pass,
        started.elapsed(),
        None,
    ));

    // One sample invocation per contributed tool (manifest + register_tool),
    // deduplicated by name so a manifest-declared tool that also registers a
    // handler is not invoked twice.
    for decl in contributed_tools(parsed.tools.clone(), calls.tools()) {
        let args = sample_for_schema(&decl.parameters);
        let started = Instant::now();
        let (outcome, detail) = match dispatch_sandbox(&context, plugin_id, &decl.name, &args) {
            Ok(output) => (
                StepOutcome::Pass,
                format!("args: {args}; result: {}", truncate(&output.content, 80)),
            ),
            Err(err) => (StepOutcome::Fail(cause_of(&err)), format!("args: {args}")),
        };
        report.steps.push(step(
            format!("sample invocation {}", decl.name),
            outcome,
            started.elapsed(),
            Some(detail),
        ));
    }

    report.messages = calls.messages();
    report
}

/// Check out a fresh sandbox, install the captured host API, and evaluate the
/// entry point under the entry budget (FR-013, FR-017, FR-026). Mirrors the
/// lifecycle load procedure but is bound to a throwaway context.
///
/// Returns `Ok(None)` for a non-JS plugin (no entry point): there is nothing to
/// execute, and the caller reports the step as a pass with a note.
fn run_entry(
    parsed: &ParsedManifest,
    budget: SandboxBudget,
    calls: &HostCalls,
) -> Result<Option<SandboxContext>, String> {
    let Some(entry) = parsed.descriptor.entry.as_ref() else {
        return Ok(None);
    };
    let pool = RuntimePool::new();
    let context = pool
        .checkout(budget)
        .map_err(|e| format!("entry: engine checkout failed: {e}"))?;
    let install = HostApiInstall::new(&parsed.descriptor.id, parsed.descriptor.root.clone())
        .with_calls(calls.clone());
    context
        .install_host_api(&install)
        .map_err(|e| format!("entry: host API install failed: {e}"))?;
    let source =
        std::fs::read_to_string(entry).map_err(|e| format!("entry: {}: {e}", entry.display()))?;
    context.reset_interrupt();
    context
        .eval(&source)
        .map_err(|e| format!("entry: {}", cause_of(&e)))?;
    Ok(Some(context))
}

/// Merge manifest-declared and `register_tool`-declared tools, keeping the
/// first occurrence of each name.
fn contributed_tools(
    manifest: Vec<PluginToolDecl>,
    registered: Vec<PluginToolDecl>,
) -> Vec<PluginToolDecl> {
    let mut seen = BTreeSet::new();
    manifest
        .into_iter()
        .chain(registered)
        .filter(|decl| seen.insert(decl.name.clone()))
        .collect()
}

/// Generate a schema-valid sample value for a tool's `parameters` JSON schema
/// (FR-013). Honours `const`, `default`, `examples`, and `enum` when present,
/// otherwise produces a typed placeholder for `type`, recursing into objects
/// and arrays.
#[must_use]
pub fn sample_for_schema(schema: &JsonValue) -> JsonValue {
    match schema {
        JsonValue::Object(map) => {
            for key in ["const", "default"] {
                if let Some(value) = map.get(key) {
                    return value.clone();
                }
            }
            for key in ["examples", "enum"] {
                if let Some(first) = map
                    .get(key)
                    .and_then(JsonValue::as_array)
                    .and_then(|values| values.first())
                {
                    return first.clone();
                }
            }
            match schema_type(map) {
                "object" => {
                    let mut out = serde_json::Map::new();
                    if let Some(properties) = map.get("properties").and_then(JsonValue::as_object) {
                        for (name, sub) in properties {
                            out.insert(name.clone(), sample_for_schema(sub));
                        }
                    }
                    JsonValue::Object(out)
                }
                "array" => match map.get("items") {
                    Some(items) => JsonValue::Array(vec![sample_for_schema(items)]),
                    None => JsonValue::Array(Vec::new()),
                },
                "string" => JsonValue::String("sample".to_string()),
                "integer" => JsonValue::from(0),
                "number" => JsonValue::from(0.0),
                "boolean" => JsonValue::Bool(false),
                "null" => JsonValue::Null,
                _ => JsonValue::Null,
            }
        }
        JsonValue::Bool(_) => JsonValue::Bool(false),
        JsonValue::Number(_) => JsonValue::from(0),
        JsonValue::String(_) => JsonValue::String("sample".to_string()),
        JsonValue::Array(_) => JsonValue::Array(Vec::new()),
        JsonValue::Null => JsonValue::Null,
    }
}

/// The `type` keyword of a JSON schema object, tolerating the array form
/// (`"type": ["string", "null"]`) by taking the first string entry.
fn schema_type(map: &serde_json::Map<String, JsonValue>) -> &str {
    match map.get("type") {
        Some(JsonValue::String(name)) => name.as_str(),
        Some(JsonValue::Array(names)) => {
            names.iter().find_map(JsonValue::as_str).unwrap_or("object")
        }
        _ => "object",
    }
}

/// Render a [`HarnessReport`] as the `/plugins test` message body (FR-013): one
/// `[ ok ]`/`[fail]` line per step with wall-clock milliseconds, unsupported
/// capabilities (FR-025), the captured messages, and a summary line.
#[must_use]
pub fn render_report(report: &HarnessReport) -> String {
    let mut lines = vec![
        attribution(&format!("test {}", report.plugin_id)),
        String::new(),
    ];

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut total = Duration::ZERO;
    for item in &report.steps {
        total += item.elapsed;
        let marker = match &item.outcome {
            StepOutcome::Pass => {
                passed += 1;
                "[ ok ]"
            }
            StepOutcome::Fail(_) => {
                failed += 1;
                "[fail]"
            }
        };
        let mut line = format!("{marker} {} ({} ms)", item.name, item.elapsed.as_millis());
        match (&item.outcome, &item.detail) {
            (StepOutcome::Fail(cause), _) => {
                line.push_str(&format!(" - {cause}"));
            }
            (StepOutcome::Pass, Some(detail)) => {
                line.push_str(&format!(" - {detail}"));
            }
            (StepOutcome::Pass, None) => {}
        }
        lines.push(line);
    }

    if !report.unsupported.is_empty() {
        lines.push(String::new());
        lines.push("Unsupported capabilities:".to_string());
        for capability in &report.unsupported {
            lines.push(format!("- {capability}"));
        }
    }

    if !report.messages.is_empty() {
        lines.push(String::new());
        lines.push(format!("Captured messages ({}):", report.messages.len()));
        for message in &report.messages {
            lines.push(format!("- {}: {}", message.level, message.text));
        }
    }

    lines.push(String::new());
    let marker = if failed == 0 { "[ ok ]" } else { "[fail]" };
    let summary = if failed == 0 {
        format!(
            "{marker} harness complete: {passed} step(s) passed in {} ms; \
             unloaded, live session untouched.",
            total.as_millis()
        )
    } else {
        format!(
            "{marker} harness aborted: {passed} passed, {failed} failed in {} ms; \
             unloaded, live session untouched.",
            total.as_millis()
        )
    };
    lines.push(summary);
    lines.join("\n")
}

/// Why `/plugins test` could not be parsed (reported as an `[err]` row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestArgError {
    /// `test` was given no plugin id.
    MissingTestId,
}

impl TestArgError {
    /// Render the usage error.
    #[must_use]
    pub fn report(self, sub: &str) -> String {
        match self {
            Self::MissingTestId => format!(
                "{}\n\n[err] Missing <pluginid>.\n\nUsage: `/plugins test <pluginid>`",
                attribution(sub)
            ),
        }
    }
}

/// Parse `/plugins <sub> <args>` for the `test` subcommand.
///
/// Returns `None` when `sub` is not `test` (the caller falls through to the
/// other subcommands), `Some(Err(_))` for a missing id, and
/// `Some(Ok(plugin_id))` for a valid command.
#[must_use]
pub fn parse_test_command(sub: &str, args: &str) -> Option<Result<String, TestArgError>> {
    if sub != "test" {
        return None;
    }
    Some(match args.split_whitespace().next() {
        Some(id) => Ok(id.to_string()),
        None => Err(TestArgError::MissingTestId),
    })
}

/// Parse and run the `test` subcommand, returning the report string (or `None`
/// when `sub` is not `test`). Refusals render as `[err]` text; nothing is ever
/// returned as an `Err` (FR-026). When the subsystem is disabled the harness is
/// refused so no plugin code executes (FR-013, acceptance criterion 8).
#[must_use]
pub fn run_test_command(
    dirs: StoreDirs,
    config: &ragent_config::PluginsConfig,
    sub: &str,
    args: &str,
) -> Option<String> {
    let parsed = parse_test_command(sub, args)?;
    Some(match parsed {
        Ok(plugin_id) => {
            if config.enabled {
                render_report(&test_plugin(dirs, config, &plugin_id))
            } else {
                format!(
                    "{}\n\n[err] Plugin subsystem is disabled \
                     (plugins.enabled = false); no plugin code was executed.",
                    attribution(&format!("test {plugin_id}"))
                )
            }
        }
        Err(arg_err) => arg_err.report(sub),
    })
}

/// Map a plugin error to its cause string (SPEC error handling: timeout and
/// memory-limit aborts name the resource; script errors carry the JS detail).
fn cause_of(error: &PluginError) -> String {
    match error {
        PluginError::Timeout => "timeout: plugin execution timed out".to_string(),
        PluginError::MemoryLimit => "memory-limit: plugin memory limit exceeded".to_string(),
        PluginError::Script { detail, .. } => format!("script: {detail}"),
        other => other.to_string(),
    }
}

/// Build a step record.
fn step(
    name: impl Into<String>,
    outcome: StepOutcome,
    elapsed: Duration,
    detail: Option<String>,
) -> HarnessStep {
    HarnessStep {
        name: name.into(),
        outcome,
        elapsed,
        detail,
    }
}

/// Truncate `text` to at most `max` characters for the report.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}...")
    }
}
