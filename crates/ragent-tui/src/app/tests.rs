//! Inline tests for the TUI app module.
#![allow(clippy::redundant_pub_crate)]
#[cfg(test)]
mod app_tests {
    use crate::app::helpers::try_extract_research_code_block;
    use crate::app::{App, ModelPickerEntry};
    use ragent_agent::{
        agent, event::EventBus, permission::PermissionChecker, provider, session::SessionManager,
        session::processor::SessionProcessor, storage::Storage, telemetry::TelemetrySubsystem,
        tool,
    };
    use ragent_types::{ThinkingConfig, ThinkingLevel};
    use std::sync::Arc;

    pub(crate) fn test_app() -> App {
        let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
        let event_bus = Arc::new(EventBus::default());
        let provider_registry = Arc::new(provider::create_default_registry());
        let tool_registry = Arc::new(tool::create_default_registry());
        let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
        let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
        let session_processor = Arc::new(SessionProcessor {
            session_manager,
            provider_registry: provider_registry.clone(),
            tool_registry,
            permission_checker,
            event_bus: event_bus.clone(),
            agent_manager: std::sync::OnceLock::new(),
            bg_service: std::sync::OnceLock::new(),
            team_manager: std::sync::OnceLock::new(),
            // M8-T1: team-context cache (unused in tests, but required by the
            // struct literal). Mirrors the wiring in `src/main.rs`.
            team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
                std::collections::HashMap::new(),
            )),
            mcp_client: std::sync::OnceLock::new(),
            code_index: std::sync::OnceLock::new(),
            extraction_engine: std::sync::OnceLock::new(),
            stream_config: ragent_agent::StreamConfig::default(),
            active_spec: tokio::sync::RwLock::new(None),
            spec_manager: std::sync::OnceLock::new(),
            cached_tool_definitions: parking_lot::RwLock::new(None),
            cached_tool_names: parking_lot::RwLock::new(None),
            cached_tool_definition_bytes: parking_lot::RwLock::new(None),
            llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
            cached_config: parking_lot::Mutex::new(None),
            auto_approve: false,
            system_prompt_cache: parking_lot::RwLock::new(None),
            skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            telemetry: std::sync::Arc::new(TelemetrySubsystem::disabled()),
            activity_log: std::sync::OnceLock::new(),
        });
        let agent_info =
            agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

        App::new(
            event_bus,
            storage,
            provider_registry,
            session_processor,
            Arc::unwrap_or_clone(agent_info),
            false,
            std::path::PathBuf::new(),
        )
    }

    #[test]
    pub(crate) fn test_format_thinking_levels_handles_empty_and_full_lists() {
        assert_eq!(App::format_thinking_levels(&[]), "—");
        assert_eq!(
            App::format_thinking_levels(&[
                ThinkingLevel::Auto,
                ThinkingLevel::Off,
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High,
            ]),
            "Auto/Off/Low/Med/High"
        );
    }

    #[test]
    pub(crate) fn test_models_for_provider_sorts_case_insensitively() {
        let app = test_app();

        // Azure Resource defaults are file-based; use a known provider that still
        // returns models from user configuration. We only test that the picker
        // sorts entries case-insensitively when models are available.
        let models = app.models_for_provider("azure_resource");
        if models.is_empty() {
            // No azureresources.json in this environment; skip the sorting assertion.
            return;
        }

        let names: Vec<String> = models.iter().map(|m| m.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(
            names, sorted,
            "models_for_provider must return models sorted case-insensitively"
        );
    }

    #[test]
    pub(crate) fn test_huggingface_models_empty_without_key_or_discovery() {
        let app = test_app();

        // Ensure no HuggingFace credential is stored in the in-memory storage.
        let _ = app.storage.delete_provider_auth("huggingface");

        let models = app.models_for_provider("huggingface");
        // ragent no longer ships a hard-coded HuggingFace fallback catalog.
        // Without a token or successful discovery, the list is empty.
        assert!(
            models.is_empty(),
            "huggingface models should be empty when no token/discovery is available"
        );
    }
    #[test]
    pub(crate) fn test_picker_entries_sort_case_insensitively() {
        // Build picker entries manually and verify they are sorted
        // case-insensitively. Provider default catalogs are now empty, so we
        // use a synthetic set of entries for this unit test.
        let app = test_app();
        let input = vec![
            ragent_agent::provider::ModelInfo {
                id: "gpt-4o".to_string(),
                provider_id: "openai".to_string(),
                name: "GPT-4o".to_string(),
                cost: ragent_config::Cost {
                    input: 2.5,
                    output: 10.0,
                },
                capabilities: ragent_config::Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: true,
                    tool_use: true,
                    thinking_levels: vec![],
                },
                context_window: 128_000,
                max_output: Some(16_384),
                request_multiplier: None,
                thinking_config: None,
            },
            ragent_agent::provider::ModelInfo {
                id: "gpt-4o-mini".to_string(),
                provider_id: "openai".to_string(),
                name: "GPT-4o Mini".to_string(),
                cost: ragent_config::Cost {
                    input: 0.15,
                    output: 0.60,
                },
                capabilities: ragent_config::Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: true,
                    tool_use: true,
                    thinking_levels: vec![],
                },
                context_window: 128_000,
                max_output: Some(16_384),
                request_multiplier: None,
                thinking_config: None,
            },
        ];

        let models = app.picker_entries_from_models(input);
        assert!(
            !models.is_empty(),
            "picker entries should be returned for non-empty input"
        );

        let names: Vec<String> = models.iter().map(|m| m.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(
            names, sorted,
            "picker entries should be sorted case-insensitively"
        );
    }
    #[test]
    pub(crate) fn test_default_thinking_level_defaults_to_off_when_unconfigured() {
        let entry = ModelPickerEntry {
            id: "model".to_string(),
            name: "Model".to_string(),
            context_window: 128_000,
            max_output: Some(8_192),
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: true,
            vision: false,
            tool_use: true,
            thinking_levels: vec![ThinkingLevel::Auto, ThinkingLevel::Off, ThinkingLevel::High],
            thinking_config: None,
            cost_tier: "Free".to_string(),
            cost_multiplier: "0x".to_string(),
        };

        assert_eq!(
            App::default_thinking_level_for_entry(&entry),
            ThinkingLevel::Off
        );
    }

    #[test]
    pub(crate) fn test_default_thinking_level_falls_back_to_off_for_nonthinking_models() {
        let entry = ModelPickerEntry {
            id: "model".to_string(),
            name: "Model".to_string(),
            context_window: 128_000,
            max_output: Some(8_192),
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: false,
            vision: false,
            tool_use: true,
            thinking_levels: vec![],
            thinking_config: None,
            cost_tier: "Free".to_string(),
            cost_multiplier: "0x".to_string(),
        };

        assert_eq!(
            App::default_thinking_level_for_entry(&entry),
            ThinkingLevel::Off
        );
    }

    #[test]
    pub(crate) fn test_default_thinking_level_uses_explicit_entry_config() {
        let entry = ModelPickerEntry {
            id: "model".to_string(),
            name: "Model".to_string(),
            context_window: 128_000,
            max_output: Some(8_192),
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: true,
            vision: false,
            tool_use: true,
            thinking_levels: vec![
                ThinkingLevel::Auto,
                ThinkingLevel::Off,
                ThinkingLevel::Low,
                ThinkingLevel::High,
            ],
            thinking_config: Some(ThinkingConfig::new(ThinkingLevel::High)),
            cost_tier: "Free".to_string(),
            cost_multiplier: "0x".to_string(),
        };

        assert_eq!(
            App::default_thinking_level_for_entry(&entry),
            ThinkingLevel::High
        );
    }

    #[test]
    pub(crate) fn test_picker_entries_from_models_sorts_unsorted_input() {
        let app = test_app();
        let unsorted = vec![
            ragent_agent::provider::ModelInfo {
                id: "z".to_string(),
                provider_id: "test".to_string(),
                name: "Zebra".to_string(),
                cost: ragent_config::Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: ragent_config::Capabilities::default(),
                context_window: 128_000,
                max_output: None,
                request_multiplier: None,
                thinking_config: None,
            },
            ragent_agent::provider::ModelInfo {
                id: "a".to_string(),
                provider_id: "test".to_string(),
                name: "apple".to_string(),
                cost: ragent_config::Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: ragent_config::Capabilities::default(),
                context_window: 128_000,
                max_output: None,
                request_multiplier: None,
                thinking_config: None,
            },
            ragent_agent::provider::ModelInfo {
                id: "m".to_string(),
                provider_id: "test".to_string(),
                name: "Mango".to_string(),
                cost: ragent_config::Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: ragent_config::Capabilities::default(),
                context_window: 128_000,
                max_output: None,
                request_multiplier: None,
                thinking_config: None,
            },
        ];

        let entries = app.picker_entries_from_models(unsorted);
        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["apple", "Mango", "Zebra"]);
    }

    #[test]
    pub(crate) fn test_load_persisted_thinking_level_ignores_legacy_auto_default() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        storage
            .set_setting("thinking_level", "auto")
            .expect("persist thinking level");

        assert_eq!(App::load_persisted_thinking_level(&storage), None);
    }

    #[test]
    pub(crate) fn test_load_persisted_thinking_level_keeps_explicit_auto() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        storage
            .set_setting("thinking_level", "auto")
            .expect("persist thinking level");
        storage
            .set_setting("thinking_level_explicit", "1")
            .expect("persist explicit marker");

        assert_eq!(
            App::load_persisted_thinking_level(&storage),
            Some(ThinkingLevel::Auto)
        );
    }

    #[test]
    pub(crate) fn test_try_extract_research_code_block_preserves_preformatted_tables() {
        let text = "From: /research list\n\n```\nNAME                 TITLE                            STATUS      CREATED                  MODIFIED                 \n------------------------------------------------------------------------------------------------------------------------\nagentassign          agentassign                      draft       2026-06-20T05:13:16Z   2026-06-20T05:13:16Z   \n```";
        let extracted = try_extract_research_code_block(text).expect("research code block");
        assert!(extracted.contains("From: /research list"));
        assert!(extracted.contains("NAME"));
        assert!(extracted.contains("------------------------------------------------------------------------------------------------------------------------"));
        assert!(!extracted.contains("```"));
    }

    #[test]
    pub(crate) fn test_try_extract_research_code_block_returns_none_for_plain_research_response() {
        let text = "From: /research create\n\nGathering sources…";
        assert!(try_extract_research_code_block(text).is_none());
    }

    #[test]
    pub(crate) fn test_try_extract_research_code_block_handles_skills_output() {
        let text = "From: /skills\nRegistered Skills:\n\n```\n  Command   Scope  Access  Description\n  -------   -----  ------  -----------\n  /simplify         both    Reviews recently changed files\n  /debug            both    Troubleshoots current session\n```\n";
        let extracted = try_extract_research_code_block(text).expect("skills code block");
        assert!(extracted.contains("From: /skills"));
        assert!(extracted.contains("Registered Skills:"));
        assert!(extracted.contains("/simplify"));
        assert!(extracted.contains("/debug"));
        // Each skill must stay on its own line — the bug was the markdown
        // pipeline collapsing all rows into a single reflowed paragraph.
        assert!(extracted.contains("\n  /simplify"));
        assert!(extracted.contains("\n  /debug"));
        assert!(!extracted.contains("```"));
    }

    #[test]
    pub(crate) fn test_try_extract_research_code_block_handles_help_output() {
        let text = "From: /help\nAvailable commands:\n\n```\n  /about            Show info about ragent\n  /quit             Exit the TUI\n\nSkills:\n  /simplify         Reviews recently changed files\n  /debug            Troubleshoots current session\n```\n";
        let extracted = try_extract_research_code_block(text).expect("help code block");
        assert!(extracted.contains("From: /help"));
        assert!(extracted.contains("/about"));
        assert!(extracted.contains("/quit"));
        assert!(extracted.contains("/simplify"));
        assert!(extracted.contains("/debug"));
        // Each command/skill must stay on its own line.
        assert!(extracted.contains("\n  /simplify"));
        assert!(extracted.contains("\n  /debug"));
        assert!(!extracted.contains("```"));
    }

    #[test]
    pub(crate) fn test_try_extract_research_code_block_returns_none_for_non_slash_text() {
        let text = "Hello world\n\n```\nsome code\n```";
        assert!(try_extract_research_code_block(text).is_none());
    }

    #[test]
    pub(crate) fn test_render_markdown_to_ascii_bypasses_research_code_blocks() {
        let mut app = test_app();
        let text = "From: /research show\n\n```\nResearch item: x\n```";
        let rendered = app.render_markdown_to_ascii(text);
        assert!(rendered.contains("Research item: x"));
        assert!(!rendered.contains("From: /research show\nFrom:"));
    }

    #[test]
    pub(crate) fn test_render_markdown_to_ascii_preserves_skills_table_lines() {
        let mut app = test_app();
        let text = "From: /skills\nRegistered Skills:\n\n```\n  Command   Scope  Description\n  -------   -----  -----------\n  /simplify         Reviews recently changed files\n  /debug            Troubleshoots current session\n```\n";
        let rendered = app.render_markdown_to_ascii(text);
        // Each skill line must survive the markdown pipeline intact.
        assert!(rendered.contains("\n  /simplify"));
        assert!(rendered.contains("\n  /debug"));
        // The two skill lines must NOT have been merged into a single sentence.
        assert!(
            !rendered.contains("/simplify         Reviews recently changed files /debug"),
            "skills output should not collapse into a single paragraph; got:\n{rendered}",
        );
    }

    #[test]
    pub(crate) fn test_render_markdown_to_ascii_preserves_websearch_help_lines() {
        let mut app = test_app();
        let text = "From: /websearch\n\
            Web search engine diagnostics.\n\n\
            Usage:\n\n\
            • `/websearch show`\n\
              — list all engines with enabled / in-use / failed status\n\n\
            • `/websearch test`\n\
              — run a live diagnostic query on each configured engine and report counts\n\n\
            • `/websearch help`\n\
              — show this help";
        let rendered = app.render_markdown_to_ascii(text);
        // The markdown renderer collapses list items to single lines; the key
        // requirement is that each command appears on its own rendered line.
        assert!(
            rendered.contains("/websearch show"),
            "websearch show line missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("/websearch test"),
            "websearch test line missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("/websearch help"),
            "websearch help line missing; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("/websearch show /websearch test /websearch help"),
            "websearch help output collapsed into one line; got:\n{rendered}"
        );
    }

    #[test]
    pub(crate) fn test_render_markdown_to_ascii_preserves_startup_table_lines() {
        use ragent_agent::StartupTimings;
        let mut app = test_app();
        let mut timings = StartupTimings::new();
        timings.record("CLI parse", std::time::Duration::from_millis(5));
        timings.record("Config load", std::time::Duration::from_millis(12));
        let text = timings.format_report();
        let rendered = app.render_markdown_to_ascii(&text);
        // The table header and each stage row must survive on their own lines.
        assert!(
            rendered.contains("Stage"),
            "header missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("CLI parse"),
            "CLI parse row missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("Config load"),
            "Config load row missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("Total"),
            "Total row missing; got:\n{rendered}"
        );
        // Rows must NOT have been merged into a single paragraph.
        assert!(
            !rendered.contains("CLI parse Config load"),
            "startup output collapsed rows into one line; got:\n{rendered}",
        );
        assert!(
            !rendered.contains("```\n```"),
            "fence markers should be stripped by the code-block bypass; got:\n{rendered}",
        );
    }

    #[test]
    pub(crate) fn test_render_markdown_to_ascii_preserves_help_command_lines() {
        let mut app = test_app();
        let text = "From: /help\nAvailable commands:\n\n```\n  /about            Show info about ragent\n  /quit             Exit the TUI\n\nSkills:\n  /simplify         Reviews recently changed files\n  /debug            Troubleshoots current session\n```\n";
        let rendered = app.render_markdown_to_ascii(text);
        // Each command must survive the markdown pipeline intact.
        assert!(rendered.contains("\n  /about"));
        assert!(rendered.contains("\n  /quit"));
        assert!(rendered.contains("\n  /simplify"));
        assert!(rendered.contains("\n  /debug"));
        // Commands and skills must NOT have been merged into a single sentence.
        assert!(
            !rendered.contains("/about            Show info about ragent /quit"),
            "help output should not collapse into a single paragraph; got:\n{rendered}",
        );
    }

    #[test]
    fn test_tool_call_batch_applies_pending_args_when_start_dropped() {
        use ragent_agent::event::Event;
        use ragent_agent::message::MessagePart;
        use ragent_types::event::ToolCallBatchEntry;

        let mut app = test_app();
        app.handle_event(Event::SessionCreated {
            session_id: "s1".to_string(),
        });

        // Args arrive, but the matching ToolCallStart is dropped by the bus.
        app.handle_event(Event::ToolCallArgs {
            session_id: "s1".to_string(),
            call_id: "c1".to_string(),
            tool: "read".to_string(),
            args: r#"{"path":"src/main.rs"}"#.to_string(),
        });

        // Batch fallback creates the missing part.
        app.handle_event(Event::ToolCallBatch {
            session_id: "s1".to_string(),
            step: 1,
            calls: vec![ToolCallBatchEntry {
                call_id: "c1".to_string(),
                tool: "read".to_string(),
                args: "{}".to_string(),
                error: None,
                duration_ms: 10,
                content: "42 lines".to_string(),
                content_line_count: 42,
                metadata: None,
                success: true,
            }],
        });

        let part = app
            .messages
            .iter()
            .flat_map(|m| &m.parts)
            .find(|p| matches!(p, MessagePart::ToolCall { call_id, .. } if call_id == "c1"))
            .expect("batch should create the missing tool call part");
        if let MessagePart::ToolCall { state, .. } = part {
            assert_eq!(
                state.input.get("path").and_then(|v| v.as_str()),
                Some("src/main.rs"),
                "pending args should be applied so the read path is visible in the header"
            );
        } else {
            panic!("expected a ToolCall part");
        }
    }

    #[test]
    fn test_tool_call_batch_does_not_overwrite_existing_input() {
        use ragent_agent::event::Event;
        use ragent_agent::message::MessagePart;
        use ragent_types::event::ToolCallBatchEntry;

        let mut app = test_app();
        app.handle_event(Event::SessionCreated {
            session_id: "s1".to_string(),
        });

        // Normal flow: start then args.
        app.handle_event(Event::ToolCallStart {
            session_id: "s1".to_string(),
            call_id: "c1".to_string(),
            tool: "read".to_string(),
        });
        app.handle_event(Event::ToolCallArgs {
            session_id: "s1".to_string(),
            call_id: "c1".to_string(),
            tool: "read".to_string(),
            args: r#"{"path":"src/lib.rs"}"#.to_string(),
        });

        // Batch should keep the already-applied input.
        app.handle_event(Event::ToolCallBatch {
            session_id: "s1".to_string(),
            step: 1,
            calls: vec![ToolCallBatchEntry {
                call_id: "c1".to_string(),
                tool: "read".to_string(),
                args: "{}".to_string(),
                error: None,
                duration_ms: 10,
                content: "42 lines".to_string(),
                content_line_count: 42,
                metadata: None,
                success: true,
            }],
        });

        let part = app
            .messages
            .iter()
            .flat_map(|m| &m.parts)
            .find(|p| matches!(p, MessagePart::ToolCall { call_id, .. } if call_id == "c1"))
            .expect("tool call part exists");
        if let MessagePart::ToolCall { state, .. } = part {
            assert_eq!(
                state.input.get("path").and_then(|v| v.as_str()),
                Some("src/lib.rs"),
                "existing input should be preserved when batch is processed"
            );
        } else {
            panic!("expected a ToolCall part");
        }
    }

    #[test]
    fn test_nested_subagent_events_are_tracked() {
        use ragent_agent::event::Event;

        let mut app = test_app();
        app.handle_event(Event::SessionCreated {
            session_id: "primary".to_string(),
        });

        // Primary spawns a direct subagent.
        app.handle_event(Event::SubagentStart {
            session_id: "primary".to_string(),
            task_id: "explore-abc".to_string(),
            child_session_id: "child-1".to_string(),
            agent: "explore".to_string(),
            task: "inspect repo".to_string(),
            background: true,
        });
        assert_eq!(app.active_tasks.len(), 1);

        // The direct subagent spawns its own subagent.
        app.handle_event(Event::SubagentStart {
            session_id: "child-1".to_string(),
            task_id: "build-def".to_string(),
            child_session_id: "child-2".to_string(),
            agent: "build".to_string(),
            task: "fix bug".to_string(),
            background: true,
        });
        assert_eq!(app.active_tasks.len(), 2);

        // Lifecycle updates on the nested subagent are handled even after the
        // intermediate parent has completed and been removed.
        app.handle_event(Event::SubagentComplete {
            session_id: "primary".to_string(),
            task_id: "explore-abc".to_string(),
            child_session_id: "child-1".to_string(),
            summary: "done".to_string(),
            success: true,
            duration_ms: 100,
            finish_reason: "complete".to_string(),
        });
        assert_eq!(app.active_tasks.len(), 1);

        app.handle_event(Event::SubagentComplete {
            session_id: "child-1".to_string(),
            task_id: "build-def".to_string(),
            child_session_id: "child-2".to_string(),
            summary: "fixed".to_string(),
            success: true,
            duration_ms: 200,
            finish_reason: "complete".to_string(),
        });
        assert!(app.active_tasks.is_empty());
    }

    #[test]
    fn test_is_current_or_descendant_session_recognises_nested_tasks() {
        use ragent_agent::event::Event;

        let mut app = test_app();
        app.handle_event(Event::SessionCreated {
            session_id: "primary".to_string(),
        });

        assert!(app.is_current_session("primary"));
        assert!(!app.is_current_or_descendant_session("child-1"));

        app.handle_event(Event::SubagentStart {
            session_id: "primary".to_string(),
            task_id: "explore-abc".to_string(),
            child_session_id: "child-1".to_string(),
            agent: "explore".to_string(),
            task: "inspect".to_string(),
            background: true,
        });

        assert!(app.is_current_or_descendant_session("primary"));
        assert!(app.is_current_or_descendant_session("child-1"));
        assert!(!app.is_current_or_descendant_session("child-2"));
    }
}
