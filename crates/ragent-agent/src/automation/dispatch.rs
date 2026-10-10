//! Third-party dispatch for automation runs (spec `openhands` T-016; FR-013).
//!
//! When an automation run reaches a terminal state, each configured dispatch
//! target is notified with a provider-shaped JSON payload. The supported kinds
//! are `slack`, `github`, `linear`, `notion`, and `webhook` (a generic JSON
//! POST). Payload construction is pure ([`payload_for`]) so it can be tested
//! without a network; [`dispatch_all`] performs the HTTP POSTs.
//!
//! # Secret safety
//!
//! A target names the environment variable holding its bearer token
//! (`token_env`); [`resolve_token`] reads the value at dispatch time. A token
//! is never written to the run record, the payload, or a log line (FR-035). A
//! target with no resolvable URL (or, for a token-auth provider, no
//! `token_env`) is reported as inert rather than failing the batch.

use ragent_config::DispatchTarget;
use ragent_types::{AutomationRun, RunOutcome};
use serde_json::{Value, json};

/// The outcome of one dispatch attempt (FR-013).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The POST succeeded (2xx).
    Delivered {
        /// The target kind.
        kind: String,
    },
    /// The target declared no usable endpoint (or no token source), so nothing
    /// was sent.
    Inert {
        /// The target kind.
        kind: String,
        /// Why the target was skipped.
        reason: String,
    },
    /// The POST failed (transport error or non-2xx status).
    Failed {
        /// The target kind.
        kind: String,
        /// The failure description (never contains a token value).
        error: String,
    },
}

impl DispatchOutcome {
    /// Whether the dispatch was delivered.
    #[must_use]
    pub const fn is_delivered(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }
}

/// Resolve a target's bearer token from its named environment variable.
///
/// Returns `None` when the target names no variable, or the named variable is
/// absent or blank. The returned value is used only to build an `Authorization`
/// header and is never logged (FR-035).
#[must_use]
pub fn resolve_token(target: &DispatchTarget) -> Option<String> {
    let name = target.token_env.as_deref()?.trim();
    if name.is_empty() {
        return None;
    }
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Build the provider-shaped JSON payload for a target kind (FR-013).
///
/// Provider-specific fields are populated from the target's `target` identifier
/// and the run's summary; an unknown kind falls back to the generic run JSON so
/// the notification still carries the run detail.
#[must_use]
pub fn payload_for(target: &DispatchTarget, automation_id: &str, run: &AutomationRun) -> Value {
    let outcome = run.outcome.map(RunOutcome::as_str).unwrap_or("running");
    let summary = format!(
        "Automation `{automation_id}` run {run_id} ({trigger}) finished with outcome `{outcome}` \
         on backend `{backend}`",
        run_id = run.id,
        trigger = run.trigger,
        backend = run.backend,
    );
    // Shared by the `github` and `linear` providers, which use the same title.
    let title = format!("Automation `{automation_id}` run {outcome}");
    match target.kind_lower().as_str() {
        "slack" => json!({
            "channel": target.target,
            "text": summary,
        }),
        "github" => json!({
            "title": title,
            "body": format!(
                "{summary}\n\nRun id: {}\nTrigger: {}\nBackend: {}\nOutput: {}",
                run.id,
                run.trigger,
                run.backend,
                run.output_ref.as_deref().unwrap_or("(none)"),
            ),
        }),
        "linear" => json!({
            "title": title,
            "description": summary,
            "teamId": target.target,
        }),
        "notion" => json!({
            "parent": { "database_id": target.target },
            "properties": {
                "Name": { "title": [{ "text": { "content": format!("Automation `{automation_id}`") } }] },
                "Outcome": { "rich_text": [{ "text": { "content": outcome } }] },
                "Run": { "rich_text": [{ "text": { "content": run.id.clone() } }] },
            },
        }),
        // `webhook` and any unknown kind: send the generic run envelope.
        _ => json!({
            "automation_id": automation_id,
            "run_id": run.id,
            "trigger": run.trigger.as_str(),
            "outcome": outcome,
            "backend": run.backend,
            "output_ref": run.output_ref,
            "started_at": run.started_at.to_rfc3339(),
            "ended_at": run.ended_at.map(|t| t.to_rfc3339()),
        }),
    }
}

/// The endpoint a target posts to, or `None` when it is inert.
#[must_use]
pub fn endpoint_for(target: &DispatchTarget) -> Option<String> {
    target
        .url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
}

/// Dispatch the run to every configured target (FR-013).
///
/// Each target is POSTed its provider-shaped payload independently; one target's
/// failure does not abort the batch. Returns one outcome per target, in order. The
/// POSTs run concurrently over one shared client, so a slow target does not delay
/// the others and connection pooling is reused across the batch.
pub async fn dispatch_all(
    targets: &[DispatchTarget],
    automation_id: &str,
    run: &AutomationRun,
) -> Vec<DispatchOutcome> {
    if targets.is_empty() {
        return Vec::new();
    }
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => Some(client),
        Err(e) => {
            let reason = format!("could not build HTTP client: {e}");
            return targets
                .iter()
                .map(|target| DispatchOutcome::Failed {
                    kind: target.kind_lower(),
                    error: reason.clone(),
                })
                .collect();
        }
    };
    futures::future::join_all(
        targets
            .iter()
            .map(|target| dispatch_one(client.as_ref(), target, automation_id, run)),
    )
    .await
}

/// Dispatch the run to one target over the shared `client`.
async fn dispatch_one(
    client: Option<&reqwest::Client>,
    target: &DispatchTarget,
    automation_id: &str,
    run: &AutomationRun,
) -> DispatchOutcome {
    let kind = target.kind_lower();
    let Some(url) = endpoint_for(target) else {
        return DispatchOutcome::Inert {
            kind,
            reason: "no `url` configured".to_string(),
        };
    };
    let Some(client) = client else {
        return DispatchOutcome::Failed {
            kind,
            error: "could not build HTTP client".to_string(),
        };
    };
    let payload = payload_for(target, automation_id, run);
    let mut request = client.post(&url).json(&payload);
    if let Some(token) = resolve_token(target) {
        request = request.bearer_auth(token);
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => DispatchOutcome::Delivered { kind },
        Ok(response) => DispatchOutcome::Failed {
            kind,
            error: format!("endpoint returned HTTP {}", response.status().as_u16()),
        },
        Err(e) => DispatchOutcome::Failed {
            kind,
            error: format!("request failed: {e}"),
        },
    }
}
