# Documentation Inventory

This document tracks all public API functions and types that lack adequate documentation.

## Summary

- **Total items requiring attention**: 112
- **Functions/types with NO documentation**: 4
- **Functions with NO examples**: 109
- **Files affected**: 25

## RAGENT-CORE (87 items)

### crates/ragent-core/src/agent/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 77 | pub fn | ✅ | ❌ |
| `create_builtin_agents` | 104 | pub fn | ✅ | ❌ |
| `resolve_agent` | 325 | pub fn | ✅ | ❌ |
| `build_system_prompt` | 368 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/config/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `load` | 229 | pub fn | ✅ | ❌ |
| `merge` | 273 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/event/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 210 | pub fn | ✅ | ❌ |
| `subscribe` | 216 | pub fn | ✅ | ❌ |
| `publish` | 228 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/id.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 22 | pub fn | ✅ | ❌ |
| `as_str` | 26 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/mcp/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 84 | pub fn | ✅ | ❌ |
| `connect` | 107 | pub async fn | ✅ | ❌ |
| `list_tools` | 238 | pub fn | ✅ | ❌ |
| `list_tools_for_server` | 254 | pub fn | ✅ | ❌ |
| `refresh_tools` | 273 | pub async fn | ✅ | ❌ |
| `refresh_tools_for_server` | 331 | pub async fn | ✅ | ❌ |
| `call_tool` | 382 | pub async fn | ✅ | ❌ |
| `call_tool_by_name` | 441 | pub async fn | ✅ | ❌ |
| `servers` | 473 | pub fn | ✅ | ❌ |
| `disconnect` | 488 | pub async fn | ✅ | ❌ |
| `disconnect_all` | 516 | pub async fn | ✅ | ❌ |

### crates/ragent-core/src/message/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `ToolCallState` | 53 | pub struct | ❌ | ✅ |
| `new` | 99 | pub fn | ✅ | ❌ |
| `user_text` | 112 | pub fn | ✅ | ❌ |
| `text_content` | 121 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/permission/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `Permission` | 32 | pub enum | ❌ | ✅ |
| `new` | 139 | pub fn | ✅ | ❌ |
| `check` | 152 | pub fn | ✅ | ❌ |
| `record_always` | 182 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/provider/copilot.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 97 | pub fn | ✅ | ❌ |
| `with_url` | 104 | pub fn | ✅ | ❌ |
| `find_copilot_token` | 558 | pub fn | ✅ | ❌ |
| `find_gh_cli_token` | 591 | pub fn | ✅ | ❌ |
| `is_pat_token` | 613 | pub fn | ✅ | ❌ |
| `resolve_copilot_github_token` | 621 | pub fn | ✅ | ❌ |
| `start_copilot_device_flow` | 697 | pub async fn | ✅ | ❌ |
| `poll_copilot_device_flow` | 729 | pub async fn | ✅ | ❌ |
| `resolve_copilot_auth` | 862 | pub async fn | ✅ | ❌ |
| `check_copilot_health` | 948 | pub async fn | ✅ | ❌ |
| `discover_copilot_api_base` | 1049 | pub async fn | ✅ | ❌ |
| `list_copilot_models` | 1079 | pub async fn | ✅ | ❌ |

### crates/ragent-core/src/provider/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 81 | pub fn | ✅ | ❌ |
| `register` | 88 | pub fn | ✅ | ❌ |
| `get` | 93 | pub fn | ✅ | ❌ |
| `list` | 98 | pub fn | ✅ | ❌ |
| `resolve_model` | 112 | pub fn | ✅ | ❌ |
| `create_default_registry` | 126 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/provider/ollama.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 42 | pub fn | ✅ | ❌ |
| `with_url` | 51 | pub fn | ✅ | ❌ |
| `list_ollama_models` | 527 | pub async fn | ✅ | ❌ |

### crates/ragent-core/src/sanitize.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `redact_secrets` | 11 | pub fn | ❌ | ❌ |

### crates/ragent-core/src/session/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 53 | pub fn | ✅ | ❌ |
| `storage` | 58 | pub fn | ✅ | ❌ |
| `create_session` | 68 | pub fn | ✅ | ❌ |
| `get_session` | 99 | pub fn | ✅ | ❌ |
| `list_sessions` | 109 | pub fn | ✅ | ❌ |
| `archive_session` | 120 | pub fn | ✅ | ❌ |
| `get_messages` | 133 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/session/processor.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `process_message` | 49 | pub async fn | ✅ | ❌ |

### crates/ragent-core/src/snapshot/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `take_snapshot` | 31 | pub fn | ✅ | ❌ |
| `restore_snapshot` | 57 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/storage/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `obfuscate_key` | 30 | pub fn | ✅ | ❌ |
| `deobfuscate_key` | 43 | pub fn | ✅ | ❌ |
| `open` | 78 | pub fn | ✅ | ❌ |
| `open_in_memory` | 97 | pub fn | ✅ | ❌ |
| `create_session` | 175 | pub fn | ✅ | ❌ |
| `get_session` | 190 | pub fn | ✅ | ❌ |
| `list_sessions` | 220 | pub fn | ✅ | ❌ |
| `update_session` | 251 | pub fn | ✅ | ❌ |
| `archive_session` | 266 | pub fn | ✅ | ❌ |
| `create_message` | 283 | pub fn | ✅ | ❌ |
| `get_messages` | 315 | pub fn | ✅ | ❌ |
| `update_message` | 363 | pub fn | ✅ | ❌ |
| `set_provider_auth` | 381 | pub fn | ✅ | ❌ |
| `delete_provider_auth` | 398 | pub fn | ✅ | ❌ |
| `get_provider_auth` | 412 | pub fn | ✅ | ❌ |
| `set_setting` | 428 | pub fn | ✅ | ❌ |
| `delete_setting` | 443 | pub fn | ✅ | ❌ |
| `get_setting` | 454 | pub fn | ✅ | ❌ |

### crates/ragent-core/src/tool/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `ToolContext` | 51 | pub struct | ❌ | ✅ |
| `new` | 92 | pub fn | ✅ | ❌ |
| `register` | 99 | pub fn | ✅ | ❌ |
| `get` | 104 | pub fn | ✅ | ❌ |
| `list` | 109 | pub fn | ✅ | ❌ |
| `definitions` | 116 | pub fn | ✅ | ❌ |
| `create_default_registry` | 140 | pub fn | ✅ | ❌ |

## RAGENT-SERVER (3 items)

### crates/ragent-server/src/routes/mod.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `start_server` | 53 | pub async fn | ✅ | ❌ |
| `router` | 63 | pub fn | ✅ | ❌ |

### crates/ragent-server/src/sse.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `event_to_sse` | 10 | pub fn | ✅ | ❌ |

## RAGENT-TUI (22 items)

### crates/ragent-tui/src/app.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `normalized` | 246 | pub fn | ✅ | ❌ |
| `new` | 349 | pub fn | ✅ | ❌ |
| `detect_provider` | 433 | pub fn | ✅ | ❌ |
| `refresh_provider` | 526 | pub fn | ✅ | ❌ |
| `load_session` | 540 | pub fn | ✅ | ❌ |
| `models_for_provider` | 594 | pub fn | ✅ | ❌ |
| `provider_model_label` | 646 | pub fn | ✅ | ❌ |
| `check_provider_health` | 660 | pub fn | ✅ | ❌ |
| `provider_health_status` | 708 | pub fn | ✅ | ❌ |
| `update_slash_menu` | 720 | pub fn | ✅ | ❌ |
| `execute_slash_command` | 752 | pub fn | ✅ | ❌ |
| `handle_mouse_event` | 945 | pub fn | ✅ | ❌ |
| `extract_text_from_lines` | 1103 | pub fn | ✅ | ❌ |
| `handle_key_event` | 1182 | pub fn | ✅ | ❌ |
| `handle_event` | 1330 | pub fn | ✅ | ❌ |
| `push_log` | 1568 | pub fn | ✅ | ❌ |

### crates/ragent-tui/src/input.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `handle_key` | 42 | pub fn | ✅ | ❌ |

### crates/ragent-tui/src/layout.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `render` | 24 | pub fn | ✅ | ❌ |

### crates/ragent-tui/src/lib.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `run_tui` | 45 | pub async fn | ✅ | ❌ |

### crates/ragent-tui/src/tips.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `random_tip` | 22 | pub fn | ✅ | ❌ |

### crates/ragent-tui/src/widgets/message_widget.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 23 | pub fn | ✅ | ❌ |

### crates/ragent-tui/src/widgets/permission_dialog.rs

| Function/Type | Line | Type | Doc | Example |
|---|---|---|---|---|
| `new` | 24 | pub fn | ✅ | ❌ |

