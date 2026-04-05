//! Swarm — Fleet-style auto-decomposition for ragent teams.
//!
//! A *swarm* takes a high-level prompt (or plan) and uses the LLM to decompose
//! it into independent subtasks with dependency edges.  An ephemeral team is
//! created, one teammate per task group, and the lead orchestrates completion.

use serde::{Deserialize, Serialize};

// ── Decomposition schema ─────────────────────────────────────────────────────

/// A single subtask produced by the LLM decomposition step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSubtask {
    /// Unique ID within the decomposition (e.g. `"s1"`, `"s2"`).
    pub id: String,
    /// Short human-readable title.
    pub title: String,
    /// Full description / instructions for the teammate.
    pub description: String,
    /// IDs of subtasks that must complete before this one can start.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Optional agent type override (defaults to `"general"`).
    #[serde(default)]
    pub agent_type: Option<String>,
    /// Optional model override (`"provider/model"` format).
    #[serde(default)]
    pub model: Option<String>,
}

/// Root of the LLM decomposition response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmDecomposition {
    /// Ordered list of subtasks.
    pub tasks: Vec<SwarmSubtask>,
}

/// Runtime state for an active swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    /// Name of the ephemeral team backing this swarm.
    pub team_name: String,
    /// Original prompt that was decomposed.
    pub prompt: String,
    /// The decomposition produced by the LLM.
    pub decomposition: SwarmDecomposition,
    /// Whether the swarm has been fully spawned.
    pub spawned: bool,
    /// Whether the orchestrator has collected results and finished.
    pub completed: bool,
}

// ── Decomposition prompt ────────────────────────────────────────────────────

/// System prompt sent to the LLM to decompose a user goal into subtasks.
pub const DECOMPOSITION_SYSTEM_PROMPT: &str = r#"You are a task decomposition engine for a multi-agent coding system.

Given a user's goal, break it into the smallest reasonable set of INDEPENDENT subtasks that can be worked on in parallel by separate AI coding agents. Each agent has its own context window and cannot see the others' work.

Rules:
1. Each subtask must be self-contained — an agent must be able to complete it without seeing another agent's output, unless declared as a dependency.
2. Minimise dependencies. Prefer independent tasks that can run in parallel.
3. When a dependency is unavoidable (e.g. "create the API" before "write integration tests"), declare it via `depends_on`.
4. Keep the number of subtasks between 2 and 8. If the goal is simple, use fewer.
5. Each subtask description should be detailed enough for an AI coding agent to implement it without further clarification.
6. Use simple short IDs like "s1", "s2", etc.

Respond with ONLY a JSON object (no markdown fences, no explanation) matching this schema:
{
  "tasks": [
    {
      "id": "s1",
      "title": "Short title",
      "description": "Detailed instructions for the agent...",
      "depends_on": [],
      "agent_type": "general",
      "model": null
    }
  ]
}

The "agent_type" and "model" fields are optional — omit or set to null to use defaults.
"depends_on" is an array of task IDs that must complete first (empty array for independent tasks)."#;

/// Build the user prompt for decomposition, injecting the user's goal.
#[must_use]
pub fn build_decomposition_user_prompt(goal: &str) -> String {
    format!(
        "Decompose the following goal into parallel subtasks for a team of AI coding agents:\n\n{goal}"
    )
}

/// Parse the LLM's JSON response into a `SwarmDecomposition`.
///
/// Handles common LLM quirks: markdown fences, leading/trailing whitespace,
/// and trailing commas.
pub fn parse_decomposition(raw: &str) -> Result<SwarmDecomposition, String> {
    // Strip markdown code fences if present
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    // Remove trailing commas before } or ] (common LLM mistake)
    let cleaned = remove_trailing_commas(json_str);

    serde_json::from_str::<SwarmDecomposition>(&cleaned)
        .map_err(|e| format!("Failed to parse decomposition JSON: {e}\n\nRaw response:\n{raw}"))
}

/// Remove trailing commas before `}` or `]` in JSON.
fn remove_trailing_commas(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for i in 0..len {
        if chars[i] == ',' {
            // Look ahead for the next non-whitespace char
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                continue; // skip this comma
            }
        }
        result.push(chars[i]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean_json() {
        let input = r#"{"tasks":[{"id":"s1","title":"Setup","description":"Do setup","depends_on":[]},{"id":"s2","title":"Build","description":"Build it","depends_on":["s1"]}]}"#;
        let dec = parse_decomposition(input).unwrap();
        assert_eq!(dec.tasks.len(), 2);
        assert_eq!(dec.tasks[0].id, "s1");
        assert_eq!(dec.tasks[1].depends_on, vec!["s1"]);
    }

    #[test]
    fn test_parse_with_markdown_fences() {
        let input = r#"```json
{"tasks":[{"id":"s1","title":"Only task","description":"Do it","depends_on":[]}]}
```"#;
        let dec = parse_decomposition(input).unwrap();
        assert_eq!(dec.tasks.len(), 1);
    }

    #[test]
    fn test_parse_with_trailing_commas() {
        let input = r#"{"tasks":[{"id":"s1","title":"A","description":"B","depends_on":[],},]}"#;
        let dec = parse_decomposition(input).unwrap();
        assert_eq!(dec.tasks.len(), 1);
    }

    #[test]
    fn test_parse_invalid_json() {
        let input = "not json at all";
        assert!(parse_decomposition(input).is_err());
    }

    #[test]
    fn test_build_user_prompt() {
        let prompt = build_decomposition_user_prompt("Build a REST API");
        assert!(prompt.contains("Build a REST API"));
        assert!(prompt.contains("Decompose"));
    }
}
