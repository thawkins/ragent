//! GitHub Actions tools — retrieve recent workflow runs and their logs.
//!
//! The [`GithubGetActionsTool`] lists the last `N` workflow runs for the
//! current repository. For each run it reports the run status (`OK` or
//! `Failed`). For failed runs it downloads the run's log archive (a zip of
//! per-job log files) and extracts log lines that contain `error` or `failed`,
//! showing ±10 lines of context around each match.

use std::io::{Cursor, Read};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::github::GitHubClient;

use super::{Tool, ToolContext, ToolOutput};

/// Number of context lines shown on each side of a matching log line.
const CONTEXT_RADIUS: usize = 10;

/// Keywords (lower-cased) that mark an interesting log line.
const ERROR_KEYWORDS: &[&str] = &["error", "failed"];

/// Returns the authenticated `GitHubClient` or a human-readable error.
fn make_client() -> Result<GitHubClient> {
    GitHubClient::new().context("GitHub not authenticated. Run /github login to authenticate.")
}

/// Resolve owner/repo from the working directory or return an error.
fn detect_repo(ctx: &ToolContext) -> Result<(String, String)> {
    GitHubClient::detect_repo(&ctx.working_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not detect GitHub repository from git remote. \
             Ensure you're in a git repo with a GitHub remote."
        )
    })
}

/// Normalise a GitHub run conclusion into a short status label.
///
/// `OK` covers a successful conclusion; everything else (failure,
/// cancelled, timed_out, action_required, neutral, skipped, stale) is
/// reported as `Failed`. A run still in progress (no conclusion) is
/// reported as `Running`.
fn run_status(conclusion: Option<&str>, status: &str) -> &'static str {
    if status == "in_progress" || status == "queued" {
        return "Running";
    }
    match conclusion {
        Some("success") => "OK",
        Some("neutral") | Some("skipped") => "OK",
        _ => "Failed",
    }
}

/// Extract the ranges of log lines that contain an error keyword plus
/// `CONTEXT_RADIUS` lines of context on each side, merging overlapping or
/// adjacent ranges so the output is contiguous and free of duplicates.
///
/// Matching is case-insensitive. The returned vector is a list of
/// `(start, end)` half-open ranges into `lines`.
pub fn extract_context_ranges(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut hits: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if ERROR_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
            hits.push(i);
        }
    }
    if hits.is_empty() {
        return Vec::new();
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for &i in &hits {
        let start = i.saturating_sub(CONTEXT_RADIUS);
        let end = (i + CONTEXT_RADIUS + 1).min(lines.len());
        if let Some(last) = ranges.last_mut()
            && start <= last.1
        {
            // Merge with the previous range.
            last.1 = last.1.max(end);
        } else {
            ranges.push((start, end));
        }
    }
    ranges
}

/// Build a human-readable snippet from a single log file's contents,
/// showing only the context windows around `error`/`failed` lines.
fn filter_log_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let ranges = extract_context_ranges(&lines);
    if ranges.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (start, end) in ranges {
        out.push_str(&format!("--- lines {start}..{end} ---\n"));
        for line in &lines[start..end] {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Download and extract the per-job log files for a workflow run.
///
/// The GitHub Actions logs endpoint responds with a 302 redirect to a
/// zip archive. `GitHubClient::get_bytes` follows the redirect and
/// returns the raw zip bytes; we then read each entry as UTF-8 text and
/// filter it down to the error context windows.
async fn fetch_run_logs(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    run_id: u64,
) -> Result<String> {
    let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/logs");
    let bytes = client.get_bytes(&path).await?;

    let cursor = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).context("Failed to open Actions logs archive (not a zip)")?;

    let mut output = String::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("Failed to read zip entry {i}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let mut text = String::new();
        if entry.read_to_string(&mut text).is_err() {
            // Fall back to lossy conversion for non-UTF-8 logs.
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .with_context(|| format!("Failed to read log bytes for {name}"))?;
            text = String::from_utf8_lossy(&buf).into_owned();
        }
        let snippet = filter_log_text(&text);
        if !snippet.is_empty() {
            output.push_str(&format!("## {name}\n{snippet}"));
        }
    }

    if output.is_empty() {
        output.push_str("(no log lines matched 'error' or 'failed')\n");
    }
    Ok(output)
}

// ---------------------------------------------------------------------------
// GithubGetActionsTool
// ---------------------------------------------------------------------------

/// Tool that lists recent GitHub Actions runs and extracts error context
/// from the logs of failed runs.
pub struct GithubGetActionsTool;

#[async_trait::async_trait]
impl Tool for GithubGetActionsTool {
    fn name(&self) -> &'static str {
        "github_get_actions"
    }

    fn description(&self) -> &'static str {
        "List the most recent GitHub Actions workflow runs for the repository detected from the working directory, \
         including status summaries and, for failed runs, filtered log excerpts around 'error' or 'failed' lines. \
         No required parameters. 'limit' (integer, default 1, max 30) sets how many recent runs to inspect. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: failed-run log extraction downloads a zip archive, which may be slow for large workflows; \
         logs for still-running jobs may be unavailable."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of recent workflow runs to inspect (default 1, max 30)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let limit = input["limit"].as_u64().unwrap_or(1).clamp(1, 30) as u32;
        let path = format!("/repos/{owner}/{repo}/actions/runs?per_page={limit}");

        let resp = client.get(&path).await?;
        let runs = resp["workflow_runs"].as_array().with_context(|| {
            format!("Unexpected response from GitHub Actions runs endpoint: {resp}")
        })?;

        if runs.is_empty() {
            return Ok(ToolOutput {
                content: format!("No Actions runs found in {owner}/{repo}."),
                metadata: None,
            });
        }

        let mut content = format!("Actions runs for {owner}/{repo} (last {limit}):\n\n");
        let mut failed_count: u32 = 0;

        for run in runs {
            let run_id = run["id"].as_u64().unwrap_or(0);
            let name = run["name"].as_str().unwrap_or("(unnamed)");
            let branch = run["head_branch"].as_str().unwrap_or("?");
            let event = run["event"].as_str().unwrap_or("?");
            let html_url = run["html_url"].as_str().unwrap_or("");
            let conclusion = run["conclusion"].as_str();
            let status = run["status"].as_str().unwrap_or("?");
            let label = run_status(conclusion, status);

            content.push_str(&format!(
                "Run #{run_id} [{label}] {name} — branch {branch}, event {event}"
            ));
            if !html_url.is_empty() {
                content.push_str(&format!("\n  {html_url}"));
            }
            content.push('\n');

            if label == "Failed" {
                failed_count += 1;
                content.push_str("\n  Logs (error/failed context):\n");
                match fetch_run_logs(&client, &owner, &repo, run_id).await {
                    Ok(logs) => {
                        for line in logs.lines() {
                            content.push_str("    ");
                            content.push_str(line);
                            content.push('\n');
                        }
                    }
                    Err(e) => {
                        content.push_str(&format!("    (failed to fetch logs: {e})\n"));
                    }
                }
            }
            content.push('\n');
        }

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "owner": owner,
                "repo": repo,
                "runs_inspected": runs.len(),
                "failed_runs": failed_count,
                "context_radius": CONTEXT_RADIUS,
            })),
        })
    }
}
