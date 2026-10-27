//! Extra coverage for `agent/mod.rs`, `agent/custom.rs`, `skill/*`, and
//! `hooks/mod.rs` that is not exercised by the existing per-area tests.
//!
//! Everything here is additive: temp-dir parsing, pure helpers, and harmless
//! hook commands (`true` / a temp script). No production code is touched.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_agent::agent::custom::{
    find_project_agents_dir, load_custom_agents, record_to_agent_info,
};
use ragent_agent::agent::oasf::{OasfAgentRecord, OasfModule, RAGENT_MODULE_TYPE};
use ragent_agent::agent::{
    AgentInfo, AgentMode, ModelRef, builtin_agents, default_thinking_config_for_levels,
    load_all_agents, resolve_agent, resolve_agent_with_customs,
    resolve_agent_with_customs_and_model, resolve_default_model,
};
use ragent_agent::hooks::{
    HookConfig, HookTrigger, fire_compaction_hooks, fire_hooks, fire_turn_end_hooks,
    fire_turn_start_hooks, merge_hook_configs, parse_hook_configs, plugin_hook_configs,
    run_stop_hooks,
};
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::skill::invoke::{
    ForkedSkillResult, format_forked_result, format_skill_message, invoke_skill, parse_model_ref,
    resolve_inline_skill_agent,
};
use ragent_agent::skill::{SkillRegistry, SkillScope};
use ragent_config::{Capabilities, Cost};
use ragent_plugins::PluginHook;
use ragent_types::ThinkingLevel;
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A single-provider registry whose one provider advertises one model, so
/// [`resolve_default_model`] has something to latch onto without network I/O.
struct StubProvider;

#[async_trait::async_trait]
impl Provider for StubProvider {
    fn id(&self) -> &'static str {
        "stub"
    }
    fn name(&self) -> &'static str {
        "Stub"
    }
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "stub-model".to_string(),
            provider_id: "stub".to_string(),
            name: "Stub Model".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 8_000,
            max_output: None,
            request_multiplier: None,
            thinking_config: None,
        }]
    }
    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn ragent_agent::llm::LlmClient>> {
        anyhow::bail!("stub provider cannot create a client")
    }
}

fn stub_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(StubProvider));
    registry
}

/// Build a syntactically valid OASF record wrapping a `ragent/agent/v1` payload.
fn oasf_record(name: &str, payload: serde_json::Value) -> OasfAgentRecord {
    OasfAgentRecord {
        name: name.to_string(),
        description: "A test agent".to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: Vec::new(),
        created_at: None,
        skills: Vec::new(),
        domains: Vec::new(),
        locators: Vec::new(),
        modules: vec![OasfModule {
            module_type: RAGENT_MODULE_TYPE.to_string(),
            payload,
        }],
    }
}

/// Write a project-local custom agent into `<root>/.ragent/agents/<file>`.
fn write_project_agent(root: &Path, file: &str, contents: &str) {
    let dir = root.join(".ragent").join("agents");
    std::fs::create_dir_all(&dir).expect("create agents dir");
    std::fs::write(dir.join(file), contents).expect("write agent file");
}

/// Write an AgentSkills pack: `<base>/<name>/SKILL.md`.
fn write_skill_pack(base: &Path, dir_name: &str, contents: &str) {
    let pack = base.join(dir_name);
    std::fs::create_dir_all(&pack).expect("create skill pack dir");
    std::fs::write(pack.join("SKILL.md"), contents).expect("write SKILL.md");
}

fn hook(trigger: HookTrigger, command: &str) -> HookConfig {
    HookConfig {
        trigger,
        command: command.to_string(),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }
}

// ---------------------------------------------------------------------------
// agent/mod.rs - resolution helpers
// ---------------------------------------------------------------------------

#[test]
fn builtin_agents_roster_is_cached_and_non_empty() {
    let first = builtin_agents();
    let second = builtin_agents();
    assert!(!first.is_empty());
    // The Arc roster is cached in a OnceLock, so both calls see the same slice.
    assert_eq!(first.len(), second.len());
    assert!(
        first.iter().any(|a| a.name == "general"),
        "the `general` builtin must be present"
    );
}

#[test]
fn resolve_agent_unknown_name_falls_back_to_custom_placeholder() {
    let config = ragent_agent::Config::default();
    let agent = resolve_agent("does-not-exist", &config).expect("fallback agent");
    assert_eq!(agent.name, "does-not-exist");
    assert_eq!(agent.description, "Custom agent: does-not-exist");
    assert_eq!(agent.mode, AgentMode::Primary);
}

#[test]
fn resolve_agent_applies_config_overrides() {
    let mut config = ragent_agent::Config::default();
    config.agent.insert(
        "general".to_string(),
        ragent_agent::AgentConfig {
            prompt: Some("overridden prompt".to_string()),
            model: Some("anthropic:claude-sonnet-4".to_string()),
            temperature: Some(0.42),
            top_p: Some(0.9),
            max_steps: Some(7),
            hidden: true,
            skills: vec!["skill-a".to_string()],
            options: std::iter::once(("k".to_string(), json!("v"))).collect(),
            ..Default::default()
        },
    );

    let agent = resolve_agent("general", &config).expect("overridden agent");
    assert_eq!(agent.prompt.as_deref(), Some("overridden prompt"));
    assert_eq!(
        agent
            .model
            .as_ref()
            .map(|m| (m.provider_id.as_str(), m.model_id.as_str())),
        Some(("anthropic", "claude-sonnet-4"))
    );
    assert_eq!(agent.temperature, Some(0.42));
    assert_eq!(agent.top_p, Some(0.9));
    assert_eq!(agent.max_steps, Some(7));
    assert!(agent.hidden);
    assert_eq!(agent.skills, vec!["skill-a".to_string()]);
    assert_eq!(agent.options.get("k"), Some(&json!("v")));
}

#[test]
fn resolve_agent_ignores_malformed_model_override() {
    let mut config = ragent_agent::Config::default();
    config.agent.insert(
        "general".to_string(),
        ragent_agent::AgentConfig {
            // No ':' delimiter - the string is silently ignored, model stays None.
            model: Some("model-without-provider".to_string()),
            ..Default::default()
        },
    );
    let agent = resolve_agent("general", &config).expect("agent");
    assert!(agent.model.is_none());
}

#[test]
fn resolve_default_model_uses_explicit_model_first() {
    let registry = stub_registry();
    let mut agent = AgentInfo::new("pinned", "pinned model");
    agent.model = Some(ModelRef {
        provider_id: "explicit".to_string(),
        model_id: "explicit-model".to_string(),
    });
    let resolved = resolve_default_model(&agent, &registry).expect("model");
    assert_eq!(resolved.provider_id, "explicit");
    assert_eq!(resolved.model_id, "explicit-model");
}

#[test]
fn resolve_default_model_scans_registry_and_empty_registry_is_none() {
    let registry = stub_registry();
    let agent = AgentInfo::new("unpinned", "no model");
    let resolved = resolve_default_model(&agent, &registry).expect("first model");
    assert_eq!(resolved.provider_id, "stub");
    assert_eq!(resolved.model_id, "stub-model");

    // A registry with no default models yields None.
    let empty = ProviderRegistry::new();
    assert!(resolve_default_model(&agent, &empty).is_none());
}

#[test]
fn resolve_agent_with_customs_falls_through_to_builtin() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let config = ragent_agent::Config::default();
    // No custom agents on disk -> builtin is returned.
    let agent = resolve_agent_with_customs("general", &config, tmp.path()).expect("builtin");
    assert_eq!(agent.name, "general");
}

#[test]
fn resolve_agent_with_customs_finds_project_local_agent() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project_agent(
        tmp.path(),
        "my-agent.json",
        &serde_json::to_string(&oasf_record(
            "my-agent",
            json!({"system_prompt": "You are my agent."}),
        ))
        .expect("serialise"),
    );

    let config = ragent_agent::Config::default();
    let agent =
        resolve_agent_with_customs("my-agent", &config, tmp.path()).expect("custom agent resolved");
    assert_eq!(agent.name, "my-agent");
    assert_eq!(agent.prompt.as_deref(), Some("You are my agent."));
}

#[test]
fn resolve_agent_with_customs_and_model_assigns_registry_model() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let config = ragent_agent::Config::default();
    let registry = stub_registry();
    let agent = resolve_agent_with_customs_and_model("general", &config, tmp.path(), &registry)
        .expect("agent with model");
    assert_eq!(
        agent.model.as_ref().map(|m| m.model_id.as_str()),
        Some("stub-model")
    );
}

#[test]
fn load_all_agents_includes_builtins_without_diagnostics() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let (agents, warnings) = load_all_agents(tmp.path());
    assert!(
        agents.iter().any(|a| a.name == "general"),
        "builtins must always be present"
    );
    assert!(
        warnings.is_empty(),
        "empty project must produce no diagnostics"
    );
}

#[test]
fn load_all_agents_renames_custom_agent_that_collides_with_builtin() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project_agent(
        tmp.path(),
        "general.json",
        &serde_json::to_string(&oasf_record(
            "general",
            json!({"system_prompt": "shadow general"}),
        ))
        .expect("serialise"),
    );

    let (agents, warnings) = load_all_agents(tmp.path());
    assert!(
        agents.iter().any(|a| a.name == "custom:general"),
        "colliding custom agent must be renamed, got {:?}",
        agents.iter().map(|a| a.name.clone()).collect::<Vec<_>>()
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("collides with a built-in")),
        "a collision diagnostic must be recorded: {warnings:?}"
    );
}

#[test]
fn default_thinking_config_for_levels_is_always_off() {
    assert_eq!(
        default_thinking_config_for_levels(&[ThinkingLevel::Low, ThinkingLevel::High]),
        ragent_types::ThinkingConfig::off()
    );
    assert_eq!(
        default_thinking_config_for_levels(&[]),
        ragent_types::ThinkingConfig::off()
    );
}

// ---------------------------------------------------------------------------
// agent/custom.rs - discovery, frontmatter, validation
// ---------------------------------------------------------------------------

#[test]
fn find_project_agents_dir_walks_up_to_nearest_ragent_dir() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let nested = tmp.path().join("a").join("b");
    std::fs::create_dir_all(&nested).expect("nested dirs");
    let agents_dir = tmp.path().join(".ragent").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    let found = find_project_agents_dir(&nested).expect("walk up finds .ragent/agents");
    assert_eq!(found, agents_dir);

    // A directory tree with no `.ragent/` yields None.
    let bare = tempfile::TempDir::new().expect("tempdir");
    assert!(find_project_agents_dir(bare.path()).is_none());
}

#[test]
fn load_custom_agents_parses_json_record_and_markdown_profile() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project_agent(
        tmp.path(),
        "json-agent.json",
        &serde_json::to_string(&oasf_record(
            "json-agent",
            json!({"system_prompt": "json body", "mode": "subagent", "max_steps": 5}),
        ))
        .expect("serialise"),
    );
    write_project_agent(
        tmp.path(),
        "md-agent.md",
        "---\n{\"name\": \"md-agent\", \"description\": \"from markdown\"}\n---\n\nYou are the markdown agent.\n",
    );

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    let by_name: HashMap<&str, &Arc<AgentInfo>> = agents
        .iter()
        .map(|d| (d.agent_info.name.as_str(), &d.agent_info))
        .collect();
    assert!(
        diagnostics.is_empty(),
        "both files should load cleanly: {diagnostics:?}"
    );
    assert_eq!(by_name["json-agent"].mode, AgentMode::Subagent);
    assert_eq!(by_name["json-agent"].max_steps, Some(5));
    assert_eq!(
        by_name["md-agent"].prompt.as_deref(),
        Some("You are the markdown agent.")
    );
    assert!(!by_name["md-agent"].model_pinned);
}

#[test]
fn load_custom_agents_accepts_yaml_frontmatter_profile() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project_agent(
        tmp.path(),
        "yaml-agent.md",
        "---\nname: yaml-agent\ndescription: yaml frontmatter profile\n---\n\nYAML body.\n",
    );
    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        agents.iter().any(|d| d.agent_info.name == "yaml-agent"),
        "YAML frontmatter profile must load"
    );
}

#[test]
fn load_custom_agents_reports_malformed_files_as_diagnostics() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project_agent(tmp.path(), "broken.json", "{ not valid json");
    write_project_agent(
        tmp.path(),
        "no-frontmatter.md",
        "just markdown, no delimiters",
    );
    // A file with a non-agent extension is skipped silently.
    write_project_agent(tmp.path(), "ignored.txt", "ignore me");

    let (agents, diagnostics) = load_custom_agents(tmp.path());
    assert!(agents.is_empty(), "no usable agents expected");
    assert_eq!(
        diagnostics.len(),
        2,
        "both malformed files reported: {diagnostics:?}"
    );
    assert!(diagnostics.iter().any(|d| d.contains("JSON parse error")));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("missing frontmatter"))
    );
}

#[test]
fn record_to_agent_info_rejects_invalid_records() {
    let path = Path::new("/tmp/x.json");

    // Empty name.
    let mut rec = oasf_record("", json!({"system_prompt": "hi"}));
    rec.name = String::new();
    assert!(record_to_agent_info(&rec, path).is_err());

    // Name with a space.
    let rec = oasf_record("has space", json!({"system_prompt": "hi"}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // Missing the ragent module.
    let mut rec = oasf_record("ok", json!({"system_prompt": "hi"}));
    rec.modules.clear();
    assert!(record_to_agent_info(&rec, path).is_err());

    // Empty system prompt.
    let rec = oasf_record("ok", json!({"system_prompt": "   "}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // Unknown mode.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "mode": "bogus"}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // Temperature out of range.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "temperature": 9.0}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // top_p out of range.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "top_p": 5.0}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // Malformed model string.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "model": "no-colon"}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // max_steps == 0.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "max_steps": 0}));
    assert!(record_to_agent_info(&rec, path).is_err());

    // Unknown permission action.
    let rec = oasf_record(
        "ok",
        json!({"system_prompt": "hi", "permissions": [{"permission": "read", "pattern": "**", "action": "nope"}]}),
    );
    assert!(record_to_agent_info(&rec, path).is_err());

    // Unknown memory scope.
    let rec = oasf_record("ok", json!({"system_prompt": "hi", "memory": "galaxy"}));
    assert!(record_to_agent_info(&rec, path).is_err());
}

#[test]
fn record_to_agent_info_parses_vendor_model_and_defaults() {
    let rec = oasf_record(
        "vendor-agent",
        json!({
            "system_prompt": "hi",
            "model": "anthropic:claude-sonnet-4@20250514",
            "hidden": true,
            "memory": "user",
            "options": {"temperature_override": 1}
        }),
    );
    let agent = record_to_agent_info(&rec, Path::new("/tmp/vendor.json")).expect("agent");
    // The `@vendor` suffix is stripped before provider:model parsing.
    assert_eq!(
        agent
            .model
            .as_ref()
            .map(|m| (m.provider_id.as_str(), m.model_id.as_str())),
        Some(("anthropic", "claude-sonnet-4"))
    );
    assert!(agent.model_pinned, "an explicit model pins the agent");
    assert!(agent.hidden);
    // Default max_steps when the payload omits it.
    assert_eq!(agent.max_steps, Some(1024));
    assert_eq!(agent.options.get("temperature_override"), Some(&json!(1)));
}

#[test]
fn record_to_agent_info_rejects_oversized_prompt() {
    let big = "x".repeat(32_769);
    let rec = oasf_record("big-agent", json!({ "system_prompt": big }));
    assert!(
        record_to_agent_info(&rec, Path::new("/tmp/big.json")).is_err(),
        "prompt over 32768 chars must be rejected"
    );
}

// ---------------------------------------------------------------------------
// skill/mod.rs + skill/invoke.rs
// ---------------------------------------------------------------------------

#[test]
fn skill_registry_loads_pack_and_exposes_counts() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_skill_pack(
        &tmp.path().join(".agents").join("skills"),
        "tiny",
        "---\nname: tiny\ndescription: tiny skill\n---\n\nBody for tiny.\n",
    );

    let registry = SkillRegistry::load(tmp.path(), &[]);
    let skill = registry.get("tiny").expect("pack discovered");
    assert_eq!(skill.description.as_deref(), Some("tiny skill"));
    assert_eq!(skill.scope, SkillScope::OpenSkillsProject);
    assert!(skill.is_user_invocable());
    assert!(skill.is_agent_invocable());
    assert!(
        registry
            .list_user_invocable()
            .iter()
            .any(|s| s.name == "tiny")
    );
    assert!(
        registry
            .list_agent_invocable()
            .iter()
            .any(|s| s.name == "tiny")
    );
    assert!(registry.list_all().iter().any(|s| s.name == "tiny"));
    assert!(!registry.is_empty());
    assert!(!registry.is_empty());
    assert!(registry.discovered_count() >= 1);
    assert!(registry.bundled_count() >= 1);
    assert!(registry.catalog().iter().any(|e| e.name == "tiny"));
}

#[test]
fn skill_register_prefers_higher_scope_on_name_clash() {
    let mut registry = SkillRegistry::new();
    let mut project = ragent_agent::skill::SkillInfo::new("dup", "project body");
    project.scope = SkillScope::Project;
    let mut global = ragent_agent::skill::SkillInfo::new("dup", "global body");
    global.scope = SkillScope::OpenSkillsGlobal;

    registry.register(project);
    registry.register(global.clone()); // lower scope must NOT replace
    assert_eq!(registry.get("dup").expect("dup").body, "project body");

    let mut higher = ragent_agent::skill::SkillInfo::new("dup", "higher body");
    higher.scope = SkillScope::Project;
    registry.register(higher); // equal scope replaces
    assert_eq!(registry.get("dup").expect("dup").body, "higher body");
}

#[tokio::test]
async fn skill_body_or_load_reads_from_disk_then_errors_without_source() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let pack = tmp.path().join(".agents").join("skills").join("disk");
    std::fs::create_dir_all(&pack).expect("pack dir");
    let skill_md = pack.join("SKILL.md");
    std::fs::write(
        &skill_md,
        "---\nname: disk\ndescription: disk skill\n---\n\nOn-disk body.\n",
    )
    .expect("write");

    let registry = SkillRegistry::load(tmp.path(), &[]);
    let skill = registry.get("disk").expect("discovered");
    assert!(skill.body.is_empty(), "discovered body is lazy");
    let body = skill.body_or_load().await.expect("load body");
    assert!(body.contains("On-disk body."));
    // Second call hits the cache.
    assert_eq!(skill.body_or_load().await.expect("cached body"), body);

    // A skill with neither an in-memory body nor a source path errors.
    let empty = ragent_agent::skill::SkillInfo::new("nodisk", "");
    assert!(empty.body_or_load().await.is_err());
}

#[tokio::test]
async fn skill_invoke_substitutes_arguments_end_to_end() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let pack = tmp.path().join(".agents").join("skills").join("greet");
    std::fs::create_dir_all(&pack).expect("pack dir");
    std::fs::write(
        pack.join("SKILL.md"),
        "---\nname: greet\ndescription: greet skill\n---\n\nHello $ARGUMENTS from $0!\n",
    )
    .expect("write");

    let registry = SkillRegistry::load(tmp.path(), &[]);
    let skill = registry.get("greet").expect("discovered");
    let invocation = invoke_skill(skill, "staging", "sess-1", tmp.path())
        .await
        .expect("invoke");
    assert_eq!(invocation.skill_name, "greet");
    assert!(invocation.content.contains("Hello staging from staging!"));
    assert!(!invocation.forked);
    assert!(format_skill_message(&invocation).contains("[Skill: /greet]"));
}

#[test]
fn skill_info_helpers_and_format_forked_result() {
    let mut skill = ragent_agent::skill::SkillInfo::new("s", "body");
    assert!(skill.is_user_invocable());
    assert!(skill.is_agent_invocable());
    assert!(!skill.is_forked());

    skill.disable_model_invocation = true;
    skill.user_invocable = false;
    assert!(!skill.is_agent_invocable());
    assert!(!skill.is_user_invocable());

    let result = ForkedSkillResult {
        skill_name: "fork-skill".to_string(),
        forked_session_id: "fork-1".to_string(),
        response: "found issues".to_string(),
    };
    let formatted = format_forked_result(&result);
    assert!(formatted.contains("fork-skill"));
    assert!(formatted.contains("found issues"));
}

#[test]
fn parse_model_ref_accepts_slash_colon_and_rejects_plain() {
    assert_eq!(
        parse_model_ref("anthropic/claude").map(|m| (m.provider_id, m.model_id)),
        Some(("anthropic".to_string(), "claude".to_string()))
    );
    assert_eq!(
        parse_model_ref("openai:gpt-4o").map(|m| (m.provider_id, m.model_id)),
        Some(("openai".to_string(), "gpt-4o".to_string()))
    );
    assert!(parse_model_ref("nodelimiter").is_none());
}

#[test]
fn resolve_inline_skill_agent_applies_tool_restriction_and_overrides() {
    let base = Arc::new(AgentInfo::new("general", "general"));

    // Explicit skill model wins over the active model.
    let agent = resolve_inline_skill_agent(
        &base,
        Some("anthropic/active"),
        Some("openai:skill-model"),
        &["Bash".to_string()],
    );
    assert_eq!(
        agent
            .model
            .as_ref()
            .map(|m| (m.provider_id.as_str(), m.model_id.as_str())),
        Some(("openai", "skill-model"))
    );
    assert_eq!(
        agent.allowed_tools.as_deref(),
        Some(&["Bash".to_string()][..])
    );

    // No model change and no tool restriction returns the same Arc.
    let same = resolve_inline_skill_agent(&base, None, None, &[]);
    assert!(Arc::ptr_eq(&same, &base));

    // Active model only (no restriction) clones but assigns a model.
    let active = resolve_inline_skill_agent(&base, Some("anthropic/active"), None, &[]);
    assert_eq!(
        active.model.as_ref().map(|m| m.model_id.as_str()),
        Some("active")
    );
}

// ---------------------------------------------------------------------------
// hooks/mod.rs - matchers, parsing, merge, and fire helpers
// ---------------------------------------------------------------------------

#[test]
fn hook_trigger_parse_covers_aliases_and_separators() {
    assert_eq!(
        HookTrigger::parse("OnSessionStart"),
        Some(HookTrigger::OnSessionStart)
    );
    assert_eq!(
        HookTrigger::parse("SessionStart"),
        Some(HookTrigger::OnSessionStart)
    );
    assert_eq!(
        HookTrigger::parse("session_end"),
        Some(HookTrigger::OnSessionEnd)
    );
    assert_eq!(HookTrigger::parse("Stop"), Some(HookTrigger::OnSessionEnd));
    assert_eq!(HookTrigger::parse("on_error"), Some(HookTrigger::OnError));
    assert_eq!(
        HookTrigger::parse("PermissionDenied"),
        Some(HookTrigger::OnPermissionDenied)
    );
    assert_eq!(
        HookTrigger::parse("UserPromptSubmit"),
        Some(HookTrigger::OnTurnStart)
    );
    assert_eq!(
        HookTrigger::parse("on-turn-end"),
        Some(HookTrigger::OnTurnEnd)
    );
    assert_eq!(
        HookTrigger::parse("PreCompact"),
        Some(HookTrigger::OnCompaction)
    );
    // Separators and case are ignored; unknown names return None.
    assert_eq!(
        HookTrigger::parse(" PRE_TOOL_USE "),
        Some(HookTrigger::PreToolUse)
    );
    assert_eq!(HookTrigger::parse("unknown-trigger"), None);

    // Display round-trips through parse.
    for trigger in [
        HookTrigger::OnSessionStart,
        HookTrigger::OnSessionEnd,
        HookTrigger::OnError,
        HookTrigger::OnPermissionDenied,
        HookTrigger::OnTurnStart,
        HookTrigger::OnTurnEnd,
        HookTrigger::OnCompaction,
        HookTrigger::PreToolUse,
        HookTrigger::PostToolUse,
    ] {
        assert_eq!(HookTrigger::parse(&trigger.to_string()), Some(trigger));
    }
}

#[test]
fn hook_config_matches_tool_variants() {
    let base = hook(HookTrigger::PreToolUse, "true");

    // No matcher -> matches every tool.
    assert!(base.matches_tool("anything", "{}"));

    // Empty matcher string -> matches every tool.
    let empty = HookConfig {
        matcher: Some(String::new()),
        ..base.clone()
    };
    assert!(empty.matches_tool("anything", "{}"));

    // Pipe-separated name list, case-insensitive.
    let names = HookConfig {
        matcher: Some("Edit|Write".to_string()),
        ..base.clone()
    };
    assert!(names.matches_tool("Edit", r#"{"file_path":"a.rs"}"#));
    assert!(names.matches_tool("write", r#"{"file_path":"a.rs"}"#));
    assert!(!names.matches_tool("Bash", r#"{"command":"ls"}"#));

    // Comma-separated list.
    let commas = HookConfig {
        matcher: Some("Read, Glob".to_string()),
        ..base.clone()
    };
    assert!(commas.matches_tool("Glob", "{}"));
    assert!(!commas.matches_tool("Edit", "{}"));

    // `Name(pattern)` Claude prefix form.
    let prefix = HookConfig {
        matcher: Some("Bash(git commit:*)".to_string()),
        ..base.clone()
    };
    assert!(prefix.matches_tool("Bash", r#"{"command":"git commit -m x"}"#));
    assert!(!prefix.matches_tool("Bash", r#"{"command":"git push"}"#));
    assert!(!prefix.matches_tool("Edit", r#"{"command":"git commit -m x"}"#));

    // `Name(glob)` glob guard.
    let glob = HookConfig {
        matcher: Some("Write(*.rs)".to_string()),
        ..base.clone()
    };
    assert!(glob.matches_tool("Write", r#"{"file_path":"src/a.rs"}"#));
    assert!(!glob.matches_tool("Write", r#"{"file_path":"a.py"}"#));

    // Wildcard guard matches everything for the named tool.
    let wildcard = HookConfig {
        matcher: Some("Edit(*)".to_string()),
        ..base.clone()
    };
    assert!(wildcard.matches_tool("Edit", r#"{"file_path":"anything"}"#));

    // An invalid glob degrades to "matches by bare tool name".
    let bad_glob = HookConfig {
        matcher: Some("Bash([)".to_string()),
        ..base
    };
    assert!(bad_glob.matches_tool("Bash", r#"{"command":"whatever"}"#));
}

#[test]
fn parse_hook_configs_keeps_valid_and_drops_invalid() {
    let raw = vec![
        json!({"trigger": "pre_tool_use", "command": "true"}),
        json!({"trigger": "post_tool_use", "command": "true", "timeout_secs": 5}),
        json!({"trigger": "not_a_trigger", "command": "true"}), // unknown trigger
        json!({"command": "missing-trigger"}),                  // missing field
    ];
    let parsed = parse_hook_configs(&raw);
    assert_eq!(parsed.len(), 2, "invalid entries dropped: {parsed:?}");
    assert_eq!(parsed[0].trigger, HookTrigger::PreToolUse);
    assert_eq!(parsed[0].timeout_secs, 30, "default timeout applied");
    assert_eq!(parsed[1].timeout_secs, 5);
}

#[test]
fn plugin_hook_configs_and_merge_drop_pre_tool_use() {
    let plugin_hooks = vec![
        PluginHook {
            plugin_id: "p".to_string(),
            plugin_root: PathBuf::from("/plugins/p"),
            trigger: "PreToolUse".to_string(),
            command: "./guard.sh".to_string(),
            timeout_secs: None,
            matcher: None,
        },
        PluginHook {
            plugin_id: "p".to_string(),
            plugin_root: PathBuf::from("/plugins/p"),
            trigger: "PostToolUse".to_string(),
            command: "./audit.sh".to_string(),
            timeout_secs: Some(11),
            matcher: Some("Edit|Write".to_string()),
        },
        PluginHook {
            plugin_id: "p".to_string(),
            plugin_root: PathBuf::from("/plugins/p"),
            trigger: "not-a-trigger".to_string(),
            command: "./never.sh".to_string(),
            timeout_secs: None,
            matcher: None,
        },
    ];

    // plugin_hook_configs maps recognised triggers and applies the default timeout.
    let mapped = plugin_hook_configs(&plugin_hooks);
    assert_eq!(mapped.len(), 2, "unrecognised trigger dropped");
    assert_eq!(mapped[0].trigger, HookTrigger::PreToolUse);
    assert_eq!(mapped[1].timeout_secs, 11);

    // merge keeps configured hooks first and drops plugin pre_tool_use.
    let configured = vec![hook(HookTrigger::OnTurnStart, "configured.sh")];
    let merged = merge_hook_configs(&configured, &plugin_hooks);
    assert_eq!(merged[0].command, "configured.sh");
    assert!(
        merged.iter().all(|h| h.trigger != HookTrigger::PreToolUse),
        "plugin pre_tool_use must be dropped: {merged:?}"
    );
    assert!(merged.iter().any(|h| h.trigger == HookTrigger::PostToolUse));
}

#[tokio::test]
async fn fire_hooks_and_turn_helpers_are_fire_and_forget() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    // A marker file lets the async hook prove it actually ran.
    let marker = tmp.path().join("turn-start.marker");

    let hooks = vec![
        hook(
            HookTrigger::OnTurnStart,
            &format!("printf started > {}", marker.display()),
        ),
        hook(HookTrigger::OnTurnEnd, "true"),
        hook(HookTrigger::OnCompaction, "true"),
        // Non-matching triggers are filtered out of the spawn.
        hook(HookTrigger::PreToolUse, "false"),
    ];

    fire_turn_start_hooks(&hooks, tmp.path(), 3);
    fire_turn_end_hooks(&hooks, tmp.path(), 3);
    fire_compaction_hooks(&hooks, tmp.path(), "auto", 100, 50);
    // Direct call with an unknown-matching trigger set is a no-op.
    fire_hooks(&hooks, HookTrigger::OnError, tmp.path(), &[]);

    // Give the fire-and-forget tasks a moment to run.
    for _ in 0..50 {
        if marker.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(marker.exists(), "turn-start hook should have run");

    // An empty hook list is a cheap no-op.
    fire_hooks(&[], HookTrigger::OnTurnStart, tmp.path(), &[]);
}

#[tokio::test]
async fn run_stop_hooks_collects_top_level_block_reason() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("block.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"decision\":\"block\",\"reason\":\"needs a test\"}'\nexit 0\n",
    )
    .expect("write");
    let configs = vec![hook(
        HookTrigger::OnSessionEnd,
        &format!("sh {}", script.display()),
    )];
    let guidance = run_stop_hooks(&configs, dir.path(), "sess-stop-extra").await;
    assert_eq!(guidance, vec!["needs a test".to_string()]);
}

#[tokio::test]
async fn run_stop_hooks_uses_stderr_when_blocking_without_reason() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("block-stderr.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s' 'stderr finding' >&2\nexit 2\n",
    )
    .expect("write");
    let configs = vec![hook(
        HookTrigger::OnSessionEnd,
        &format!("sh {}", script.display()),
    )];
    let guidance = run_stop_hooks(&configs, dir.path(), "sess-stop-extra").await;
    assert_eq!(guidance, vec!["stderr finding".to_string()]);
}

#[tokio::test]
async fn run_stop_hooks_ignores_non_session_end_and_quiet_hooks() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let configs = vec![
        hook(
            HookTrigger::OnTurnStart,
            "echo '{\"decision\":\"block\",\"reason\":\"x\"}'",
        ),
        hook(HookTrigger::OnSessionEnd, "cat >/dev/null; exit 0"),
    ];
    let guidance = run_stop_hooks(&configs, dir.path(), "sess-stop-extra").await;
    assert!(
        guidance.is_empty(),
        "no findings expected, got {guidance:?}"
    );
}
