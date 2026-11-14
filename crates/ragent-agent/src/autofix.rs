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
//! - Stale results: a poll's result is stored and acted on only while the
//!   session is still bound to the same request with Auto-fix on, checked
//!   in the same write, so a rebind or `/autofix off` during a poll wins.
//! - Logs: a failed job's log is read only when it is about to wake the
//!   session, its download is bounded to its tail, and credentials are
//!   redacted before it reaches the prompt.
//!
//! Review comments come from the REST API, which has no resolved state on
//! GitHub; the resolved review threads are read over GraphQL and their
//! comments are left out, so an answered comment never wakes the session.
//! Dedup on comment id keeps each remaining comment to one wake-up
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
/// Pages read per list endpoint per poll.
const MAX_PAGES: u32 = 10;

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

/// Where a failed job's log can be read. Logs are fetched only for jobs
/// that are about to wake the session, never for one already acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLog {
    /// A GitHub Actions workflow job id.
    GitHubJob(u64),
    /// A GitLab job id.
    GitLabJob(u64),
}

/// One thing a trigger can be about: a failed job, the conflict, or a
/// comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerItem {
    /// Stable id, unique within its kind (e.g. `check:123`, `note:9`).
    pub id: String,
    /// Prompt text describing it.
    pub text: String,
    /// The job log to append to `text`, when it is a failed job with one.
    pub log: Option<JobLog>,
}

impl TriggerItem {
    fn new(id: String, text: String) -> Self {
        Self {
            id,
            text,
            log: None,
        }
    }
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
/// token's own user. Every list endpoint is read page by page, up to
/// [`MAX_PAGES`] pages. When the token's user cannot be determined (for
/// example a GitHub App installation token, which `/user` refuses), review
/// comments are left out rather than the whole poll failing. Job logs are
/// not read here; see [`fetch_job_log`].
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] for any other failure of a required
/// call.
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

/// The last [`LOG_TAIL_LINES`] lines of a failed job's log, with
/// credentials redacted; `None` when the log cannot be read or is empty.
pub async fn fetch_job_log(
    source: &RequestSource,
    remote: &ProjectRemote,
    log: &JobLog,
) -> Option<String> {
    let fetched = match (source, log) {
        (RequestSource::GitHub(client), JobLog::GitHubJob(id)) => client
            .get_bytes_tail(
                &format!("/repos/{}/actions/jobs/{id}/logs", remote.path),
                LOG_LIMIT_BYTES,
            )
            .await
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        (RequestSource::GitLab(client), JobLog::GitLabJob(id)) => {
            client
                .get_text_tail(
                    &format!(
                        "/projects/{}/jobs/{id}/trace",
                        ragent_tools_vcs::percent::encode_project_path(&remote.path)
                    ),
                    LOG_LIMIT_BYTES,
                )
                .await
        }
        _ => return None,
    };
    match fetched {
        Ok(text) => {
            let tail = log_tail(&text);
            // CI logs are where secrets get echoed (`set -x`, a failed
            // `curl -H "Authorization: ..."`); this text goes to the model
            // and the session store.
            (!tail.trim().is_empty()).then(|| ragent_types::sanitize::redact_secrets(&tail))
        }
        Err(e) => {
            tracing::debug!(error = %e, ?log, "autofix: job log unavailable");
            None
        }
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

/// Every item of a paginated GitHub list, up to [`MAX_PAGES`] pages.
/// `path` already has a query string; `key` names the array inside an
/// object response (`check_runs`, `statuses`), `None` for a bare array.
async fn github_list(
    client: &ragent_tools_vcs::github::client::GitHubClient,
    path: &str,
    key: Option<&str>,
) -> Result<Vec<Value>, FetchError> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let body = client
            .get(&format!("{path}&per_page={PER_PAGE}&page={page}"))
            .await
            .map_err(|e| api_error(&e))?;
        let items = match key {
            Some(key) => body[key].as_array().cloned().unwrap_or_default(),
            None => body.as_array().cloned().unwrap_or_default(),
        };
        let last = items.len() < PER_PAGE;
        all.extend(items);
        if last {
            break;
        }
    }
    Ok(all)
}

/// The GraphQL node id of a REST review comment.
fn github_comment_node_id(comment: &Value) -> String {
    comment["node_id"].as_str().unwrap_or_default().to_string()
}

/// The node ids of the review comments that sit in a **resolved** review
/// thread, read over GraphQL because the REST comment list carries no
/// resolved state.
///
/// A thread query that fails (an enterprise host without GraphQL, a rate
/// limit) yields an empty set with a `debug` record rather than an error, so
/// a poll degrades to the previous behaviour (every comment counts, deduped by
/// id) instead of losing its CI and conflict triggers too.
async fn resolved_github_threads(
    client: &ragent_tools_vcs::github::client::GitHubClient,
    repo: &str,
    number: u64,
) -> Result<std::collections::HashSet<String>, FetchError> {
    let (owner, name) = repo.split_once('/').unwrap_or((repo, ""));
    let path = format!("/graphql?owner={owner}&name={name}&number={number}");
    let body = match client.get(&path).await {
        Ok(body) => body,
        Err(e) => {
            // A rate limit is a poll-wide condition: surface it so the
            // heartbeat records it.
            if let FetchError::RateLimited(text) = api_error(&e) {
                return Err(FetchError::RateLimited(text));
            }
            tracing::debug!(
                error = %e,
                "autofix: GitHub review-thread state unavailable; resolved threads are not detected"
            );
            return Ok(std::collections::HashSet::new());
        }
    };
    let mut resolved = std::collections::HashSet::new();
    for thread in body["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
    {
        if thread["isResolved"] != Value::Bool(true) {
            continue;
        }
        for comment in thread["comments"]["nodes"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            if let Some(id) = comment["id"].as_str() {
                resolved.insert(id.to_string());
            }
        }
    }
    Ok(resolved)
}

/// Every item of a paginated GitLab list, following `x-next-page`, up to
/// [`MAX_PAGES`] pages. `path` already has a query string.
async fn gitlab_list(
    client: &ragent_tools_vcs::gitlab::GitLabClient,
    path: &str,
) -> Result<Vec<Value>, FetchError> {
    let mut all = Vec::new();
    let mut page = 1;
    for _ in 0..MAX_PAGES {
        let (body, next) = client
            .get_paged(&format!("{path}&per_page={PER_PAGE}&page={page}"))
            .await
            .map_err(|e| api_error(&e))?;
        all.extend(body.as_array().cloned().unwrap_or_default());
        match next {
            Some(next) => page = next,
            None => break,
        }
    }
    Ok(all)
}

/// The token user's name from a `/user` response, or `None` when it cannot
/// be read. A rate limit is still an error.
fn identity(result: anyhow::Result<Value>, field: &str) -> Result<Option<String>, FetchError> {
    match result {
        Ok(user) => Ok(user[field].as_str().map(str::to_string)),
        Err(e) => match api_error(&e) {
            FetchError::RateLimited(text) => Err(FetchError::RateLimited(text)),
            FetchError::Other(text) => {
                tracing::debug!(error = %text, "autofix: token user unknown; reviews skipped");
                Ok(None)
            }
        },
    }
}

/// The workflow job id of a GitHub Actions check run. Its `details_url`
/// ends in `/actions/runs/{run}/job/{job}`; the check run id is a separate
/// identifier.
fn actions_job_id(run: &Value) -> Option<u64> {
    if run["app"]["slug"] != "github-actions" {
        return None;
    }
    let url = run["details_url"].as_str()?;
    let (_, rest) = url.rsplit_once("/job/")?;
    rest.split(['/', '?', '#']).next()?.parse().ok()
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
    let runs = github_list(
        client,
        &format!("/repos/{repo}/commits/{head_sha}/check-runs?filter=latest"),
        Some("check_runs"),
    )
    .await?;
    for run in &runs {
        let conclusion = run["conclusion"].as_str().unwrap_or_default();
        if run["status"] != "completed" || !matches!(conclusion, "failure" | "timed_out") {
            continue;
        }
        let Some(id) = run["id"].as_u64() else {
            continue;
        };
        let name = run["name"].as_str().unwrap_or("check");
        failed.push(TriggerItem {
            id: format!("check:{id}"),
            text: format!("- job `{name}`"),
            log: actions_job_id(run).map(JobLog::GitHubJob),
        });
    }
    let statuses = github_list(
        client,
        &format!("/repos/{repo}/commits/{head_sha}/statuses?"),
        None,
    )
    .await?;
    // The list is newest first and keeps every state change; only the
    // latest state of each context counts.
    let mut seen = std::collections::HashSet::new();
    for entry in &statuses {
        let context = entry["context"].as_str().unwrap_or("status");
        if !seen.insert(context.to_string()) {
            continue;
        }
        let (Some(id), Some("failure" | "error")) = (entry["id"].as_u64(), entry["state"].as_str())
        else {
            continue;
        };
        let detail = entry["description"].as_str().unwrap_or_default();
        failed.push(TriggerItem::new(
            format!("status:{id}"),
            format!("- status `{context}`: {detail}"),
        ));
    }
    push_kind(&mut items, TriggerKind::Ci, failed);

    if pull["mergeable"] == Value::Bool(false) || pull["mergeable_state"] == "dirty" {
        push_kind(
            &mut items,
            TriggerKind::Conflict,
            vec![TriggerItem::new(
                "conflict".to_string(),
                format!(
                    "- `{}` conflicts with `{}`",
                    info.source_branch,
                    pull["base"]["ref"].as_str().unwrap_or("the target branch")
                ),
            )],
        );
    }

    // Review comments by anyone but the token's own user.
    let Some(me) = identity(client.current_user().await, "login")? else {
        return Ok(snapshot(info, head_sha, items));
    };
    let mut comments = Vec::new();
    let inline = github_list(
        client,
        &format!("/repos/{repo}/pulls/{number}/comments?sort=created"),
        None,
    )
    .await?;
    // The REST comment list carries no resolved state; the GraphQL review
    // threads do. A comment inside a resolved thread is answered and must
    // not wake the session, so read what has been resolved first.
    let resolved = resolved_github_threads(client, repo, number).await?;
    for c in &inline {
        if resolved.contains(&github_comment_node_id(c)) {
            continue;
        }
        let author = c["user"]["login"].as_str().unwrap_or_default();
        let Some(id) = c["id"].as_u64().filter(|_| author != me) else {
            continue;
        };
        let location = c["path"].as_str().map(|path| match c["line"].as_u64() {
            Some(line) => format!("{path}:{line}"),
            None => path.to_string(),
        });
        comments.push(TriggerItem::new(
            format!("comment:{id}"),
            comment_text(author, location, c["body"].as_str().unwrap_or_default()),
        ));
    }
    let reviews = github_list(
        client,
        &format!("/repos/{repo}/pulls/{number}/reviews?"),
        None,
    )
    .await?;
    for r in &reviews {
        let author = r["user"]["login"].as_str().unwrap_or_default();
        let body = r["body"].as_str().unwrap_or_default().trim();
        let Some(id) = r["id"]
            .as_u64()
            .filter(|_| author != me && !body.is_empty())
        else {
            continue;
        };
        comments.push(TriggerItem::new(
            format!("review:{id}"),
            comment_text(author, None, body),
        ));
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
            let jobs = gitlab_list(
                client,
                &format!("/projects/{project}/pipelines/{pipeline_id}/jobs?scope[]=failed"),
            )
            .await?;
            for job in &jobs {
                let Some(id) = job["id"].as_u64() else {
                    continue;
                };
                if job["allow_failure"] == Value::Bool(true) {
                    continue;
                }
                let name = job["name"].as_str().unwrap_or("job");
                failed.push(TriggerItem {
                    id: format!("job:{id}"),
                    text: format!("- job `{name}`"),
                    log: Some(JobLog::GitLabJob(id)),
                });
            }
        }
    }
    push_kind(&mut items, TriggerKind::Ci, failed);

    if mr["detailed_merge_status"] == "conflict" || mr["has_conflicts"] == Value::Bool(true) {
        push_kind(
            &mut items,
            TriggerKind::Conflict,
            vec![TriggerItem::new(
                "conflict".to_string(),
                format!(
                    "- `{}` conflicts with `{}`",
                    info.source_branch,
                    mr["target_branch"].as_str().unwrap_or("the target branch")
                ),
            )],
        );
    }

    // Unresolved, resolvable notes by anyone but the token's own user.
    let Some(me) = identity(client.get("/user").await, "username")? else {
        return Ok(snapshot(info, head_sha, items));
    };
    let discussions = gitlab_list(
        client,
        &format!("/projects/{project}/merge_requests/{number}/discussions?"),
    )
    .await?;
    let mut comments = Vec::new();
    for discussion in &discussions {
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
            comments.push(TriggerItem::new(
                format!("note:{id}"),
                comment_text(author, location, note["body"].as_str().unwrap_or_default()),
            ));
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

/// Settings key of the watcher heartbeat ([`WatcherHeartbeat`]).
pub const HEARTBEAT_KEY: &str = "autofix.watcher";
/// Most poll problems kept in the heartbeat.
const MAX_PROBLEMS: usize = 10;

/// What the running watcher last did, stored as JSON under
/// [`HEARTBEAT_KEY`] after every poll so any process sharing the database
/// (the `autofix_status` tool, the server) can tell whether a watcher is
/// running and how its last poll went.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WatcherHeartbeat {
    /// Process id of the ragent running the watcher.
    pub pid: u32,
    /// RFC 3339 time the watcher started.
    pub started_at: String,
    /// RFC 3339 time of the last finished poll.
    pub last_tick_at: String,
    /// Seconds between polls.
    pub poll_secs: u64,
    /// Wake-ups per kind per commit before pausing.
    pub max_attempts: u32,
    /// Bindings polled in the last tick.
    pub watched: usize,
    /// Wake-ups published in the last tick.
    pub triggered: usize,
    /// Why a binding could not be polled in the last tick (no credentials,
    /// an API error), at most [`MAX_PROBLEMS`].
    pub problems: Vec<String>,
}

impl WatcherHeartbeat {
    /// Reads the stored heartbeat; `None` when no watcher ever ran against
    /// this database or the value cannot be read.
    #[must_use]
    pub fn load(storage: &Storage) -> Option<Self> {
        let json = match storage.get_setting(HEARTBEAT_KEY) {
            Ok(json) => json?,
            Err(e) => {
                tracing::debug!(error = %e, "autofix: heartbeat unreadable");
                return None;
            }
        };
        match serde_json::from_str(&json) {
            Ok(beat) => Some(beat),
            Err(e) => {
                tracing::debug!(error = %e, "autofix: heartbeat malformed");
                None
            }
        }
    }

    /// Whether the watcher looks alive at `now`: its last poll finished
    /// within two intervals plus a minute (a poll can take a while).
    #[must_use]
    pub fn is_live(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        let Ok(last) = chrono::DateTime::parse_from_rfc3339(&self.last_tick_at) else {
            return false;
        };
        let grace = i64::try_from(self.poll_secs.saturating_mul(2) + 60).unwrap_or(i64::MAX);
        now.signed_duration_since(last.with_timezone(&chrono::Utc))
            .num_seconds()
            <= grace
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
    started_at: String,
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
            started_at: chrono::Utc::now().to_rfc3339(),
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
        let mut problems = Vec::new();
        for binding in &bindings {
            for event in self.check(binding, &mut problems).await {
                self.event_bus.publish(event.clone());
                published.push(event);
            }
        }
        problems.truncate(MAX_PROBLEMS);
        self.beat(bindings.len(), published.len(), problems);
        published
    }

    /// Stores the heartbeat for this tick.
    fn beat(&self, watched: usize, triggered: usize, problems: Vec<String>) {
        let beat = WatcherHeartbeat {
            pid: std::process::id(),
            started_at: self.started_at.clone(),
            last_tick_at: chrono::Utc::now().to_rfc3339(),
            poll_secs: self.poll.as_secs(),
            max_attempts: self.max_attempts,
            watched,
            triggered,
            problems,
        };
        let stored = serde_json::to_string(&beat)
            .map_err(anyhow::Error::from)
            .and_then(|json| self.storage.set_setting(HEARTBEAT_KEY, &json));
        if let Err(e) = stored {
            tracing::warn!(error = %e, "autofix: could not store heartbeat");
        }
    }

    /// The events one binding's poll produces; records what it acts on.
    async fn check(&self, binding: &SessionRequest, problems: &mut Vec<String>) -> Vec<Event> {
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
        let noun = kind.noun();
        let request = format!("{} {}{}", noun.short, noun.sigil, binding.number);
        let Some(source) = (self.sources)(&remote) else {
            tracing::debug!(session_id = %binding.session_id, "autofix: no credentials");
            problems.push(format!(
                "{request} ({}): no {} credentials for {}",
                binding.repo, kind, binding.host
            ));
            return Vec::new();
        };
        let snap = match fetch_snapshot(&source, &remote, binding.number).await {
            Ok(snap) => snap,
            Err(e) => {
                tracing::warn!(error = %e, session_id = %binding.session_id, "autofix: poll failed");
                let text = ragent_types::sanitize::redact_secrets(&e.to_string());
                let short: String = text.chars().take(200).collect();
                problems.push(format!("{request} ({}): {short}", binding.repo));
                return Vec::new();
            }
        };
        let session_id = binding.session_id.as_str();
        match self.storage.record_autofix_poll(
            binding,
            snap.state.as_str(),
            &snap.title,
            &snap.head_sha,
        ) {
            Ok(true) => {}
            Ok(false) => {
                // Rebound or turned off while this poll ran.
                tracing::debug!(session_id, "autofix: binding changed; poll dropped");
                return Vec::new();
            }
            Err(e) => {
                tracing::warn!(error = %e, "autofix: could not record poll");
                return Vec::new();
            }
        }
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
            let woken = self
                .wake_for(binding, &source, &remote, &request, &snap, *trigger, items)
                .await;
            match woken {
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
    #[allow(clippy::too_many_arguments)] // reason: one call site, all borrowed context
    async fn wake_for(
        &self,
        binding: &SessionRequest,
        source: &RequestSource,
        remote: &ProjectRemote,
        request: &str,
        snap: &Snapshot,
        kind: TriggerKind,
        items: &[TriggerItem],
    ) -> anyhow::Result<Option<Event>> {
        let session_id = binding.session_id.as_str();
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
        let mut details = Vec::new();
        for item in items.iter().filter(|i| fresh.contains(&i.id)) {
            let log = match &item.log {
                Some(log) => fetch_job_log(source, remote, log).await,
                None => None,
            };
            details.push(match log {
                Some(log) => format!("{}, log tail:\n```\n{log}\n```", item.text),
                None => item.text.clone(),
            });
        }
        if self.storage.record_autofix_wakeup(
            binding,
            &snap.head_sha,
            kind.as_str(),
            &fresh,
            &summary,
        )? == 0
        {
            // Another process recorded these first, or the binding changed
            // or Auto-fix was turned off meanwhile.
            return Ok(None);
        }
        Ok(Some(Event::AutofixTrigger {
            session_id: session_id.to_string(),
            kind: kind.as_str().to_string(),
            summary,
            prompt: prompt(request, snap, kind, &details),
        }))
    }
}

/// The user-role message that wakes the session.
fn prompt(request: &str, snap: &Snapshot, kind: TriggerKind, details: &[String]) -> String {
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
