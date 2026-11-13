//! `autofix_status` - report the pull/merge requests Auto-fix watches and
//! the state of the Auto-fix watcher (spec SESSIONMENU).
//!
//! Read-only. The request list comes from the session bindings in storage;
//! the watcher state comes from the heartbeat the running watcher stores
//! after every poll ([`crate::autofix::WatcherHeartbeat`]), so the tool
//! works in any process that shares the database. The settings come from
//! the session's `vcs.autofix` config.

use anyhow::{Context, Result};
use ragent_storage::{AutofixBinding, Storage};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::autofix::WatcherHeartbeat;
use crate::project::RemoteKind;

/// Read-only tool listing watched Auto-fix requests and the watcher state.
pub struct AutofixStatusTool;

/// Trigger kinds whose attempts are reported.
const KINDS: [&str; 3] = ["ci", "conflict", "review"];

/// `PR #12` / `MR !5`.
fn label(binding: &AutofixBinding) -> String {
    let (short, sigil) =
        RemoteKind::parse(&binding.request.forge).map_or(("request", '#'), |kind| {
            let noun = kind.noun();
            (noun.short, noun.sigil)
        });
    format!("{short} {sigil}{}", binding.request.number)
}

/// Whether the watcher polls this binding: Auto-fix on, not paused, the
/// request open when last seen, the session not archived.
fn is_watched(binding: &AutofixBinding) -> bool {
    binding.state.enabled
        && !binding.state.paused
        && binding.request.state == "open"
        && !binding.archived
}

/// `on`, `off` or `paused`.
fn mode(binding: &AutofixBinding) -> &'static str {
    match (binding.state.enabled, binding.state.paused) {
        (false, _) => "off",
        (true, true) => "paused",
        (true, false) => "on",
    }
}

/// One request as JSON, with its attempt counts at the last-seen commit.
fn request_json(storage: &Storage, binding: &AutofixBinding) -> Value {
    let request = &binding.request;
    let attempts: serde_json::Map<String, Value> = match &binding.state.head_sha {
        Some(sha) => KINDS
            .iter()
            .map(|kind| {
                let n = storage
                    .autofix_attempts(&request.session_id, sha, kind)
                    .unwrap_or_else(|e| {
                        tracing::debug!(error = %e, "autofix_status: attempts unreadable");
                        0
                    });
                ((*kind).to_string(), json!(n))
            })
            .collect(),
        None => serde_json::Map::new(),
    };
    json!({
        "session_id": request.session_id,
        "session_title": binding.session_title,
        "label": label(binding),
        "forge": request.forge,
        "host": request.host,
        "repo": request.repo,
        "number": request.number,
        "title": request.title,
        "url": request.web_url,
        "branch": request.head_branch,
        "state": request.state,
        "autofix": mode(binding),
        "watched": is_watched(binding),
        "archived": binding.archived,
        "head_sha": binding.state.head_sha,
        "last_polled_at": binding.state.last_polled_at,
        "last_trigger": binding.state.last_trigger,
        "attempts": attempts,
    })
}

/// The watcher section as JSON: `running`, `stopped` (a heartbeat older
/// than two intervals) or `never_started`.
fn watcher_json(beat: Option<&WatcherHeartbeat>) -> Value {
    match beat {
        None => json!({ "state": "never_started" }),
        Some(beat) => {
            let state = if beat.is_live(chrono::Utc::now()) {
                "running"
            } else {
                "stopped"
            };
            json!({
                "state": state,
                "pid": beat.pid,
                "started_at": beat.started_at,
                "last_poll_at": beat.last_tick_at,
                "poll_secs": beat.poll_secs,
                "max_attempts": beat.max_attempts,
                "watched_last_poll": beat.watched,
                "triggered_last_poll": beat.triggered,
                "problems": beat.problems,
            })
        }
    }
}

/// The markdown report. It carries every field the tool advertises: only
/// this text reaches the model, not the metadata.
fn render(report: &Value) -> String {
    let watcher = &report["watcher"];
    let config = &report["config"];
    let mut out = String::from("## Auto-fix status\n\n");
    match watcher["state"].as_str().unwrap_or_default() {
        "never_started" => {
            out.push_str("Watcher: never started (it runs inside the ragent TUI).\n")
        }
        state => {
            out.push_str(&format!(
                "Watcher: {state} (pid {}), started {}, last poll {}, every {}s, pauses after \
                 {} attempts; last poll: {} watched, {} triggered\n",
                watcher["pid"],
                watcher["started_at"].as_str().unwrap_or("-"),
                watcher["last_poll_at"].as_str().unwrap_or("-"),
                watcher["poll_secs"],
                watcher["max_attempts"],
                watcher["watched_last_poll"],
                watcher["triggered_last_poll"],
            ));
            for problem in watcher["problems"].as_array().into_iter().flatten() {
                out.push_str(&format!("- problem: {}\n", problem.as_str().unwrap_or("")));
            }
        }
    }
    out.push_str(&format!(
        "Settings: poll every {}s, pause after {} attempts, new bindings start {}\n\n",
        config["poll_secs"],
        config["max_attempts"],
        if config["default_on"] == json!(true) {
            "on"
        } else {
            "off"
        }
    ));
    let requests = report["requests"].as_array().map_or(&[][..], Vec::as_slice);
    if requests.is_empty() {
        out.push_str(if report["all"] == json!(true) {
            "No session is bound to a pull or merge request.\n"
        } else {
            "No request is watched. Pass `all: true` to list every binding.\n"
        });
        return out;
    }
    out.push_str(
        "| Request | Repo | Branch | State | Auto-fix | Last poll | Attempts at head \
         (ci/conflict/review) | Last trigger | Session |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|---|\n");
    let cell = |v: &Value| v.as_str().unwrap_or("-").replace('|', "/");
    for r in requests {
        let session = match r["session_title"].as_str() {
            Some(title) if !title.is_empty() => title.to_string(),
            _ => r["session_id"].as_str().unwrap_or_default().to_string(),
        };
        let attempts = &r["attempts"];
        let attempts = match r["head_sha"].as_str() {
            Some(sha) => format!(
                "{}/{}/{} at {}",
                attempts["ci"].as_u64().unwrap_or(0),
                attempts["conflict"].as_u64().unwrap_or(0),
                attempts["review"].as_u64().unwrap_or(0),
                sha.chars().take(10).collect::<String>()
            ),
            None => "-".to_string(),
        };
        out.push_str(&format!(
            "| [{}]({}) {} | {} | {} | {} | {}{} | {} | {attempts} | {} | {} (`{}`) |\n",
            cell(&r["label"]),
            cell(&r["url"]),
            cell(&r["title"]),
            cell(&r["repo"]),
            cell(&r["branch"]),
            cell(&r["state"]),
            cell(&r["autofix"]),
            if r["archived"] == json!(true) {
                " (archived)"
            } else {
                ""
            },
            cell(&r["last_polled_at"]),
            cell(&r["last_trigger"]),
            session.replace('|', "/"),
            cell(&r["session_id"]),
        ));
    }
    out
}

#[async_trait::async_trait]
impl Tool for AutofixStatusTool {
    fn name(&self) -> &'static str {
        "autofix_status"
    }

    fn description(&self) -> &'static str {
        "Report PR/MR Auto-fix: the pull requests (GitHub) and merge requests (GitLab) \
         that Auto-fix watches, and the state of the Auto-fix watcher. For each request: \
         repo, number, title, URL, branch, state, Auto-fix on/off/paused, last poll, last \
         trigger and attempt counts. For the watcher: running/stopped/never_started, last \
         poll time, interval, attempt cap and problems from its last poll. Read-only. \
         Optional: 'all' (boolean, default false) lists every bound request, not only \
         watched ones."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "all": {
                    "type": "boolean",
                    "description": "List every bound request, including ones with Auto-fix off, paused, merged, closed or archived (default false: only watched ones)"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "system"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_ref()
            .context("Storage is not available in this session.")?;
        let all = input["all"].as_bool().unwrap_or(false);
        let bindings = storage
            .autofix_bindings()
            .context("Could not read Auto-fix bindings")?;
        let requests: Vec<Value> = bindings
            .iter()
            .filter(|b| all || is_watched(b))
            .map(|b| request_json(storage, b))
            .collect();
        let settings = ctx
            .config
            .as_ref()
            .map(|c| c.vcs.autofix.clone())
            .unwrap_or_default();
        let beat = WatcherHeartbeat::load(storage);
        let report = json!({
            "all": all,
            "watcher": watcher_json(beat.as_ref()),
            "config": {
                "poll_secs": settings.poll_secs(),
                "max_attempts": settings.max_attempts(),
                "default_on": settings.default_on(),
            },
            "watched_count": bindings.iter().filter(|b| is_watched(b)).count(),
            "bound_count": bindings.len(),
            "requests": requests,
        });
        Ok(ToolOutput {
            content: render(&report),
            metadata: Some(report),
        })
    }
}
