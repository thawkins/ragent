//! Auto-fix watcher (spec SESSIONMENU D5, FR-17 to FR-20).
//!
//! A session bound to a pull request (GitHub) or merge request (GitLab)
//! with Auto-fix on is polled on a fixed interval. Each poll reads the
//! request's state and head commit, its CI result for that commit, whether
//! it conflicts with its target, and its review comments. Anything new
//! becomes an [`Event::AutofixTrigger`] carrying a prompt for the session;
//! the TUI sends it as a user message (or queues it when the session is
//! busy).
//!
//! - Dedup: each item (a failed job, the conflict, a comment) fires once per
//!   head commit, recorded in storage (`autofix_events`), so a restart
//!   neither loses nor repeats a trigger, and a second ragent process that
//!   loses the insert race does not wake the session again.
//! - Attempt cap: at most `max_attempts` wake-ups per trigger kind per head
//!   commit; one more pauses Auto-fix and tells the user instead.
//! - Lifecycle: a merged or closed request is reported once and is no
//!   longer polled; the binding stays so `Open PR/MR` still works.
//!
//! Review comments come from the REST API, which has no resolved state on
//! GitHub; dedup on comment id keeps each comment to one wake-up
//! (spec R1).
//!
//! Depends on the GitHub and GitLab clients in `ragent-tools-vcs` and the
//! request helpers in [`crate::project::requests`].

use std::sync::Arc;
use std::time::Duration;

use ragent_storage::{SessionRequest, Storage};
use serde_json::Value;
use tokio::sync::Notify;

use crate::event::{Event, EventBus};
use crate::project::requests::{FetchError, RequestInfo, RequestSource, RequestState};
use crate::project::{ProjectRemote, RemoteKind};

/// Lines of a failed job's log included in a wake-up prompt.
const LOG_TAIL_LINES: usize = 40;
/// Largest job log read, in bytes; only its tail is used.
const LOG_LIMIT_BYTES: usize = 4 * 1024 * 1024;
/// Items per page; the maximum both APIs allow.
const PER_PAGE: usize = 100;

/// What woke a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    /// CI failed for the head commit.
    Ci,
    /// The request conflicts with its target branch.
    Conflict,
    /// A new review comment by someone else.
    Review,
}

impl TriggerKind {
    /// The stored spelling: `ci`, `conflict` or `review`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ci => "ci",
            Self::Conflict => "conflict",
            Self::Review => "review",
        }
    }

    /// Status-line wording, e.g. `CI failed`.
    const fn label(self) -> &'static str {
        match self {
            Self::Ci => "CI failed",
            Self::Conflict => "merge conflict",
            Self::Review => "new review comments",
        }
    }
}

/// One thing a trigger can be about: a failed job, the conflict, or a
/// comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerItem {
    /// Stable id, unique within its kind (e.g. `check:123`, `note:9`).
    pub id: String,
    /// Prompt text describing it.
    pub text: String,
}

/// A request as one poll saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Open, merged or closed.
    pub state: RequestState,
    /// Title.
    pub title: String,
    /// Head commit.
    pub head_sha: String,
    /// Source branch.
    pub source_branch: String,
    /// Items per trigger kind; empty kinds are left out.
    pub items: Vec<(TriggerKind, Vec<TriggerItem>)>,
}

/// Reads request `number` of the repository at `remote`: state, head
/// commit, failed CI jobs, conflict and review comments by anyone but the
/// token's own user.
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] for any other failure of a required
/// call. A job log that cannot be read is left out, not an error.
pub async fn fetch_snapshot(
    source: &RequestSource,
    remote: &ProjectRemote,
    number: u64,
) -> Result<Snapshot, FetchError> {
    match source {
        RequestSource::GitHub(client) => github_snapshot(client, remote, number).await,
        RequestSource::GitLab(client) => gitlab_snapshot(client, remote, number).await,
    }
}

fn api_error(error: &anyhow::Error) -> FetchError {
    let text = format!("{error:#}");
    let lower = text.to_ascii_lowercase();
    if lower.contains("rate limit") || lower.contains(" 429") {
        FetchError::RateLimited(text)
    } else {
        FetchError::Other(text)
    }
}

/// The last [`LOG_TAIL_LINES`] lines of `log`.
fn log_tail(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
}

/// Prompt text for one failed job.
fn job_text(name: &str, log: Option<&str>) -> String {
    match log.map(str::trim).filter(|l| !l.is_empty()) {
        Some(log) => format!("- job `{name}`, log tail:\n```\n{}\n```", log_tail(log)),
        None => format!("- job `{name}`"),
    }
}

/// Prompt text for one review comment.
fn comment_text(author: &str, location: Option<String>, body: &str) -> String {
    let place = location.map(|l| format!(" on `{l}`")).unwrap_or_default();
    let quoted: String = body.lines().map(|l| format!("  > {l}\n")).collect();
    format!("- {author}{place}:\n{}", quoted.trim_end())
}

fn push_kind(
    items: &mut Vec<(TriggerKind, Vec<TriggerItem>)>,
    kind: TriggerKind,
    found: Vec<TriggerItem>,
) {
    if !found.is_empty() {
        items.push((kind, found));
    }
}

async fn github_snapshot(
    client: &ragent_tools_vcs::github::client::GitHubClient,
    remote: &ProjectRemote,
    number: u64,
) -> Result<Snapshot, FetchError> {
    let repo = &remote.path;
    let pull = client
        .get(&format!("/repos/{repo}/pulls/{number}"))
        .await
        .map_err(|e| api_error(&e))?;
    let info = RequestInfo::from_github(&pull)
        .ok_or_else(|| FetchError::Other(format!("unexpected response: {pull}")))?;
    let head_sha = pull["head"]["sha"].as_str().unwrap_or_default().to_string();
    let mut items = Vec::new();
    if info.state != RequestState::Open || head_sha.is_empty() {
        return Ok(snapshot(info, head_sha, items));
    }

    // CI: failed check runs and failed commit statuses for the head commit.
    let mut failed = Vec::new();
    let runs = client
        .get(&format!(
            "/repos/{repo}/commits/{head_sha}/check-runs?per_page={PER_PAGE}"
        ))
        .await
        .map_err(|e| api_error(&e))?;
    for run in runs["check_runs"].as_array().map_or(&[][..], Vec::as_slice) {
        let conclusion = run["conclusion"].as_str().unwrap_or_default();
        if run["status"] != "completed" || !matches!(conclusion, "failure" | "timed_out") {
            continue;
        }
        let Some(id) = run["id"].as_u64() else {
            continue;
        };
        let name = run["name"].as_str().unwrap_or("check");
        // An Actions check run's id is its job id; other apps have no log.
        let log = if run["app"]["slug"] == "github-actions" {
            match client
                .get_bytes_limited(
                    &format!("/repos/{repo}/actions/jobs/{id}/logs"),
                    LOG_LIMIT_BYTES,
                )
                .await
            {
                Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
                Err(e) => {
                    tracing::debug!(error = %e, job = id, "autofix: job log unavailable");
                    None
                }
            }
        } else {
            None
        };
        failed.push(TriggerItem {
            id: format!("check:{id}"),
            text: job_text(name, log.as_deref()),
        });
    }
    let status = client
        .get(&format!("/repos/{repo}/commits/{head_sha}/status"))
        .await
        .map_err(|e| api_error(&e))?;
    for entry in status["statuses"].as_array().map_or(&[][..], Vec::as_slice) {
        let (Some(id), Some("failure" | "error")) = (entry["id"].as_u64(), entry["state"].as_str())
        else {
            continue;
        };
        let name = entry["context"].as_str().unwrap_or("status");
        let detail = entry["description"].as_str().unwrap_or_default();
        failed.push(TriggerItem {
            id: format!("status:{id}"),
            text: format!("- status `{name}`: {detail}"),
        });
    }
    push_kind(&mut items, TriggerKind::Ci, failed);

    if pull["mergeable"] == Value::Bool(false) || pull["mergeable_state"] == "dirty" {
        push_kind(
            &mut items,
            TriggerKind::Conflict,
            vec![TriggerItem {
                id: "conflict".to_string(),
                text: format!(
                    "- `{}` conflicts with `{}`",
                    info.source_branch,
                    pull["base"]["ref"].as_str().unwrap_or("the target branch")
                ),
            }],
        );
    }

    // Review comments by anyone but the token's own user.
    let me = client
        .current_user()
        .await
        .map_err(|e| api_error(&e))?
        .get("login")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut comments = Vec::new();
    let inline = client
        .get(&format!(
            "/repos/{repo}/pulls/{number}/comments?per_page={PER_PAGE}"
        ))
        .await
        .map_err(|e| api_error(&e))?;
    for c in inline.as_array().map_or(&[][..], Vec::as_slice) {
        let author = c["user"]["login"].as_str().unwrap_or_default();
        let Some(id) = c["id"].as_u64().filter(|_| author != me) else {
            continue;
        };
        let location = c["path"].as_str().map(|path| match c["line"].as_u64() {
            Some(line) => format!("{path}:{line}"),
            None => path.to_string(),
        });
        comments.push(TriggerItem {
            id: format!("comment:{id}"),
            text: comment_text(author, location, c["body"].as_str().unwrap_or_default()),
        });
    }
    let reviews = client
        .get(&format!(
            "/repos/{repo}/pulls/{number}/reviews?per_page={PER_PAGE}"
        ))
        .await
        .map_err(|e| api_error(&e))?;
    for r in reviews.as_array().map_or(&[][..], Vec::as_slice) {
        let author = r["user"]["login"].as_str().unwrap_or_default();
        let body = r["body"].as_str().unwrap_or_default().trim();
        let Some(id) = r["id"]
            .as_u64()
            .filter(|_| author != me && !body.is_empty())
        else {
            continue;
        };
        comments.push(TriggerItem {
            id: format!("review:{id}"),
            text: comment_text(author, None, body),
        });
    }
    push_kind(&mut items, TriggerKind::Review, comments);
    Ok(snapshot(info, head_sha, items))
}

async fn gitlab_snapshot(
    client: &ragent_tools_vcs::gitlab::GitLabClient,
    remote: &ProjectRemote,
    number: u64,
) -> Result<Snapshot, FetchError> {
    let project = ragent_tools_vcs::percent::encode_project_path(&remote.path);
    let mr = client
        .get(&format!("/projects/{project}/merge_requests/{number}"))
        .await
        .map_err(|e| api_error(&e))?;
    let info = RequestInfo::from_gitlab(&mr)
        .ok_or_else(|| FetchError::Other(format!("unexpected response: {mr}")))?;
    let head_sha = mr["sha"].as_str().unwrap_or_default().to_string();
    let mut items = Vec::new();
    if info.state != RequestState::Open || head_sha.is_empty() {
        return Ok(snapshot(info, head_sha, items));
    }

    // CI: the failed jobs of the head pipeline, when it is for the head
    // commit and has failed.
    let pipeline = &mr["head_pipeline"];
    let mut failed = Vec::new();
    if pipeline["status"] == "failed" && pipeline["sha"].as_str().is_none_or(|s| s == head_sha) {
        if let Some(pipeline_id) = pipeline["id"].as_u64() {
            let jobs = client
                .get(&format!(
                    "/projects/{project}/pipelines/{pipeline_id}/jobs?scope[]=failed&per_page={PER_PAGE}"
                ))
                .await
                .map_err(|e| api_error(&e))?;
            for job in jobs.as_array().map_or(&[][..], Vec::as_slice) {
                let Some(id) = job["id"].as_u64() else {
                    continue;
                };
                if job["allow_failure"] == Value::Bool(true) {
                    continue;
                }
                let name = job["name"].as_str().unwrap_or("job");
                let log = match client
                    .get_text(&format!("/projects/{project}/jobs/{id}/trace"))
                    .await
                {
                    Ok(log) => Some(log),
                    Err(e) => {
                        tracing::debug!(error = %e, job = id, "autofix: job log unavailable");
                        None
                    }
                };
                failed.push(TriggerItem {
                    id: format!("job:{id}"),
                    text: job_text(name, log.as_deref()),
                });
            }
        }
    }
    push_kind(&mut items, TriggerKind::Ci, failed);

    if mr["detailed_merge_status"] == "conflict" || mr["has_conflicts"] == Value::Bool(true) {
        push_kind(
            &mut items,
            TriggerKind::Conflict,
            vec![TriggerItem {
                id: "conflict".to_string(),
                text: format!(
                    "- `{}` conflicts with `{}`",
                    info.source_branch,
                    mr["target_branch"].as_str().unwrap_or("the target branch")
                ),
            }],
        );
    }

    // Unresolved, resolvable notes by anyone but the token's own user.
    let me = client
        .get("/user")
        .await
        .map_err(|e| api_error(&e))?
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let discussions = client
        .get(&format!(
            "/projects/{project}/merge_requests/{number}/discussions?per_page={PER_PAGE}"
        ))
        .await
        .map_err(|e| api_error(&e))?;
    let mut comments = Vec::new();
    for discussion in discussions.as_array().map_or(&[][..], Vec::as_slice) {
        for note in discussion["notes"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            let author = note["author"]["username"].as_str().unwrap_or_default();
            let open = note["resolvable"] == Value::Bool(true)
                && note["resolved"] != Value::Bool(true)
                && note["system"] != Value::Bool(true);
            let Some(id) = note["id"].as_u64().filter(|_| open && author != me) else {
                continue;
            };
            let position = &note["position"];
            let location =
                position["new_path"]
                    .as_str()
                    .map(|path| match position["new_line"].as_u64() {
                        Some(line) => format!("{path}:{line}"),
                        None => path.to_string(),
                    });
            comments.push(TriggerItem {
                id: format!("note:{id}"),
                text: comment_text(author, location, note["body"].as_str().unwrap_or_default()),
            });
        }
    }
    push_kind(&mut items, TriggerKind::Review, comments);
    Ok(snapshot(info, head_sha, items))
}

fn snapshot(
    info: RequestInfo,
    head_sha: String,
    items: Vec<(TriggerKind, Vec<TriggerItem>)>,
) -> Snapshot {
    Snapshot {
        state: info.state,
        title: info.title,
        head_sha,
        source_branch: info.source_branch,
        items,
    }
}

/// Resolves the API client for a remote; `None` when there are no usable
/// credentials for its host.
pub type SourceResolver = Arc<dyn Fn(&ProjectRemote) -> Option<RequestSource> + Send + Sync>;

/// Polls bound sessions with Auto-fix on and publishes
/// [`Event::AutofixTrigger`] for what is new (spec SESSIONMENU D5).
pub struct AutofixWatcher {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
    sources: SourceResolver,
    max_attempts: u32,
    poll: Duration,
    wake: Arc<Notify>,
}

impl std::fmt::Debug for AutofixWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutofixWatcher")
            .field("max_attempts", &self.max_attempts)
            .field("poll", &self.poll)
            .finish_non_exhaustive()
    }
}

impl AutofixWatcher {
    /// A watcher over `storage`'s bindings.
    ///
    /// # Arguments
    ///
    /// * `storage` - bindings and trigger records.
    /// * `event_bus` - where triggers are published.
    /// * `sources` - API client per remote.
    /// * `config` - `vcs.autofix` settings (poll interval, attempt cap).
    #[must_use]
    pub fn new(
        storage: Arc<Storage>,
        event_bus: Arc<EventBus>,
        sources: SourceResolver,
        config: &ragent_config::VcsAutofixConfig,
    ) -> Self {
        Self {
            storage,
            event_bus,
            sources,
            max_attempts: config.max_attempts(),
            poll: Duration::from_secs(config.poll_secs()),
            wake: Arc::new(Notify::new()),
        }
    }

    /// A handle that makes the running watcher poll now, e.g. after
    /// Auto-fix is turned on.
    #[must_use]
    pub fn wake_handle(&self) -> Arc<Notify> {
        Arc::clone(&self.wake)
    }

    /// Starts polling every interval, and whenever the wake handle is
    /// notified, until the returned task is aborted.
    #[must_use]
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                self.tick().await;
                tokio::select! {
                    () = tokio::time::sleep(self.poll) => {}
                    () = self.wake.notified() => {}
                }
            }
        })
    }

    /// Polls every watched binding once and publishes what is new.
    ///
    /// # Returns
    ///
    /// The events published, in order (for tests and logging).
    pub async fn tick(&self) -> Vec<Event> {
        let bindings = match self.storage.autofix_watch_list() {
            Ok(bindings) => bindings,
            Err(e) => {
                tracing::warn!(error = %e, "autofix: could not read bindings");
                return Vec::new();
            }
        };
        let mut published = Vec::new();
        for binding in bindings {
            for event in self.check(&binding).await {
                self.event_bus.publish(event.clone());
                published.push(event);
            }
        }
        published
    }

    /// The events one binding's poll produces; records what it acts on.
    async fn check(&self, binding: &SessionRequest) -> Vec<Event> {
        let Some(kind) = RemoteKind::parse(&binding.forge) else {
            return Vec::new();
        };
        let remote = ProjectRemote {
            kind,
            host: binding.host.clone(),
            path: binding.repo.clone(),
            url: String::new(),
            remote_name: String::new(),
            default_branch: None,
        };
        let Some(source) = (self.sources)(&remote) else {
            tracing::debug!(session_id = %binding.session_id, "autofix: no credentials");
            return Vec::new();
        };
        let snap = match fetch_snapshot(&source, &remote, binding.number).await {
            Ok(snap) => snap,
            Err(e) => {
                tracing::warn!(error = %e, session_id = %binding.session_id, "autofix: poll failed");
                return Vec::new();
            }
        };
        let session_id = binding.session_id.as_str();
        if let Err(e) = self.storage.record_autofix_poll(
            session_id,
            snap.state.as_str(),
            &snap.title,
            &snap.head_sha,
        ) {
            tracing::warn!(error = %e, "autofix: could not record poll");
            return Vec::new();
        }
        let noun = kind.noun();
        let request = format!("{} {}{}", noun.short, noun.sigil, binding.number);
        if snap.state != RequestState::Open {
            return vec![Event::AutofixTrigger {
                session_id: session_id.to_string(),
                kind: "closed".to_string(),
                summary: format!("{request} {}; auto-fix stopped", snap.state.as_str()),
                prompt: String::new(),
            }];
        }
        let mut events = Vec::new();
        for (trigger, items) in &snap.items {
            match self.wake_for(session_id, &request, &snap, *trigger, items) {
                Ok(Some(event)) => {
                    let paused =
                        matches!(&event, Event::AutofixTrigger { kind, .. } if kind == "paused");
                    events.push(event);
                    if paused {
                        break;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, session_id, "autofix: could not record trigger")
                }
            }
        }
        events
    }

    /// The event for the new `items` of one trigger kind, if any: a wake-up,
    /// or a pause once the attempt cap is reached.
    fn wake_for(
        &self,
        session_id: &str,
        request: &str,
        snap: &Snapshot,
        kind: TriggerKind,
        items: &[TriggerItem],
    ) -> anyhow::Result<Option<Event>> {
        let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let fresh =
            self.storage
                .new_autofix_items(session_id, &snap.head_sha, kind.as_str(), &ids)?;
        if fresh.is_empty() {
            return Ok(None);
        }
        let attempts = self
            .storage
            .autofix_attempts(session_id, &snap.head_sha, kind.as_str())?;
        if attempts >= self.max_attempts {
            self.storage.set_autofix_paused(session_id, true)?;
            return Ok(Some(Event::AutofixTrigger {
                session_id: session_id.to_string(),
                kind: "paused".to_string(),
                summary: format!(
                    "{request} auto-fix paused: {} after {attempts} attempts; needs you",
                    kind.label()
                ),
                prompt: String::new(),
            }));
        }
        let attempt = attempts + 1;
        let summary = format!(
            "{request} auto-fix: {} (attempt {attempt}/{})",
            kind.label(),
            self.max_attempts
        );
        if self.storage.record_autofix_wakeup(
            session_id,
            &snap.head_sha,
            kind.as_str(),
            &fresh,
            &summary,
        )? == 0
        {
            // Another process recorded these first and woke the session.
            return Ok(None);
        }
        let details: Vec<&str> = items
            .iter()
            .filter(|i| fresh.contains(&i.id))
            .map(|i| i.text.as_str())
            .collect();
        Ok(Some(Event::AutofixTrigger {
            session_id: session_id.to_string(),
            kind: kind.as_str().to_string(),
            summary,
            prompt: prompt(request, snap, kind, &details),
        }))
    }
}

/// The user-role message that wakes the session.
fn prompt(request: &str, snap: &Snapshot, kind: TriggerKind, details: &[&str]) -> String {
    let short_sha: String = snap.head_sha.chars().take(10).collect();
    let (what, ask) = match kind {
        TriggerKind::Ci => ("CI failed", "Find the cause, fix it, then commit and push."),
        TriggerKind::Conflict => (
            "the branch has merge conflicts",
            "Merge or rebase onto the target branch, resolve the conflicts, run the \
             tests, then commit and push.",
        ),
        TriggerKind::Review => (
            "there are new review comments",
            "Address each comment with a code change where one is needed, then commit \
             and push. Reply to a comment only if asked.",
        ),
    };
    format!(
        "Auto-fix: {what} on {request} \"{}\" (branch `{}`, commit {short_sha}).\n\n{}\n\n{ask} \
         Never merge the {request}, never force-push, and never resolve review threads.",
        snap.title,
        snap.source_branch,
        details.join("\n")
    )
}
