//! GitLab pipeline and job tools: list pipelines, get pipeline details,
//! list jobs, get job details, retry/cancel jobs, and download job logs.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::gitlab::client::GitLabClient;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Create an authenticated client and detect the project path.
fn detect(ctx: &ToolContext) -> Result<(GitLabClient, String)> {
    let storage = ctx
        .storage
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Storage not available for GitLab client"))?;
    let client = GitLabClient::new(storage)
        .map_err(|_| anyhow::anyhow!("GitLab not configured. Run /gitlab setup."))?;
    let project = GitLabClient::detect_project(&ctx.working_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not detect GitLab project from git remote."))?;
    Ok((client, project))
}

/// Format a pipeline status with a visual indicator.
fn status_icon(status: &str) -> &str {
    match status {
        "success" | "passed" => "✅",
        "failed" => "❌",
        "running" => "🔄",
        "pending" => "⏳",
        "canceled" | "cancelled" => "🚫",
        "skipped" => "⏭️",
        "manual" => "👆",
        "created" => "🆕",
        "waiting_for_resource" => "⏳",
        _ => "❓",
    }
}

// ── GitlabListPipelinesTool ──────────────────────────────────────────────────

/// Tool that lists pipelines in a GitLab project.
pub struct GitlabListPipelinesTool;

#[async_trait::async_trait]
impl Tool for GitlabListPipelinesTool {
    fn name(&self) -> &'static str {
        "gitlab_list_pipelines"
    }

    fn description(&self) -> &'static str {
        "List GitLab CI/CD pipelines for the current project. \
         Optional: status (running/pending/success/failed/canceled/skipped), \
         ref (branch or tag), limit (default 20)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": [
                        "running", "pending", "success", "failed",
                        "canceled", "skipped", "created",
                        "waiting_for_resource", "manual"
                    ],
                    "description": "Filter by pipeline status"
                },
                "ref": {
                    "type": "string",
                    "description": "Filter by branch or tag name"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max pipelines to return (default 20, max 100)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;

        let limit = input["limit"].as_u64().unwrap_or(20).min(100);
        let mut path = format!(
            "/projects/{project}/pipelines?per_page={limit}&order_by=id&sort=desc"
        );
        if let Some(status) = input["status"].as_str() {
            path.push_str(&format!("&status={status}"));
        }
        if let Some(git_ref) = input["ref"].as_str() {
            path.push_str(&format!("&ref={git_ref}"));
        }

        let pipelines = client.get(&path).await?;
        let arr = pipelines
            .as_array()
            .context("Expected array from GitLab pipelines endpoint")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: "No pipelines found.".to_string(),
                metadata: None,
            });
        }

        let mut lines = vec!["Pipelines:\n".to_string()];
        for p in arr {
            let id = p["id"].as_u64().unwrap_or(0);
            let status = p["status"].as_str().unwrap_or("?");
            let git_ref = p["ref"].as_str().unwrap_or("?");
            let sha = p["sha"].as_str().unwrap_or("?");
            let short_sha = if sha.len() >= 8 { &sha[..8] } else { sha };
            let source = p["source"].as_str().unwrap_or("?");
            let created = p["created_at"].as_str().unwrap_or("?");
            lines.push(format!(
                "  {} #{id} [{status}] ref:{git_ref} ({short_sha}) source:{source} {created}",
                status_icon(status)
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({"count": arr.len(), "project": project})),
        })
    }
}

// ── GitlabGetPipelineTool ────────────────────────────────────────────────────

/// Tool that retrieves details of a specific pipeline.
pub struct GitlabGetPipelineTool;

#[async_trait::async_trait]
impl Tool for GitlabGetPipelineTool {
    fn name(&self) -> &'static str {
        "gitlab_get_pipeline"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitLab pipeline including status, \
         duration, stages, and metadata."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pipeline_id": {
                    "type": "integer",
                    "description": "The pipeline ID"
                }
            },
            "required": ["pipeline_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let pipeline_id = input["pipeline_id"]
            .as_u64()
            .context("pipeline_id is required")?;

        let pipeline = client
            .get(&format!("/projects/{project}/pipelines/{pipeline_id}"))
            .await?;

        let id = pipeline["id"].as_u64().unwrap_or(0);
        let status = pipeline["status"].as_str().unwrap_or("?");
        let git_ref = pipeline["ref"].as_str().unwrap_or("?");
        let sha = pipeline["sha"].as_str().unwrap_or("?");
        let source = pipeline["source"].as_str().unwrap_or("?");
        let created = pipeline["created_at"].as_str().unwrap_or("?");
        let started = pipeline["started_at"].as_str().unwrap_or("not started");
        let finished = pipeline["finished_at"].as_str().unwrap_or("not finished");
        let duration = pipeline["duration"]
            .as_f64()
            .map(|d| format!("{d:.1}s"))
            .unwrap_or_else(|| "N/A".to_string());
        let web_url = pipeline["web_url"].as_str().unwrap_or("?");
        let user = pipeline["user"]["username"].as_str().unwrap_or("?");

        let content = format!(
            "{icon} Pipeline #{id}\n\
             Status:   {status}\n\
             Ref:      {git_ref} ({sha})\n\
             Source:   {source}\n\
             User:     {user}\n\
             Created:  {created}\n\
             Started:  {started}\n\
             Finished: {finished}\n\
             Duration: {duration}\n\
             URL:      {web_url}",
            icon = status_icon(status),
        );

        Ok(ToolOutput {
            content,
            metadata: Some(json!({"pipeline_id": id, "status": status})),
        })
    }
}

// ── GitlabListJobsTool ───────────────────────────────────────────────────────

/// Tool that lists jobs for a pipeline.
pub struct GitlabListJobsTool;

#[async_trait::async_trait]
impl Tool for GitlabListJobsTool {
    fn name(&self) -> &'static str {
        "gitlab_list_jobs"
    }

    fn description(&self) -> &'static str {
        "List jobs in a GitLab pipeline. Shows each job's name, stage, \
         status, and duration."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pipeline_id": {
                    "type": "integer",
                    "description": "The pipeline ID to list jobs for"
                },
                "scope": {
                    "type": "string",
                    "enum": [
                        "created", "pending", "running", "failed",
                        "success", "canceled", "skipped", "manual"
                    ],
                    "description": "Filter jobs by scope/status"
                }
            },
            "required": ["pipeline_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let pipeline_id = input["pipeline_id"]
            .as_u64()
            .context("pipeline_id is required")?;

        let mut path = format!(
            "/projects/{project}/pipelines/{pipeline_id}/jobs?per_page=100"
        );
        if let Some(scope) = input["scope"].as_str() {
            path.push_str(&format!("&scope[]={scope}"));
        }

        let jobs = client.get(&path).await?;
        let arr = jobs
            .as_array()
            .context("Expected array from GitLab jobs endpoint")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: format!("No jobs found for pipeline #{pipeline_id}."),
                metadata: None,
            });
        }

        let mut lines = vec![format!("Jobs for pipeline #{pipeline_id}:\n")];
        for job in arr {
            let id = job["id"].as_u64().unwrap_or(0);
            let name = job["name"].as_str().unwrap_or("?");
            let stage = job["stage"].as_str().unwrap_or("?");
            let status = job["status"].as_str().unwrap_or("?");
            let duration = job["duration"]
                .as_f64()
                .map(|d| format!("{d:.1}s"))
                .unwrap_or_else(|| "-".to_string());
            let runner = job["runner"]["description"]
                .as_str()
                .unwrap_or("no runner");
            lines.push(format!(
                "  {} [{stage}] {name} (id:{id}) — {status}, {duration}, runner:{runner}",
                status_icon(status)
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({"count": arr.len(), "pipeline_id": pipeline_id})),
        })
    }
}

// ── GitlabGetJobTool ─────────────────────────────────────────────────────────

/// Tool that retrieves details of a specific job.
pub struct GitlabGetJobTool;

#[async_trait::async_trait]
impl Tool for GitlabGetJobTool {
    fn name(&self) -> &'static str {
        "gitlab_get_job"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitLab CI/CD job including status, \
         stage, artifacts, runner info, and timing."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "integer",
                    "description": "The job ID"
                }
            },
            "required": ["job_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let job_id = input["job_id"]
            .as_u64()
            .context("job_id is required")?;

        let job = client
            .get(&format!("/projects/{project}/jobs/{job_id}"))
            .await?;

        let name = job["name"].as_str().unwrap_or("?");
        let stage = job["stage"].as_str().unwrap_or("?");
        let status = job["status"].as_str().unwrap_or("?");
        let git_ref = job["ref"].as_str().unwrap_or("?");
        let created = job["created_at"].as_str().unwrap_or("?");
        let started = job["started_at"].as_str().unwrap_or("not started");
        let finished = job["finished_at"].as_str().unwrap_or("not finished");
        let duration = job["duration"]
            .as_f64()
            .map(|d| format!("{d:.1}s"))
            .unwrap_or_else(|| "N/A".to_string());
        let web_url = job["web_url"].as_str().unwrap_or("?");
        let runner_desc = job["runner"]["description"]
            .as_str()
            .unwrap_or("none");
        let pipeline_id = job["pipeline"]["id"].as_u64().unwrap_or(0);
        let failure_reason = job["failure_reason"].as_str().unwrap_or("");
        let allow_failure = job["allow_failure"].as_bool().unwrap_or(false);

        let artifacts = job["artifacts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|art| art["filename"].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let mut content = format!(
            "{icon} Job: {name} (id:{job_id})\n\
             Stage:         {stage}\n\
             Status:        {status}\n\
             Ref:           {git_ref}\n\
             Pipeline:      #{pipeline_id}\n\
             Allow failure: {allow_failure}\n\
             Created:       {created}\n\
             Started:       {started}\n\
             Finished:      {finished}\n\
             Duration:      {duration}\n\
             Runner:        {runner_desc}\n\
             URL:           {web_url}",
            icon = status_icon(status),
        );

        if !failure_reason.is_empty() {
            content.push_str(&format!("\nFailure:       {failure_reason}"));
        }
        if !artifacts.is_empty() {
            content.push_str(&format!("\nArtifacts:     {artifacts}"));
        }

        Ok(ToolOutput {
            content,
            metadata: Some(json!({"job_id": job_id, "status": status})),
        })
    }
}

// ── GitlabGetJobLogTool ──────────────────────────────────────────────────────

/// Tool that downloads the log (trace) output of a job.
pub struct GitlabGetJobLogTool;

#[async_trait::async_trait]
impl Tool for GitlabGetJobLogTool {
    fn name(&self) -> &'static str {
        "gitlab_get_job_log"
    }

    fn description(&self) -> &'static str {
        "Download the log output of a GitLab CI/CD job. Returns the last \
         N lines of the job trace (default 200, max 2000)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "integer",
                    "description": "The job ID"
                },
                "tail": {
                    "type": "integer",
                    "description": "Number of lines from the end to return (default 200, max 2000)"
                }
            },
            "required": ["job_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx
            .storage
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Storage not available"))?;
        let client_obj = GitLabClient::new(storage)
            .map_err(|_| anyhow::anyhow!("GitLab not configured. Run /gitlab setup."))?;
        let project = GitLabClient::detect_project(&ctx.working_dir)
            .ok_or_else(|| anyhow::anyhow!("Could not detect GitLab project."))?;

        let job_id = input["job_id"]
            .as_u64()
            .context("job_id is required")?;
        let tail = input["tail"].as_u64().unwrap_or(200).min(2000) as usize;

        // The job trace endpoint returns plain text, not JSON.
        let url = format!(
            "{}/api/v4/projects/{project}/jobs/{job_id}/trace",
            client_obj.instance_url()
        );
        let resp = reqwest::Client::new()
            .get(&url)
            .header("PRIVATE-TOKEN", &super::super::gitlab::auth::load_token(storage)
                .context("No GitLab token")?)
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .context("Failed to fetch job trace")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitLab API error {status} fetching job trace: {body}");
        }

        let full_log = resp.text().await.context("Failed to read job trace")?;

        // Return last `tail` lines
        let lines: Vec<&str> = full_log.lines().collect();
        let start = lines.len().saturating_sub(tail);
        let truncated = start > 0;
        let output_lines = &lines[start..];

        let mut content = String::new();
        if truncated {
            content.push_str(&format!(
                "--- showing last {tail} of {} lines ---\n",
                lines.len()
            ));
        }
        content.push_str(&output_lines.join("\n"));

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "job_id": job_id,
                "total_lines": lines.len(),
                "returned_lines": output_lines.len(),
                "truncated": truncated,
            })),
        })
    }
}

// ── GitlabRetryJobTool ───────────────────────────────────────────────────────

/// Tool that retries a failed or cancelled job.
pub struct GitlabRetryJobTool;

#[async_trait::async_trait]
impl Tool for GitlabRetryJobTool {
    fn name(&self) -> &'static str {
        "gitlab_retry_job"
    }

    fn description(&self) -> &'static str {
        "Retry a failed or cancelled GitLab CI/CD job."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "integer",
                    "description": "The job ID to retry"
                }
            },
            "required": ["job_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let job_id = input["job_id"]
            .as_u64()
            .context("job_id is required")?;

        let result = client
            .post(
                &format!("/projects/{project}/jobs/{job_id}/retry"),
                &json!({}),
            )
            .await?;

        let new_id = result["id"].as_u64().unwrap_or(0);
        let status = result["status"].as_str().unwrap_or("?");
        let name = result["name"].as_str().unwrap_or("?");

        Ok(ToolOutput {
            content: format!(
                "Retried job '{name}': new job id={new_id}, status={status}"
            ),
            metadata: Some(json!({"new_job_id": new_id, "status": status})),
        })
    }
}

// ── GitlabCancelJobTool ──────────────────────────────────────────────────────

/// Tool that cancels a running or pending job.
pub struct GitlabCancelJobTool;

#[async_trait::async_trait]
impl Tool for GitlabCancelJobTool {
    fn name(&self) -> &'static str {
        "gitlab_cancel_job"
    }

    fn description(&self) -> &'static str {
        "Cancel a running or pending GitLab CI/CD job."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "integer",
                    "description": "The job ID to cancel"
                }
            },
            "required": ["job_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let job_id = input["job_id"]
            .as_u64()
            .context("job_id is required")?;

        let result = client
            .post(
                &format!("/projects/{project}/jobs/{job_id}/cancel"),
                &json!({}),
            )
            .await?;

        let status = result["status"].as_str().unwrap_or("?");
        let name = result["name"].as_str().unwrap_or("?");

        Ok(ToolOutput {
            content: format!("Cancelled job '{name}' (id:{job_id}), status={status}"),
            metadata: Some(json!({"job_id": job_id, "status": status})),
        })
    }
}

// ── GitlabRetryPipelineTool ──────────────────────────────────────────────────

/// Tool that retries all failed jobs in a pipeline.
pub struct GitlabRetryPipelineTool;

#[async_trait::async_trait]
impl Tool for GitlabRetryPipelineTool {
    fn name(&self) -> &'static str {
        "gitlab_retry_pipeline"
    }

    fn description(&self) -> &'static str {
        "Retry all failed jobs in a GitLab CI/CD pipeline."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pipeline_id": {
                    "type": "integer",
                    "description": "The pipeline ID to retry"
                }
            },
            "required": ["pipeline_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let pipeline_id = input["pipeline_id"]
            .as_u64()
            .context("pipeline_id is required")?;

        let result = client
            .post(
                &format!("/projects/{project}/pipelines/{pipeline_id}/retry"),
                &json!({}),
            )
            .await?;

        let status = result["status"].as_str().unwrap_or("?");

        Ok(ToolOutput {
            content: format!(
                "Retried failed jobs in pipeline #{pipeline_id}, status={status}"
            ),
            metadata: Some(json!({"pipeline_id": pipeline_id, "status": status})),
        })
    }
}

// ── GitlabCancelPipelineTool ─────────────────────────────────────────────────

/// Tool that cancels all running/pending jobs in a pipeline.
pub struct GitlabCancelPipelineTool;

#[async_trait::async_trait]
impl Tool for GitlabCancelPipelineTool {
    fn name(&self) -> &'static str {
        "gitlab_cancel_pipeline"
    }

    fn description(&self) -> &'static str {
        "Cancel a running GitLab CI/CD pipeline (cancels all pending and running jobs)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pipeline_id": {
                    "type": "integer",
                    "description": "The pipeline ID to cancel"
                }
            },
            "required": ["pipeline_id"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let pipeline_id = input["pipeline_id"]
            .as_u64()
            .context("pipeline_id is required")?;

        let result = client
            .post(
                &format!("/projects/{project}/pipelines/{pipeline_id}/cancel"),
                &json!({}),
            )
            .await?;

        let status = result["status"].as_str().unwrap_or("?");

        Ok(ToolOutput {
            content: format!(
                "Cancelled pipeline #{pipeline_id}, status={status}"
            ),
            metadata: Some(json!({"pipeline_id": pipeline_id, "status": status})),
        })
    }
}
