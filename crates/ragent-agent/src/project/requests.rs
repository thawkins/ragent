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
/// this gets counts for its newest `MAX_PAGES * PER_PAGE` requests.
const MAX_PAGES: u32 = 10;

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
/// branch of the same name is a different branch.
///
/// # Errors
///
/// Returns [`FetchError::RateLimited`] when the API's rate limit is
/// exhausted, and [`FetchError::Other`] for any other failure.
pub async fn fetch_request_counts(
    source: &RequestSource,
    remote: &ProjectRemote,
) -> std::result::Result<HashMap<String, i64>, FetchError> {
    let mut counts: HashMap<String, i64> = HashMap::new();
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
        }
    }
    Ok(counts)
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
    schedule: Mutex<HashMap<String, Schedule>>,
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
            schedule: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `project_id` may be refreshed now.
    #[must_use]
    pub fn is_due(&self, project_id: &str) -> bool {
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
    /// never sent to a host the user did not configure.
    #[must_use]
    pub fn source_for(&self, remote: &ProjectRemote) -> Option<RequestSource> {
        match remote.kind {
            RemoteKind::GitHub => ragent_tools_vcs::github::auth::load_token()
                .map(|token| RequestSource::GitHub(GitHubClient::with_token(token))),
            RemoteKind::GitLab => {
                let client = GitLabClient::new(self.storage.as_ref()).ok()?;
                let instance = client.instance_url().to_ascii_lowercase();
                let instance_host = instance
                    .split_once("://")
                    .map_or(instance.as_str(), |(_, rest)| rest)
                    .trim_end_matches('/');
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
            Ok(counts) => {
                self.reschedule(&project.id, false);
                match self.store(&project.id, &branches, &counts) {
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

    fn store(
        &self,
        project_id: &str,
        branches: &[String],
        counts: &HashMap<String, i64>,
    ) -> Result<usize> {
        let mut changed = 0;
        for branch in branches {
            let count = counts.get(branch).copied().unwrap_or(0);
            changed += self
                .storage
                .set_branch_pr_count(project_id, branch, Some(count))?;
        }
        Ok(changed)
    }
}
