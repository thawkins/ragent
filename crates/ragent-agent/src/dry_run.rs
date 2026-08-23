//! Dry-run readiness report for ragent deployments.
//!
//! Implements `ragent --dry-run` and `ragent config check`.  The report loads
//! configuration, resolves provider/model auth state, discovers skills,
//! enumerates visible tools, and performs lightweight MCP connectivity checks
//! without invoking any LLM or executing any tool.  The result is returned as a
//! structured [`ReadinessReport`] that can be printed for humans or serialised
//! as JSON for CI/CD consumption.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command as TokioCommand;

use crate::{
    Config, ProviderRegistry,
    agent::{self},
    skill::SkillRegistry,
    tool::ToolRegistry,
};
use crate::{McpServerConfig, McpTransport};

/// High-level readiness verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReadinessVerdict {
    /// All checks passed.
    Ready,
    /// Non-fatal problems were detected; deployment can still run.
    Warning,
    /// Fatal problems were detected; the deployment is not usable.
    Blocked,
}

impl std::fmt::Display for ReadinessVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "READY"),
            Self::Warning => write!(f, "WARNING"),
            Self::Blocked => write!(f, "BLOCKED"),
        }
    }
}

/// Per-item or per-section status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    /// Check passed.
    Ready,
    /// Non-fatal issue.
    Warning,
    /// Fatal issue.
    Blocked,
}

impl From<ReadinessVerdict> for ReadinessStatus {
    fn from(v: ReadinessVerdict) -> Self {
        match v {
            ReadinessVerdict::Ready => Self::Ready,
            ReadinessVerdict::Warning => Self::Warning,
            ReadinessVerdict::Blocked => Self::Blocked,
        }
    }
}

impl std::fmt::Display for ReadinessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::Warning => write!(f, "warning"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

/// A single check result inside a readiness section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessItem {
    /// Short name for the check (e.g. file path, provider id, MCP server id).
    pub name: String,
    /// Status of this specific check.
    pub status: ReadinessStatus,
    /// Human-readable explanation.
    pub message: String,
}

/// A group of related readiness checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessSection {
    /// Section identifier: `config`, `provider`, `skills`, `tools`, or `mcp`.
    pub name: String,
    /// Aggregated status for the section.
    pub status: ReadinessStatus,
    /// Individual checks within the section.
    pub items: Vec<ReadinessItem>,
}

/// Complete dry-run readiness report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// Overall deployment verdict.
    pub verdict: ReadinessVerdict,
    /// Per-section check results.
    pub sections: Vec<ReadinessSection>,
}

impl ReadinessReport {
    /// Serialise the report as a pretty-printed JSON object.
    ///
    /// # Errors
    ///
    /// Returns an error if the report cannot be serialised to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Render the report for human consumption.
    #[must_use]
    pub fn to_human_string(&self) -> String {
        let mut out = String::new();
        for section in &self.sections {
            out.push_str(&format!("{}: {}\n", section.name, section.status));
            for item in &section.items {
                out.push_str(&format!(
                    "  [{:>7}] {}: {}\n",
                    item.status.to_string(),
                    item.name,
                    item.message
                ));
            }
        }
        out.push_str(&format!("\nVerdict: {}\n", self.verdict));
        out
    }
}

/// Inputs required to run a dry-run readiness check.
pub struct DryRunInputs {
    /// Optional explicit config file path (`--config`).  When supplied, this
    /// file is loaded in place of the normal global/project files.
    pub config_path: Option<PathBuf>,
    /// Agent name to resolve.
    pub agent_name: String,
    /// Optional `provider/model` override (`--model`).
    pub model_override: Option<String>,
    /// Resolved provider registry.
    pub provider_registry: Arc<ProviderRegistry>,
    /// Resolved tool registry.
    pub tool_registry: Arc<ToolRegistry>,
    /// Current working directory for skill discovery and project config
    /// resolution.
    pub working_dir: PathBuf,
    /// Tool names hidden from the LLM (computed from `tool_visibility`).
    pub hidden_tools: Vec<String>,
}

/// Run the full dry-run readiness check and return the report plus an exit
/// code suitable for CI/CD.
///
/// Exit codes: `0` for `READY` or `WARNING`, `1` for `BLOCKED`.
pub async fn run_dry_run(inputs: DryRunInputs) -> (ReadinessReport, u8) {
    let mut sections = Vec::new();

    let (config, config_section) = load_config_section(&inputs.config_path, &inputs.working_dir);
    sections.push(config_section);

    let provider_section = provider_section(&config, &inputs);
    sections.push(provider_section);

    let skills_section = skills_section(&config, &inputs.working_dir);
    sections.push(skills_section);

    let tools_section = tools_section(&inputs.tool_registry, &inputs.hidden_tools);
    sections.push(tools_section);

    let mcp_section = mcp_section(&config).await;
    sections.push(mcp_section);

    let verdict = compute_verdict(&sections);
    let exit_code = match verdict {
        ReadinessVerdict::Blocked => 1,
        ReadinessVerdict::Ready | ReadinessVerdict::Warning => 0,
    };

    let report = ReadinessReport { verdict, sections };
    (report, exit_code)
}

fn compute_verdict(sections: &[ReadinessSection]) -> ReadinessVerdict {
    let mut worst = ReadinessVerdict::Ready;
    for section in sections {
        match section.status {
            ReadinessStatus::Blocked => return ReadinessVerdict::Blocked,
            ReadinessStatus::Warning => worst = ReadinessVerdict::Warning,
            ReadinessStatus::Ready => {}
        }
    }
    worst
}

fn section_status_from_items(items: &[ReadinessItem]) -> ReadinessStatus {
    let mut status = ReadinessStatus::Ready;
    for item in items {
        match item.status {
            ReadinessStatus::Blocked => return ReadinessStatus::Blocked,
            ReadinessStatus::Warning => status = ReadinessStatus::Warning,
            ReadinessStatus::Ready => {}
        }
    }
    status
}

fn load_config_section(
    config_path: &Option<PathBuf>,
    working_dir: &Path,
) -> (Config, ReadinessSection) {
    let mut config = Config::default();
    let mut items: Vec<ReadinessItem> = Vec::new();
    let mut loaded = false;

    // Optional explicit file passed via `--config`.
    if let Some(path) = config_path {
        match load_config_file(path) {
            Ok(overlay) => {
                config = Config::merge(config, overlay);
                loaded = true;
                items.push(ReadinessItem {
                    name: path.display().to_string(),
                    status: ReadinessStatus::Ready,
                    message: "Config loaded".to_string(),
                });
            }
            Err(e) => items.push(ReadinessItem {
                name: path.display().to_string(),
                status: ReadinessStatus::Blocked,
                message: e,
            }),
        }
    }

    // Global config: ~/.config/ragent/ragent.json
    if let Some(config_dir) = dirs::config_dir() {
        let path = config_dir.join("ragent").join("ragent.json");
        if path.exists() {
            match load_config_file(&path) {
                Ok(overlay) => {
                    config = Config::merge(config, overlay);
                    loaded = true;
                    items.push(ReadinessItem {
                        name: path.display().to_string(),
                        status: ReadinessStatus::Ready,
                        message: "Config loaded".to_string(),
                    });
                }
                Err(e) => items.push(ReadinessItem {
                    name: path.display().to_string(),
                    status: ReadinessStatus::Blocked,
                    message: e,
                }),
            }
        }
    }

    // Project config: {working_dir}/.ragent/ragent.json
    let project_path = working_dir.join(".ragent").join("ragent.json");
    if project_path.exists() {
        match load_config_file(&project_path) {
            Ok(overlay) => {
                config = Config::merge(config, overlay);
                loaded = true;
                items.push(ReadinessItem {
                    name: project_path.display().to_string(),
                    status: ReadinessStatus::Ready,
                    message: "Config loaded".to_string(),
                });
            }
            Err(e) => items.push(ReadinessItem {
                name: project_path.display().to_string(),
                status: ReadinessStatus::Blocked,
                message: e,
            }),
        }
    }

    // Environment variable pointing to a config file.
    if let Ok(env_path_str) = std::env::var("RAGENT_CONFIG") {
        let path = PathBuf::from(&env_path_str);
        if path.exists() {
            match load_config_file(&path) {
                Ok(overlay) => {
                    config = Config::merge(config, overlay);
                    loaded = true;
                    items.push(ReadinessItem {
                        name: path.display().to_string(),
                        status: ReadinessStatus::Ready,
                        message: "Config loaded".to_string(),
                    });
                }
                Err(e) => items.push(ReadinessItem {
                    name: path.display().to_string(),
                    status: ReadinessStatus::Blocked,
                    message: e,
                }),
            }
        }
    }

    // Inline config from environment variable.
    if let Ok(content) = std::env::var("RAGENT_CONFIG_CONTENT") {
        match parse_config_str(&content, "RAGENT_CONFIG_CONTENT") {
            Ok(overlay) => {
                config = Config::merge(config, overlay);
                loaded = true;
                items.push(ReadinessItem {
                    name: "RAGENT_CONFIG_CONTENT".to_string(),
                    status: ReadinessStatus::Ready,
                    message: "Inline config loaded".to_string(),
                });
            }
            Err(e) => items.push(ReadinessItem {
                name: "RAGENT_CONFIG_CONTENT".to_string(),
                status: ReadinessStatus::Blocked,
                message: e,
            }),
        }
    }

    if !loaded && items.is_empty() {
        items.push(ReadinessItem {
            name: "config".to_string(),
            status: ReadinessStatus::Ready,
            message: "No config files found; using defaults".to_string(),
        });
    }

    config.config_paths.clear();
    let status = section_status_from_items(&items);
    (
        config,
        ReadinessSection {
            name: "config".to_string(),
            status,
            items,
        },
    )
}

fn load_config_file(path: &Path) -> Result<Config, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config file '{}': {e}", path.display()))?;
    parse_config_str(&content, &path.display().to_string())
}

fn parse_config_str(content: &str, source: &str) -> Result<Config, String> {
    serde_json::from_str(content).map_err(|e| {
        let line = e.line();
        let column = e.column();
        let problematic_line = content
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("<line not found>");
        format!(
            "Failed to parse {source} at line {line}, column {column}: {e}\nProblematic line: {problematic_line}"
        )
    })
}

fn provider_section(config: &Config, inputs: &DryRunInputs) -> ReadinessSection {
    let mut items: Vec<ReadinessItem> = Vec::new();

    let mut agent = match agent::resolve_agent_with_model(
        &inputs.agent_name,
        config,
        inputs.provider_registry.as_ref(),
    ) {
        Ok(a) => a,
        Err(e) => {
            return ReadinessSection {
                name: "provider".to_string(),
                status: ReadinessStatus::Blocked,
                items: vec![ReadinessItem {
                    name: inputs.agent_name.clone(),
                    status: ReadinessStatus::Blocked,
                    message: format!("Failed to resolve agent/model: {e}"),
                }],
            };
        }
    };
    if let Some(model_str) = &inputs.model_override {
        if let Some((provider, model)) = model_str.split_once('/') {
            Arc::make_mut(&mut agent).model = Some(agent::ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        } else {
            items.push(ReadinessItem {
                name: "model_override".to_string(),
                status: ReadinessStatus::Blocked,
                message: format!("Invalid --model format '{model_str}'. Expected 'provider/model'"),
            });
        }
    }

    let Some(ref model) = agent.model else {
        return ReadinessSection {
            name: "provider".to_string(),
            status: ReadinessStatus::Blocked,
            items: vec![ReadinessItem {
                name: agent.name.clone(),
                status: ReadinessStatus::Blocked,
                message: "No provider/model could be resolved".to_string(),
            }],
        };
    };

    items.push(ReadinessItem {
        name: "resolved".to_string(),
        status: ReadinessStatus::Ready,
        message: format!(
            "{} uses {}/{}",
            agent.name, model.provider_id, model.model_id
        ),
    });

    let provider_config = config.provider.get(&model.provider_id).cloned();
    let required_env: Vec<String> = provider_config
        .as_ref()
        .map(|p| p.env.clone())
        .unwrap_or_default();

    if required_env.is_empty() {
        items.push(ReadinessItem {
            name: model.provider_id.clone(),
            status: ReadinessStatus::Ready,
            message: "No required environment variables configured".to_string(),
        });
    } else {
        let mut missing = Vec::new();
        for var in &required_env {
            if std::env::var(var)
                .ok()
                .as_ref()
                .is_none_or(|v| v.trim().is_empty())
            {
                missing.push(var.clone());
            }
        }
        if missing.is_empty() {
            items.push(ReadinessItem {
                name: model.provider_id.clone(),
                status: ReadinessStatus::Ready,
                message: format!(
                    "All required environment variables present: {}",
                    required_env.join(", ")
                ),
            });
        } else {
            items.push(ReadinessItem {
                name: model.provider_id.clone(),
                status: ReadinessStatus::Warning,
                message: format!(
                    "Missing environment variables for {}/{}: {}",
                    model.provider_id,
                    model.model_id,
                    missing.join(", ")
                ),
            });
        }
    }

    let status = section_status_from_items(&items);
    ReadinessSection {
        name: "provider".to_string(),
        status,
        items,
    }
}

fn skills_section(config: &Config, working_dir: &Path) -> ReadinessSection {
    let registry = SkillRegistry::load(working_dir, &config.skill_dirs);
    let mut items = Vec::new();

    items.push(ReadinessItem {
        name: "total".to_string(),
        status: ReadinessStatus::Ready,
        message: format!("{} skills registered", registry.len()),
    });
    items.push(ReadinessItem {
        name: "bundled".to_string(),
        status: ReadinessStatus::Ready,
        message: format!("{} bundled skills", registry.bundled_count()),
    });
    items.push(ReadinessItem {
        name: "discovered".to_string(),
        status: ReadinessStatus::Ready,
        message: format!("{} discovered skills", registry.discovered_count()),
    });

    for dir in &config.skill_dirs {
        let path = Path::new(dir);
        if !path.exists() {
            items.push(ReadinessItem {
                name: dir.clone(),
                status: ReadinessStatus::Warning,
                message: "Configured skill directory does not exist".to_string(),
            });
        } else if !path.is_dir() {
            items.push(ReadinessItem {
                name: dir.clone(),
                status: ReadinessStatus::Warning,
                message: "Configured skill_dirs entry is not a directory".to_string(),
            });
        }
    }

    let status = section_status_from_items(&items);
    ReadinessSection {
        name: "skills".to_string(),
        status,
        items,
    }
}

fn tools_section(tool_registry: &ToolRegistry, hidden_tools: &[String]) -> ReadinessSection {
    tool_registry.set_hidden(hidden_tools);
    let names = tool_registry.list();
    let mut family_counts: HashMap<String, usize> = HashMap::new();
    let mut unknown_category = 0usize;

    for name in &names {
        if let Some(tool) = tool_registry.get(name) {
            let category = tool.permission_category();
            let family = category.split(':').next().unwrap_or(category).to_string();
            if family.is_empty() {
                unknown_category += 1;
            } else {
                *family_counts.entry(family).or_insert(0) += 1;
            }
        }
    }

    let mut families: Vec<(String, usize)> = family_counts.into_iter().collect();
    families.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items = Vec::new();
    items.push(ReadinessItem {
        name: "total".to_string(),
        status: ReadinessStatus::Ready,
        message: format!("{} visible tools", names.len()),
    });

    for (family, count) in families {
        items.push(ReadinessItem {
            name: family,
            status: ReadinessStatus::Ready,
            message: format!("{count} tools"),
        });
    }

    if unknown_category > 0 {
        items.push(ReadinessItem {
            name: "uncategorized".to_string(),
            status: ReadinessStatus::Warning,
            message: format!("{unknown_category} tools with no family prefix"),
        });
    }

    ReadinessSection {
        name: "tools".to_string(),
        status: ReadinessStatus::Ready,
        items,
    }
}

async fn mcp_section(config: &Config) -> ReadinessSection {
    let mut items = Vec::new();

    if config.mcp.is_empty() {
        items.push(ReadinessItem {
            name: "mcp".to_string(),
            status: ReadinessStatus::Ready,
            message: "No MCP servers configured".to_string(),
        });
        return ReadinessSection {
            name: "mcp".to_string(),
            status: ReadinessStatus::Ready,
            items,
        };
    }

    let mut servers: Vec<(String, McpServerConfig)> = config
        .mcp
        .iter()
        .map(|(id, cfg)| (id.clone(), cfg.clone()))
        .collect();
    servers.sort_by(|a, b| a.0.cmp(&b.0));

    for (id, cfg) in servers {
        if cfg.disabled {
            items.push(ReadinessItem {
                name: id.clone(),
                status: ReadinessStatus::Ready,
                message: "Server disabled".to_string(),
            });
            continue;
        }

        if let Err(e) = crate::mcp::validate_mcp_config(&id, &cfg) {
            items.push(ReadinessItem {
                name: id.clone(),
                status: ReadinessStatus::Blocked,
                message: format!("Config invalid: {e}"),
            });
            continue;
        }

        match cfg.type_ {
            McpTransport::Stdio => {
                let command = cfg.command.clone().unwrap_or_default();
                let mut cmd = TokioCommand::new(&command);
                cmd.args(&cfg.args).envs(&cfg.env).kill_on_drop(true);
                match cmd.spawn() {
                    Ok(_child) => {
                        // Child is killed when dropped because `kill_on_drop(true)`.
                        items.push(ReadinessItem {
                            name: id.clone(),
                            status: ReadinessStatus::Ready,
                            message: "stdio server spawned successfully".to_string(),
                        });
                    }
                    Err(e) => items.push(ReadinessItem {
                        name: id.clone(),
                        status: ReadinessStatus::Blocked,
                        message: format!("Failed to spawn stdio server: {e}"),
                    }),
                }
            }
            McpTransport::Http | McpTransport::Sse => {
                if let Some(url) = cfg.url.as_deref() {
                    match check_http_mcp(url, &cfg.headers).await {
                        Ok(status) => {
                            items.push(ReadinessItem {
                                name: id.clone(),
                                status: ReadinessStatus::Ready,
                                message: format!("HTTP/SSE reachable (status {status})"),
                            });
                        }
                        Err(e) => items.push(ReadinessItem {
                            name: id.clone(),
                            status: ReadinessStatus::Blocked,
                            message: format!("HTTP/SSE connectivity failed: {e}"),
                        }),
                    }
                } else {
                    items.push(ReadinessItem {
                        name: id.clone(),
                        status: ReadinessStatus::Blocked,
                        message: "HTTP/SSE transport requires a url".to_string(),
                    });
                }
            }
        }
    }

    let status = section_status_from_items(&items);
    ReadinessSection {
        name: "mcp".to_string(),
        status,
        items,
    }
}

async fn check_http_mcp(url: &str, headers: &HashMap<String, String>) -> Result<u16, String> {
    let client = reqwest::Client::new();
    let mut request = client.head(url).timeout(Duration::from_secs(5));
    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = response.status().as_u16();
    if response.status().is_success() || response.status().is_redirection() {
        Ok(status)
    } else {
        Err(format!("HTTP {status}"))
    }
}
