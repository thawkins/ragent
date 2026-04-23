//! `team_spawn` — Spawn a named teammate session within an existing team.
//!
//! Full implementation requires `TeamManager` (M3). This stub validates
//! parameters and returns an informative error until M3 is wired in.

use anyhow::Result;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::event::Event;
use crate::tool::metadata::MetadataBuilder;

use super::{Tool, ToolContext, ToolOutput};

/// Spawns a named teammate into an existing team.
pub struct TeamSpawnTool;

#[async_trait::async_trait]
impl Tool for TeamSpawnTool {
    fn name(&self) -> &'static str {
        "team_spawn"
    }

    fn description(&self) -> &'static str {
        "Spawn a teammate agent session within an existing team. \
         Each teammate receives the team context and works on a single, bounded task. \
         CRITICAL: Spawn ONE teammate per independent work item — never assign a list of \
         items to one teammate (context overflow). After spawning all teammates in the same \
         response turn, call `team_wait` to block until they finish. \
         Do NOT use `wait_tasks` for teammates — use `team_wait`."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to spawn the teammate into"
                },
                "teammate_name": {
                    "type": "string",
                    "description": "Unique name for this teammate within the team (e.g. 'security-reviewer')"
                },
                "agent_type": {
                    "type": "string",
                    "description": "Agent type / definition name (e.g. 'general', 'explore')"
                },
                "prompt": {
                    "type": "string",
                    "description": "Initial task prompt for the teammate. Must be scoped to a SINGLE work item — never list multiple items. Keep under ~500 words; reference files by path rather than pasting content."
                },
                "task_id": {
                    "type": "string",
                    "description": "Optional task ID to pre-assign to this teammate. If provided, the task is claimed on their behalf; they should use team_task_complete when done."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override in 'provider_id/model_id' format (e.g. 'anthropic/claude-sonnet-4-20250514'). If omitted, the teammate inherits the lead session's model."
                },
                "memory": {
                    "type": "string",
                    "enum": ["user", "project", "none"],
                    "description": "Persistent memory scope: 'user' (global), 'project' (local), or 'none' (default). Gives the teammate a memory directory for cross-session notes."
                }
            },
            "required": ["team_name", "teammate_name", "agent_type", "prompt"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        use crate::agent::ModelRef;

        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let teammate_name = input
            .get("teammate_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: teammate_name"))?;

        let agent_type = input
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: prompt"))?;

        // Guard: detect multi-item prompts that violate the "one per spawn" rule.
        // Only flag when there is strong evidence of an actual enumerated list
        // (3+ numbered items, or 3+ bullet items). Single connectives like "and"
        // or "or" in natural prose are NOT multi-item violations.
        let multi_item_detected = detect_multi_item_list(prompt);

        if multi_item_detected {
            // Ask the user for permission to override instead of hard-blocking.
            let request_id = uuid::Uuid::new_v4().to_string();
            let mut rx = ctx.event_bus.subscribe();

            let description = format!(
                "Teammate '{}' prompt appears to contain a multi-item list.\n\
                           RULE: Each team_spawn should cover exactly ONE work item.\n\
                           Allow this spawn anyway?",
                teammate_name,
            );

            tracing::info!(
                session_id = %ctx.session_id,
                request_id = %request_id,
                teammate = %teammate_name,
                "Publishing PermissionRequested for team:spawn_override"
            );

            ctx.event_bus.publish(Event::PermissionRequested {
                session_id: ctx.session_id.clone(),
                request_id: request_id.clone(),
                permission: "team:spawn_override".to_string(),
                description,
                options: vec![],
            });

            // Wait for the user's decision with a timeout to prevent indefinite blocking.
            let allowed = tokio::time::timeout(
                std::time::Duration::from_secs(300), // 5 minute timeout
                async {
                    loop {
                        match rx.recv().await {
                            Ok(Event::PermissionReplied {
                                session_id: ref s,
                                request_id: ref rid,
                                allowed,
                                ..
                            }) if s == &ctx.session_id && rid == &request_id => {
                                tracing::info!(
                                    session_id = %ctx.session_id,
                                    request_id = %request_id,
                                    allowed = allowed,
                                    "Received PermissionReplied for team:spawn_override"
                                );
                                break allowed;
                            }
                            Ok(Event::PermissionReplied {
                                session_id: ref s,
                                request_id: ref rid,
                                allowed: _,
                                ..
                            }) => {
                                tracing::debug!(
                                    expected_session = %ctx.session_id,
                                    received_session = %s,
                                    expected_request = %request_id,
                                    received_request = %rid,
                                    "Ignoring mismatched PermissionReplied event"
                                );
                            }
                            Ok(other) => {
                                tracing::trace!(
                                    "Received other event while waiting for permission: {:?}",
                                    other.type_name()
                                );
                            }
                            Err(RecvError::Lagged(n)) => {
                                tracing::warn!("Event bus lagged, dropped {} events", n);
                            }
                            Err(RecvError::Closed) => {
                                tracing::error!("Event bus closed while waiting for permission");
                                return false; // Deny on error
                            }
                        }
                    }
                },
            )
            .await;

            let allowed = match allowed {
                Ok(result) => result,
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Timeout waiting for user permission to spawn teammate '{}'. Permission dialog may not have been shown.",
                        teammate_name,
                    ));
                }
            };

            if !allowed {
                return Err(anyhow::anyhow!(
                    "Spawn for teammate '{}' denied by user — prompt appears to \
                               contain multiple work items. Split into separate team_spawn calls.",
                    teammate_name,
                ));
            }
        }
        // Parse optional per-teammate model override ("provider_id/model_id").
        let teammate_model: Option<ModelRef> =
            input.get("model").and_then(|v| v.as_str()).and_then(|s| {
                s.split_once('/')
                    .or_else(|| s.split_once(':'))
                    .map(|(p, m)| ModelRef {
                        provider_id: p.to_string(),
                        model_id: m.to_string(),
                    })
            });

        // Parse optional memory scope.
        let memory_scope = match input.get("memory").and_then(|v| v.as_str()) {
            Some("user") => crate::team::MemoryScope::User,
            Some("project") => crate::team::MemoryScope::Project,
            _ => crate::team::MemoryScope::None,
        };

        // TeamManager is wired in M3. Until then, return a clear pending message.
        if ctx.team_manager.is_none() {
            tracing::warn!(
                team = %team_name,
                teammate = %teammate_name,
                session = %ctx.session_id,
                "TeamManager missing when attempting to spawn teammate; returning pending_manager"
            );
            return Ok(ToolOutput {
                content: format!(
                    "Teammate '{teammate_name}' queued for team '{team_name}' \
                     (agent_type: {agent_type}).\n\
                     Note: TeamManager not yet initialised — teammate will be spawned \
                     when the session processor is upgraded to M3."
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "teammate_name": teammate_name,
                    "agent_type": agent_type,
                    "status": "pending_manager"
                })),
            });
        }

        let manager = ctx.team_manager.as_ref().expect("checked above");
        let agent_id = manager
            .spawn_teammate(
                team_name,
                teammate_name,
                agent_type,
                input.get("prompt").and_then(|v| v.as_str()).unwrap_or(""),
                teammate_model.as_ref(),
                ctx.active_model.as_ref(),
                &ctx.working_dir,
            )
            .await?;

        // Extract optional task_id and pre-assign to the teammate if provided.
        let task_id = input.get("task_id").and_then(|v| v.as_str());
        let mut task_assignment_msg = String::new();

        if let Some(task_id) = task_id
            && let Some(team_dir) = crate::team::find_team_dir(&ctx.working_dir, team_name)
            && let Ok(task_store) = crate::team::task::TaskStore::open(&team_dir)
        {
            match task_store.pre_assign_task(task_id, &agent_id) {
                Ok(_) => {
                    task_assignment_msg =
                        format!("\n📋 Task '{task_id}' pre-assigned to this teammate.");
                    tracing::info!(
                        agent_id = %agent_id,
                        task_id = %task_id,
                        team = %team_name,
                        "Task pre-assigned to teammate"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        agent_id = %agent_id,
                        task_id = %task_id,
                        error = %e,
                        "Failed to pre-assign task to teammate"
                    );
                    task_assignment_msg =
                        format!("\n⚠️ Failed to pre-assign task '{task_id}': {e}");
                }
            }
        }

        // Persist memory scope on the member record.
        if memory_scope != crate::team::MemoryScope::None
            && let Some(team_dir) = crate::team::find_team_dir(&ctx.working_dir, team_name)
            && let Ok(mut store) = crate::team::TeamStore::load(&team_dir)
        {
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.memory_scope = memory_scope;
            }
            let _ = store.save();
        }

        let model_display = teammate_model
            .as_ref()
            .map(|m| format!("{}/{}", m.provider_id, m.model_id))
            .or_else(|| {
                ctx.active_model
                    .as_ref()
                    .map(|m| format!("{}/{} (inherited)", m.provider_id, m.model_id))
            })
            .unwrap_or_else(|| "default".to_string());

        Ok(ToolOutput {
            content: format!(
                "Teammate '{teammate_name}' spawned in team '{team_name}'.\nAgent ID: {agent_id}\n\
                           Model: {model_display}{task_assignment_msg}\n\
                           ⏳ Teammate is now working. Call `team_wait` (not `wait_tasks`) after all spawns \
                           to block until teammates finish before the lead continues."
            ),
            metadata: MetadataBuilder::new()
                .custom("team_name", team_name)
                .custom("teammate_name", teammate_name)
                .custom("agent_id", &agent_id)
                .custom("model", model_display)
                .task_id(task_id.unwrap_or(""))
                .custom("status", "spawned")
                .build(),
        })
    }
}

/// Detects whether a prompt contains a genuine multi-item enumerated list.
///
/// Returns `true` only when there is strong structural evidence of 3+ numbered
/// items (e.g. "1. ... 2. ... 3. ...") or 3+ bullet items (lines starting with
/// "- " or "* "). Single connectives like "and" / "or" in prose do **not**
/// trigger detection — those caused rampant false positives previously.
fn detect_multi_item_list(prompt: &str) -> bool {
    // Count numbered list items: digits followed by a dot and a space at line start.
    let numbered_count = prompt
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.len() >= 3
                && trimmed.as_bytes()[0].is_ascii_digit()
                && trimmed.find(". ").map_or(false, |pos| pos <= 3)
        })
        .count();

    if numbered_count >= 3 {
        return true;
    }

    // Count bullet list items: lines starting with "- " or "* ".
    let bullet_count = prompt
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ") || trimmed.starts_with("* ")
        })
        .count();

    if bullet_count >= 3 {
        return true;
    }

    // Count lettered list items: "a) ", "b) ", etc. at line start.
    let letter_count = prompt
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.len() >= 3
                && trimmed.as_bytes()[0].is_ascii_lowercase()
                && trimmed.as_bytes()[1] == b')'
                && trimmed.as_bytes()[2] == b' '
        })
        .count();

    letter_count >= 3
}
