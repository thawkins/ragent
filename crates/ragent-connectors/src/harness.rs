//! `/connectors test` isolated connect-and-invoke harness (spec `connectors`
//! T-013; FR-015, FR-016).
//!
//! [`test_connector`] exercises one installed connector end-to-end **without
//! touching the live session**:
//!
//! 1. discovery - locate the connector in the configured stores (a manifest
//!    parse only, FR-001);
//! 2. validation - the descriptor's shape, including the recorded
//!    unsupported-capability labels (FR-025);
//! 3. auth resolution - the shape's credential, resolved through the same
//!    [`resolve`] the session uses, so a missing
//!    credential is reported `needs auth` here exactly as `/connectors list`
//!    reports it (FR-022, FR-032);
//! 4. per-server connect - each expressible server is connected **in
//!    isolation** through the [`McpProbe`] seam, which reports the wall-clock
//!    connect time and the tools the server advertised;
//! 5. one sample invocation per server - the first advertised tool is invoked
//!    once with schema-valid sample arguments generated from its JSON schema
//!    (FR-015);
//! 6. disconnect - every server connected in step 4 is torn down, so nothing
//!    the harness started survives the run.
//!
//! Every step is reported as one `[ ok ]`/`[fail]` line carrying its wall-clock
//! time; a step failure records its cause and the harness continues to the
//! disconnect step rather than leaking a connection (FR-016).
//!
//! # Why a dedicated seam
//!
//! The crate must not depend on `ragent-agent` (the MCP client), and the live
//! session's `McpConnect` (crate::lifecycle) only returns tool
//! *names*, not the JSON schemas the sample-argument step needs. [`McpProbe`]
//! therefore carries the three operations the harness performs
//! (connect+list-schemas, invoke, disconnect) as a seam the session implements
//! over the live `McpClient`. The harness never borrows a session, so it cannot
//! observe or mutate anything the session loaded (FR-015).
//!
//! The module is modelled on `ragent_plugins::harness` so the two `/test`
//! surfaces read the same way.

use std::path::Path;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use ragent_config::{ConnectorsConfig, McpServerConfig};
use serde_json::Value as JsonValue;

use crate::auth::{AuthRequirement, CredentialStore, EnvSource, resolve, secret_value};
use crate::bridge::resolve_servers;
use crate::descriptor::ConnectorDescriptor;
use crate::help::attribution;
use crate::store::{StoreDirs, scan_dirs, store_dirs};

/// One tool a probed server advertised: the name to invoke and the JSON schema
/// the harness generates schema-valid sample arguments from (FR-015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeTool {
    /// The tool name the server advertises (invoked verbatim).
    pub name: String,
    /// The tool's `inputSchema`, used by [`sample_for_schema`].
    pub parameters: JsonValue,
}

/// The isolated MCP seam the harness drives (FR-015).
///
/// Implemented by the session over the live `McpClient`: `probe_connect`
/// connects a *throwaway* server and lists its tools with their schemas,
/// `probe_call` invokes one tool, and `probe_disconnect` drops the connection.
/// Every method is contained - a failure is an error string, never a panic
/// (FR-016).
#[async_trait]
pub trait McpProbe: Send {
    /// Connect `server_id` with `config` in isolation and return the tools it
    /// advertised, each with the schema its arguments are generated from.
    ///
    /// # Errors
    ///
    /// Returns the connection failure cause when the server cannot be reached.
    async fn probe_connect(
        &mut self,
        server_id: &str,
        config: McpServerConfig,
    ) -> Result<Vec<ProbeTool>, String>;

    /// Invoke `tool` on `server_id` with `args` and return a short rendering of
    /// the result.
    ///
    /// # Errors
    ///
    /// Returns the server's own error message when the call fails (FR-016).
    async fn probe_call(
        &mut self,
        server_id: &str,
        tool: &str,
        args: JsonValue,
    ) -> Result<String, String>;

    /// Drop the isolated connection to `server_id`.
    ///
    /// # Errors
    ///
    /// Returns the teardown failure cause when the connection cannot be
    /// dropped.
    async fn probe_disconnect(&mut self, server_id: &str) -> Result<(), String>;
}

/// Outcome of one harness step (FR-015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step completed successfully.
    Pass,
    /// The step failed; the string is the cause (SPEC error-handling policy).
    Fail(String),
}

/// One reported harness step: a name, its outcome, the wall-clock time it took,
/// and an optional detail line (the server's tool set, the generated sample
/// arguments, the invocation result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessStep {
    /// Step label (`discovery`, `validation`, `auth`, `connect <server>`,
    /// `sample invocation <tool>` on `<server>`, `disconnect <server>`).
    pub name: String,
    /// Pass/fail plus the cause on failure.
    pub outcome: StepOutcome,
    /// Wall-clock time spent in this step.
    pub elapsed: Duration,
    /// Optional human-readable detail appended to the rendered line.
    pub detail: Option<String>,
}

/// Full result of a `/connectors test` run (FR-015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessReport {
    /// Connector id under test.
    pub connector_id: String,
    /// Ordered steps that ran; a failure records its cause and the sequence
    /// continues to the teardown steps.
    pub steps: Vec<HarnessStep>,
    /// Unsupported-capability labels from the descriptor (FR-025).
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

    /// The step with `name`, if it ran.
    #[must_use]
    pub fn step(&self, name: &str) -> Option<&HarnessStep> {
        self.steps.iter().find(|step| step.name == name)
    }
}

/// Run the isolated harness over the connector with `connector_id` discovered
/// in `dirs` (FR-015). All failures are contained as step outcomes; this
/// function never panics, never writes the store ledger, and never touches the
/// live session.
pub async fn test_connector(
    dirs: StoreDirs,
    _config: &ConnectorsConfig,
    credentials: &dyn CredentialStore,
    env: &dyn EnvSource,
    probe: &mut dyn McpProbe,
    connector_id: &str,
) -> HarnessReport {
    let found = scan_dirs(dirs)
        .into_iter()
        .find(|connector| match &connector.outcome {
            Ok(descriptor) => descriptor.id.as_str() == connector_id,
            Err(_) => false,
        });

    let Some(scanned) = found else {
        return HarnessReport {
            connector_id: connector_id.to_string(),
            steps: vec![step(
                "discovery",
                StepOutcome::Fail(format!("unknown connector: {connector_id}")),
                Duration::ZERO,
                None,
            )],
            unsupported: Vec::new(),
        };
    };

    let started = Instant::now();
    let mut descriptor = match scanned.outcome {
        Ok(descriptor) => descriptor,
        Err(failure) => {
            return HarnessReport {
                connector_id: connector_id.to_string(),
                steps: vec![step(
                    "discovery",
                    StepOutcome::Fail(failure.error.to_string()),
                    started.elapsed(),
                    Some(scanned.dir.display().to_string()),
                )],
                unsupported: Vec::new(),
            };
        }
    };
    let mut report = HarnessReport {
        connector_id: connector_id.to_string(),
        steps: vec![step(
            "discovery",
            StepOutcome::Pass,
            started.elapsed(),
            Some(scanned.dir.display().to_string()),
        )],
        unsupported: Vec::new(),
    };

    // Validation: an unexpressible connector records its cause (FR-025).
    let started = Instant::now();
    if let Err(error) = descriptor.validate() {
        report.steps.push(step(
            "validation",
            StepOutcome::Fail(error.to_string()),
            started.elapsed(),
            None,
        ));
        report.unsupported = descriptor.unsupported.clone();
        return report;
    }
    report.unsupported = descriptor.unsupported.clone();
    let served = descriptor.supported_servers().len();
    report.steps.push(step(
        "validation",
        StepOutcome::Pass,
        started.elapsed(),
        Some(format!(
            "{served} expressible server(s); auth shape {}",
            descriptor.auth
        )),
    ));

    // Auth: a shape with no valid credential refuses the connect and is not
    // retried (FR-022, FR-032), mirroring the session lifecycle gate.
    let started = Instant::now();
    let auth = match resolve(&descriptor, credentials, env) {
        Ok(auth) => auth,
        Err(error) => {
            tracing::warn!(
                connector = %descriptor.id,
                error = %error,
                "connector test harness could not resolve the auth state; reporting auth failed"
            );
            crate::auth::AuthState::Failed
        }
    };
    let requirement = AuthRequirement::for_descriptor(&descriptor);
    if !auth.permits_connect() {
        let guidance = requirement.guidance();
        let cause = if guidance.is_empty() {
            "auth failed".to_string()
        } else {
            format!("auth failed: {guidance}")
        };
        report.steps.push(step(
            "auth",
            StepOutcome::Fail(cause),
            started.elapsed(),
            Some(requirement.describe()),
        ));
        return report;
    }
    report.steps.push(step(
        "auth",
        StepOutcome::Pass,
        started.elapsed(),
        Some(format!("{} ({})", auth.label(), requirement.describe())),
    ));

    // Each expressible server is connected, exercised, and torn down
    // independently, so one failing server cannot leak a connection or hide
    // another server's result (FR-016, FR-026).
    // A credential-store read fault is contained as "no secret": the probe then
    // exercises the server without a credential rather than aborting the whole
    // harness (FR-016). The fault is logged so it is not silent.
    let secret = match secret_value(&descriptor, credentials, env) {
        Ok(secret) => secret,
        Err(error) => {
            tracing::warn!(
                connector = %descriptor.id,
                error = %error,
                "connector test harness could not read the credential; probing without a secret"
            );
            None
        }
    };
    // The isolation guarantee (FR-015): the probe's server ids live in a
    // dedicated namespace, so a bridged id can never collide with a live-session
    // MCP key. No configured-id surface is consulted and no collision is
    // expected.
    let plan = resolve_servers(std::iter::once(&descriptor), std::iter::empty());
    for refused in &plan.refused {
        report.steps.push(step(
            format!("connect {}", refused.server_id),
            StepOutcome::Fail(refused.reason.describe()),
            Duration::ZERO,
            None,
        ));
    }

    for server in &plan.servers {
        let bridged = server.server_id.clone();
        let config = with_secret(&server.config, &descriptor, secret.as_deref());

        let started = Instant::now();
        let tools = match probe.probe_connect(&bridged, config).await {
            Ok(tools) => {
                report.steps.push(step(
                    format!("connect {bridged}"),
                    StepOutcome::Pass,
                    started.elapsed(),
                    Some(format!("{} tool(s) advertised", tools.len())),
                ));
                tools
            }
            Err(cause) => {
                report.steps.push(step(
                    format!("connect {bridged}"),
                    StepOutcome::Fail(cause),
                    started.elapsed(),
                    None,
                ));
                continue;
            }
        };

        // Invoke one advertised tool once (FR-015); a server that advertises no
        // tool has nothing to invoke and is reported distinctly, not failed.
        let started = Instant::now();
        match tools.first() {
            None => report.steps.push(step(
                format!("sample invocation on {bridged}"),
                StepOutcome::Pass,
                started.elapsed(),
                Some("server advertises no tool; nothing to invoke".to_string()),
            )),
            Some(tool) => {
                let args = sample_for_schema(&tool.parameters);
                let (outcome, detail) =
                    match probe.probe_call(&bridged, &tool.name, args.clone()).await {
                        Ok(result) => (
                            StepOutcome::Pass,
                            format!(
                                "tool {} args: {args}; result: {}",
                                tool.name,
                                truncate(&result, 80)
                            ),
                        ),
                        Err(cause) => (
                            StepOutcome::Fail(cause),
                            format!("tool {} args: {args}", tool.name),
                        ),
                    };
                report.steps.push(step(
                    format!("sample invocation {} on {bridged}", tool.name),
                    outcome,
                    started.elapsed(),
                    Some(detail),
                ));
            }
        }

        // Teardown the isolated connection (FR-015). A failed disconnect is
        // reported, not fatal.
        let started = Instant::now();
        match probe.probe_disconnect(&bridged).await {
            Ok(()) => report.steps.push(step(
                format!("disconnect {bridged}"),
                StepOutcome::Pass,
                started.elapsed(),
                None,
            )),
            Err(cause) => report.steps.push(step(
                format!("disconnect {bridged}"),
                StepOutcome::Fail(cause),
                started.elapsed(),
                None,
            )),
        }
    }

    report
}

/// Inject the resolved secret into a server's configuration for the duration of
/// the probe (FR-005), so an authenticated connector is exercised the same way
/// the session would connect it. No secret is ever written to a store or
/// report.
fn with_secret(
    config: &McpServerConfig,
    descriptor: &ConnectorDescriptor,
    secret: Option<&str>,
) -> McpServerConfig {
    let Some(secret) = secret else {
        return config.clone();
    };
    let requirement = AuthRequirement::for_descriptor(descriptor);
    let mut out = config.clone();
    match requirement.env_var {
        Some(name) => {
            out.env.insert(name, secret.to_string());
        }
        None => {
            if out.headers.is_empty() {
                out.headers
                    .insert("Authorization".to_string(), format!("Bearer {secret}"));
            }
        }
    }
    out
}

/// Generate a schema-valid sample value for a tool's `inputSchema` (FR-015).
///
/// Honours `const`, `default`, `examples`, and `enum` when present, otherwise
/// produces a typed placeholder for `type`, recursing into objects and arrays.
/// Mirrors `ragent_plugins::harness::sample_for_schema` so the two `/test`
/// surfaces generate the same arguments for the same schema.
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

/// Render a [`HarnessReport`] as the `/connectors test` message body (FR-015):
/// one `[ ok ]`/`[fail]` line per step with wall-clock milliseconds, the
/// recorded unsupported capabilities (FR-025), and a summary line.
#[must_use]
pub fn render_report(report: &HarnessReport) -> String {
    let mut lines = vec![
        attribution(&format!("test {}", report.connector_id)),
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
            (StepOutcome::Fail(cause), _) => line.push_str(&format!(" - {cause}")),
            (StepOutcome::Pass, Some(detail)) => line.push_str(&format!(" - {detail}")),
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

    lines.push(String::new());
    let marker = if failed == 0 { "[ ok ]" } else { "[fail]" };
    lines.push(format!(
        "{marker} harness complete: {passed} step(s) passed, {failed} failed in {} ms; \
         disconnected, live session untouched.",
        total.as_millis()
    ));
    lines.join("\n")
}

/// Why `/connectors test` could not run: the subsystem's master switch is off
/// (FR-021).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessError {
    /// The connector subsystem is disabled; the harness is refused so no MCP
    /// connection is attempted (FR-021).
    Disabled,
}

impl HarnessError {
    /// Render the refusal report (ASCII only, FR-006).
    #[must_use]
    pub fn report(self, connector_id: &str) -> String {
        match self {
            Self::Disabled => format!(
                "{}\n\n[err] Connector subsystem is disabled \
                 (connectors.enabled = false); no connector was tested.",
                attribution(&format!("test {connector_id}"))
            ),
        }
    }
}

/// Resolve the store directories for `workdir` and run the harness for
/// `connector_id`, returning the report string, or [`HarnessError::Disabled`]
/// when the master switch is off (FR-015, FR-021).
///
/// This is the entry point the shared dispatcher
/// (crate::commands::run_connector_subcommand)
/// calls; the caller supplies the isolated [`McpProbe`], the credential store,
/// and the environment.
pub async fn run_harness(
    workdir: &Path,
    config: &ConnectorsConfig,
    credentials: &dyn CredentialStore,
    env: &dyn EnvSource,
    probe: &mut dyn McpProbe,
    connector_id: &str,
) -> Result<String, HarnessError> {
    if !config.is_enabled() {
        return Err(HarnessError::Disabled);
    }
    let dirs = store_dirs(workdir, config.store_dir.as_deref());
    Ok(render_report(
        &test_connector(dirs, config, credentials, env, probe, connector_id).await,
    ))
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
///
/// A single char-iteration: `char_indices().nth(max)` is `Some` only when the
/// text is longer than `max`, so no separate length pass is needed.
fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((idx, _)) => format!("{}...", &text[..idx]),
        None => text.to_string(),
    }
}
