//! Pull/merge request counts per session branch (spec `projects`, T-015).
//!
//! The sessions sidebar shows, beside a session's branch, how many pull
//! requests (GitHub) or merge requests (GitLab) have that branch as their
//! source. The count is fetched with one paginated list call per project,
//! grouped by source branch in memory, never one call per session.
//!
//! [`PullRequestCounts`] owns the schedule: a project is refreshed at most
//! once per interval, the interval doubles (up to one hour) after a rate
//! limit, and any failure leaves the stored counts as they were. The last
//! known count is kept in `sessions.pr_count` so it is available at startup
//! before the first fetch completes.
//!
//! Depends on the GitHub and GitLab API clients in `ragent-tools-vcs`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::Result;
use ragent_storage::Storage;
use ragent_tools_vcs::github::client::GitHubClient;
use ragent_tools_vcs::gitlab::GitLabClient;
use serde_json::Value;

use super::{Project, ProjectRemote, RemoteKind};
use crate::event::{Event, EventBus};

/// Interval between refreshes of one project.
pub const BASE_INTERVAL: Duration = Duration::from_mins(5);
/// Longest interval a rate-limited project backs off to.
pub const MAX_INTERVAL: Duration = Duration::from_hours(1);
/// Shortest gap between two refreshes of one project that an activity hint
/// ([`PullRequestCounts::nudge`]) can bring about.
pub const NUDGE_GAP: Duration = Duration::from_mins(1);
/// Requests per page; the maximum both APIs allow.
const PER_PAGE: usize = 100;
/// Pages read per project per refresh. A repository with more requests than
/// `MAX_PAGES * PER_PAGE` yields an incomplete result (see
/// [`RequestCountsPage::complete`]).
const MAX_PAGES: u32 = 10;

/// Request counts by source branch, and whether every request was read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestCountsPage {
    /// Requests seen per source branch.
    pub counts: HashMap<String, i64>,
    /// `false` when the repository has more requests than one refresh
    /// reads. The counts are then lower bounds, and a branch that is absent
    /// may still have requests.
    pub complete: bool,
}

/// An API client able to list a project's pull/merge requests.
pub enum RequestSource {
    /// The GitHub REST API.
    GitHub(GitHubClient),
    /// The GitLab REST API.
    GitLab(GitLabClient),
}

impl std::fmt::Debug for RequestSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The clients hold a token; never print them.
        f.write_str(match self {
            Self::GitHub(_) => "RequestSource::GitHub",
            Self::GitLab(_) => "RequestSource::GitLab",
        })
    }
}

/// Why request counts could not be fetched.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The API refused the call because its rate limit is exhausted.
    #[error("rate limited: {0}")]
    RateLimited(String),
    /// Any other failure (network, authentication, unexpected response).
    #[error("{0}")]
    Other(String),
}

impl FetchError {
    fn from_api(error: &anyhow::Error) -> Self {
        let text = format!("{error:#}");
        let lower = text.to_ascii_lowercase();
        if lower.contains("rate limit") || lower.contains(" 429") {
            Self::RateLimited(text)
        } else {
            Self::Other(text)
        }
    }
}

/// Counts the pull/merge requests of the repository at `remote`, by source
/// branch. Requests in every state (open, merged, closed) are counted;
/// requests whose source branch lives in a fork are not, because a fork's
/// branch of the same name is a different branch. At most
/// `MAX_PAGES * PER_PAGE` requests are read; the result says whether that
/// was all of them.
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] for any other failure.
pub async fn fetch_request_counts(
    source: &RequestSource,
    remote: &ProjectRemote,
) -> std::result::Result<RequestCountsPage, FetchError> {
    let mut counts: HashMap<String, i64> = HashMap::new();
    // Set when the last page was reached; stays false if the page limit
    // stopped the read first.
    let mut complete = false;
    match source {
        RequestSource::GitHub(client) => {
            for page in 1..=MAX_PAGES {
                let path = format!(
                    "/repos/{}/pulls?state=all&per_page={PER_PAGE}&page={page}",
                    remote.path
                );
                let body = client
                    .get(&path)
                    .await
                    .map_err(|e| FetchError::from_api(&e))?;
                let items = body.as_array().map_or(&[][..], Vec::as_slice);
                for item in items {
                    let same_repo = item["head"]["repo"]["full_name"]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&remote.path));
                    if let (true, Some(branch)) = (same_repo, item["head"]["ref"].as_str()) {
                        *counts.entry(branch.to_string()).or_default() += 1;
                    }
                }
                if items.len() < PER_PAGE {
                    complete = true;
                    break;
                }
            }
        }
        RequestSource::GitLab(client) => {
            let project = ragent_tools_vcs::percent::encode_project_path(&remote.path);
            let mut page = Some(1u32);
            let mut read = 0u32;
            while let Some(current) = page.filter(|_| read < MAX_PAGES) {
                let path = format!(
                    "/projects/{project}/merge_requests?state=all&per_page={PER_PAGE}&page={current}"
                );
                let (body, next) = client
                    .get_paged(&path)
                    .await
                    .map_err(|e| FetchError::from_api(&e))?;
                for item in body.as_array().map_or(&[][..], Vec::as_slice) {
                    let same_project = match (
                        item.get("source_project_id").filter(|v| !v.is_null()),
                        item.get("target_project_id").filter(|v| !v.is_null()),
                    ) {
                        (Some(source), Some(target)) => source == target,
                        _ => true,
                    };
                    if let (true, Some(branch)) = (
                        same_project,
                        item.get("source_branch").and_then(Value::as_str),
                    ) {
                        *counts.entry(branch.to_string()).or_default() += 1;
                    }
                }
                read += 1;
                page = next;
            }
            complete = page.is_none();
        }
    }
    Ok(RequestCountsPage { counts, complete })
}

/// The state of one pull/merge request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    /// Open (GitHub `open`; GitLab `opened` or `locked`).
    Open,
    /// Merged.
    Merged,
    /// Closed without merging.
    Closed,
}

impl RequestState {
    /// The stored spelling: `open`, `merged` or `closed`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

/// One pull/merge request, as far as binding a session to it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInfo {
    /// PR number (GitHub) or MR iid (GitLab).
    pub number: u64,
    /// Title.
    pub title: String,
    /// Browser URL.
    pub web_url: String,
    /// Source branch.
    pub source_branch: String,
    /// Current state.
    pub state: RequestState,
}

impl RequestInfo {
    /// Reads a GitHub pull request object.
    pub(crate) fn from_github(item: &Value) -> Option<Self> {
        let state = if item["merged_at"].is_string() || item["merged"] == Value::Bool(true) {
            RequestState::Merged
        } else if item["state"] == "open" {
            RequestState::Open
        } else {
            RequestState::Closed
        };
        Some(Self {
            number: item["number"].as_u64()?,
            title: item["title"].as_str().unwrap_or_default().to_string(),
            web_url: item["html_url"].as_str().unwrap_or_default().to_string(),
            source_branch: item["head"]["ref"].as_str().unwrap_or_default().to_string(),
            state,
        })
    }

    /// Reads a GitLab merge request object.
    pub(crate) fn from_gitlab(item: &Value) -> Option<Self> {
        let state = match item["state"].as_str() {
            Some("merged") => RequestState::Merged,
            Some("closed") => RequestState::Closed,
            _ => RequestState::Open,
        };
        Some(Self {
            number: item["iid"].as_u64()?,
            title: item["title"].as_str().unwrap_or_default().to_string(),
            web_url: item["web_url"].as_str().unwrap_or_default().to_string(),
            source_branch: item["source_branch"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            state,
        })
    }
}

/// The open pull/merge requests of the repository at `remote` whose source
/// is `branch` in that same repository (a fork's branch of the same name is
/// a different branch and is left out).
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] for any other failure.
pub async fn open_requests_for_branch(
    source: &RequestSource,
    remote: &ProjectRemote,
    branch: &str,
) -> std::result::Result<Vec<RequestInfo>, FetchError> {
    use ragent_tools_vcs::percent::{encode_component, encode_project_path};
    let found = match source {
        RequestSource::GitHub(client) => {
            let owner = remote.path.split('/').next().unwrap_or_default();
            let head = encode_component(&format!("{owner}:{branch}"));
            let path = format!(
                "/repos/{}/pulls?state=open&head={head}&per_page={PER_PAGE}",
                remote.path
            );
            let body = client
                .get(&path)
                .await
                .map_err(|e| FetchError::from_api(&e))?;
            body.as_array()
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter(|item| {
                    item["head"]["repo"]["full_name"]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&remote.path))
                })
                .filter_map(RequestInfo::from_github)
                .collect::<Vec<_>>()
        }
        RequestSource::GitLab(client) => {
            let path = format!(
                "/projects/{}/merge_requests?state=opened&source_branch={}&per_page={PER_PAGE}",
                encode_project_path(&remote.path),
                encode_component(branch)
            );
            let body = client
                .get(&path)
                .await
                .map_err(|e| FetchError::from_api(&e))?;
            body.as_array()
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter(|item| {
                    match (
                        item.get("source_project_id").filter(|v| !v.is_null()),
                        item.get("target_project_id").filter(|v| !v.is_null()),
                    ) {
                        (Some(source), Some(target)) => source == target,
                        _ => true,
                    }
                })
                .filter_map(RequestInfo::from_gitlab)
                .collect::<Vec<_>>()
        }
    };
    Ok(found
        .into_iter()
        .filter(|r| r.source_branch == branch)
        .collect())
}

/// Reads pull request `number` (GitHub) or merge request iid `number`
/// (GitLab) of the repository at `remote`.
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] when the request does not exist or
/// the call fails.
pub async fn get_request(
    source: &RequestSource,
    remote: &ProjectRemote,
    number: u64,
) -> std::result::Result<RequestInfo, FetchError> {
    let (body, parsed) = match source {
        RequestSource::GitHub(client) => {
            let path = format!("/repos/{}/pulls/{number}", remote.path);
            let body = client
                .get(&path)
                .await
                .map_err(|e| FetchError::from_api(&e))?;
            let parsed = RequestInfo::from_github(&body);
            (body, parsed)
        }
        RequestSource::GitLab(client) => {
            let path = format!(
                "/projects/{}/merge_requests/{number}",
                ragent_tools_vcs::percent::encode_project_path(&remote.path)
            );
            let body = client
                .get(&path)
                .await
                .map_err(|e| FetchError::from_api(&e))?;
            let parsed = RequestInfo::from_gitlab(&body);
            (body, parsed)
        }
    };
    parsed.ok_or_else(|| FetchError::Other(format!("unexpected response: {body}")))
}

impl RequestInfo {
    /// The binding row for `session_id` to this request on `remote`.
    #[must_use]
    pub fn to_session_request(
        &self,
        session_id: &str,
        remote: &ProjectRemote,
    ) -> ragent_storage::SessionRequest {
        ragent_storage::SessionRequest {
            session_id: session_id.to_string(),
            forge: remote.kind.as_str().to_string(),
            host: remote.host.clone(),
            repo: remote.path.clone(),
            number: self.number,
            title: self.title.clone(),
            web_url: self.web_url.clone(),
            head_branch: self.source_branch.clone(),
            state: self.state.as_str().to_string(),
        }
    }
}

/// Which request [`bind_session`] binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindTarget {
    /// The request with this number (any state).
    Number(u64),
    /// The single open request whose source is this branch.
    Branch(String),
}

/// What [`bind_session`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindOutcome {
    /// The session is now bound to this request.
    Bound(RequestInfo),
    /// The branch has no open request; nothing was bound.
    NoneOpen,
    /// The branch has several open requests; nothing was bound.
    Several(Vec<RequestInfo>),
}

/// Looks up the request `target` names on `remote` and binds it to the
/// top-level session of `session_id` (spec SESSIONMENU FR-14).
/// `autofix_default` turns Auto-fix on when the session had no binding.
///
/// # Errors
///
/// Returns a [`FetchError`] when the lookup fails or the request does not
/// exist, and [`FetchError::Other`] when the binding cannot be stored.
pub async fn bind_session(
    storage: &Storage,
    session_id: &str,
    source: &RequestSource,
    remote: &ProjectRemote,
    target: BindTarget,
    autofix_default: bool,
) -> std::result::Result<BindOutcome, FetchError> {
    let info = match target {
        BindTarget::Number(number) => get_request(source, remote, number).await?,
        BindTarget::Branch(branch) => {
            let mut open = open_requests_for_branch(source, remote, &branch).await?;
            match open.len() {
                0 => return Ok(BindOutcome::NoneOpen),
                1 => open.remove(0),
                _ => return Ok(BindOutcome::Several(open)),
            }
        }
    };
    storage
        .bind_session_request(
            &info.to_session_request(session_id, remote),
            autofix_default,
        )
        .map_err(|e| FetchError::Other(format!("could not store the binding: {e}")))?;
    Ok(BindOutcome::Bound(info))
}

/// What one call to [`PullRequestCounts::refresh`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// Counts were fetched; this many sessions' stored counts changed.
    Updated(usize),
    /// The project was refreshed too recently.
    NotDue,
    /// The project has no GitHub or GitLab remote.
    NoRemote,
    /// No live session of the project has a branch, so nothing would be shown.
    NoBranches,
    /// No stored credentials match the project's remote host.
    NoCredentials,
    /// The fetch failed; stored counts are unchanged.
    Failed {
        /// Whether the failure was a rate limit (the interval was doubled).
        rate_limited: bool,
    },
}

/// When a project may next be refreshed.
#[derive(Debug, Clone, Copy)]
struct Schedule {
    next_allowed: Instant,
    interval: Duration,
    last_attempt: Instant,
}

/// Fetches and stores request counts, at most once per interval per
/// project.
pub struct PullRequestCounts {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
    github_hosts: Vec<ragent_config::GitHubHost>,
    schedule: Mutex<HashMap<String, Schedule>>,
    /// Projects with a refresh running; guarded separately from the schedule
    /// only because the two are never needed together.
    in_flight: Mutex<std::collections::HashSet<String>>,
}

/// Marks a project's refresh as running until dropped, so a refresh that is
/// cancelled or panics still releases the project.
struct InFlight<'a> {
    service: &'a PullRequestCounts,
    project_id: String,
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.service
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.project_id);
    }
}

impl std::fmt::Debug for PullRequestCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PullRequestCounts").finish_non_exhaustive()
    }
}

impl PullRequestCounts {
    /// Creates the service over the given storage and event bus.
    #[must_use]
    pub fn new(storage: Arc<Storage>, event_bus: Arc<EventBus>) -> Self {
        Self {
            storage,
            event_bus,
            github_hosts: Vec::new(),
            schedule: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Serves remotes on these GitHub Enterprise Server hosts
    /// (`vcs.github.hosts`) through their configured `api_url`.
    #[must_use]
    pub fn with_github_hosts(mut self, hosts: Vec<ragent_config::GitHubHost>) -> Self {
        self.github_hosts = hosts;
        self
    }

    /// Reserves `project_id` for one refresh. Returns `None` when a refresh
    /// of that project is already running.
    fn begin(&self, project_id: &str) -> Option<InFlight<'_>> {
        self.in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(project_id.to_string())
            .then(|| InFlight {
                service: self,
                project_id: project_id.to_string(),
            })
    }

    /// Whether `project_id` may be refreshed now: its interval has passed
    /// and no refresh of it is running.
    #[must_use]
    pub fn is_due(&self, project_id: &str) -> bool {
        if self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(project_id)
        {
            return false;
        }
        self.schedule
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(project_id)
            .is_none_or(|s| Instant::now() >= s.next_allowed)
    }

    /// The current refresh interval of `project_id`.
    #[must_use]
    pub fn interval(&self, project_id: &str) -> Duration {
        self.schedule
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(project_id)
            .map_or(BASE_INTERVAL, |s| s.interval)
    }

    /// Lets `project_id` be refreshed immediately (an explicit user refresh).
    /// A rate-limit back-off is kept: if the next attempt is refused again
    /// the interval keeps widening from where it was.
    pub fn force_due(&self, project_id: &str) {
        if let Some(entry) = self
            .schedule
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(project_id)
        {
            entry.next_allowed = Instant::now();
        }
    }

    /// Hints that something just happened in `project_id` that may have
    /// changed its requests (a turn ended, the project was focused). The
    /// project becomes due once [`NUDGE_GAP`] has passed since its last
    /// attempt, unless it is backing off after a rate limit.
    pub fn nudge(&self, project_id: &str) {
        if let Some(entry) = self
            .schedule
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(project_id)
        {
            if entry.interval == BASE_INTERVAL {
                entry.next_allowed = entry.next_allowed.min(entry.last_attempt + NUDGE_GAP);
            }
        }
    }

    /// Records the outcome of an attempt and schedules the next one.
    fn reschedule(&self, project_id: &str, rate_limited: bool) {
        let mut schedule = self.schedule.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = schedule
            .get(project_id)
            .map_or(BASE_INTERVAL, |s| s.interval);
        let interval = if rate_limited {
            (previous * 2).min(MAX_INTERVAL)
        } else {
            BASE_INTERVAL
        };
        let now = Instant::now();
        schedule.insert(
            project_id.to_string(),
            Schedule {
                next_allowed: now + interval,
                interval,
                last_attempt: now,
            },
        );
    }

    /// Builds an API client for `remote` from the stored credentials.
    ///
    /// A GitLab token is only ever used against the configured GitLab
    /// instance: a remote on any other host gets no client, so the token is
    /// never sent to a host the user did not configure. Likewise the
    /// github.com token is only sent to github.com: a GitHub Enterprise
    /// Server host uses its configured `api_url` and the enterprise token
    /// ([`ragent_config::github::enterprise_env_token`]).
    #[must_use]
    pub fn source_for(&self, remote: &ProjectRemote) -> Option<RequestSource> {
        match remote.kind {
            RemoteKind::GitHub => {
                if let Some(entry) = super::github_host_entry(&remote.host, &self.github_hosts) {
                    let token = ragent_config::github::enterprise_env_token()?;
                    return Some(RequestSource::GitHub(GitHubClient::with_base_url(
                        entry.api_url.clone(),
                        token,
                    )));
                }
                let name = remote.host.split(':').next().unwrap_or(&remote.host);
                (name.strip_prefix("www.").unwrap_or(name) == "github.com")
                    .then(ragent_tools_vcs::github::auth::load_token)
                    .flatten()
                    .map(|token| RequestSource::GitHub(GitHubClient::with_token(token)))
            }
            RemoteKind::GitLab => {
                let client = GitLabClient::new(self.storage.as_ref()).ok()?;
                // Compare hosts only: the instance may be served under a
                // path (`https://host/gitlab`), which the client keeps in its
                // base URL and the remote's host never contains.
                let (instance_host, _prefix) = super::split_instance_url(client.instance_url());
                (instance_host == remote.host).then_some(RequestSource::GitLab(client))
            }
        }
    }

    /// Refreshes the request counts of `project` if it is due, using the
    /// stored credentials.
    pub async fn refresh(&self, project: &Project) -> RefreshOutcome {
        let source = project.remote.as_ref().and_then(|r| self.source_for(r));
        self.refresh_with(project, source.as_ref()).await
    }

    /// Refreshes the request counts of `project` if it is due, with an
    /// explicit API client (`None` when there are no credentials).
    ///
    /// On success every live session branch of the project gets its count
    /// stored (zero when no request has that branch) and
    /// [`Event::ProjectUpdated`] is published if any stored count changed.
    /// On failure nothing stored changes.
    pub async fn refresh_with(
        &self,
        project: &Project,
        source: Option<&RequestSource>,
    ) -> RefreshOutcome {
        let Some(remote) = project.remote.as_ref() else {
            return RefreshOutcome::NoRemote;
        };
        if !self.is_due(&project.id) {
            return RefreshOutcome::NotDue;
        }
        // One refresh per project at a time: a slow fetch must not be
        // joined by the next sweep or by an explicit refresh.
        let Some(_running) = self.begin(&project.id) else {
            return RefreshOutcome::NotDue;
        };
        let branches = match self.storage.project_session_branches(&project.id) {
            Ok(branches) => branches,
            Err(e) => {
                tracing::debug!(error = %e, "could not read session branches");
                return RefreshOutcome::Failed {
                    rate_limited: false,
                };
            }
        };
        if branches.is_empty() {
            return RefreshOutcome::NoBranches;
        }
        let Some(source) = source else {
            self.reschedule(&project.id, false);
            return RefreshOutcome::NoCredentials;
        };
        match fetch_request_counts(source, remote).await {
            Ok(page) => {
                self.reschedule(&project.id, false);
                match self.store(&project.id, &branches, &page) {
                    Ok(changed) => {
                        if changed > 0 {
                            self.event_bus.publish(Event::ProjectUpdated {
                                project_id: project.id.clone(),
                            });
                        }
                        RefreshOutcome::Updated(changed)
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "could not store request counts");
                        RefreshOutcome::Failed {
                            rate_limited: false,
                        }
                    }
                }
            }
            Err(e) => {
                let rate_limited = matches!(e, FetchError::RateLimited(_));
                tracing::debug!(project = %project.name, error = %e, "request counts not refreshed");
                self.reschedule(&project.id, rate_limited);
                RefreshOutcome::Failed { rate_limited }
            }
        }
    }

    /// Stores the fetched counts for the project's session branches.
    ///
    /// A complete result is authoritative: a branch it does not mention has
    /// no requests and is stored as zero. An incomplete result only raises
    /// what is known: a branch it mentions gets that count unless a larger
    /// one is already stored, and a branch it does not mention keeps its
    /// last known count.
    fn store(
        &self,
        project_id: &str,
        branches: &[String],
        page: &RequestCountsPage,
    ) -> Result<usize> {
        let mut changed = 0;
        for branch in branches {
            let fetched = page.counts.get(branch).copied();
            let count = if page.complete {
                fetched.unwrap_or(0)
            } else {
                let Some(fetched) = fetched else {
                    continue;
                };
                let known = self.storage.branch_pr_count(project_id, branch)?;
                fetched.max(known.unwrap_or(0))
            };
            changed += self
                .storage
                .set_branch_pr_count(project_id, branch, Some(count))?;
        }
        Ok(changed)
    }
}
