# Skill Test Quick Reference - Function List

## 1. LOADER.RS (25 tests)

### Parsing (11 tests)
- `test_parse_minimal_frontmatter` - Basic `---\n---\nBody`
- `test_parse_full_frontmatter` - All fields: description, model, agent, context, allowed-tools, name
- `test_parse_single_allowed_tool` - Single tool as scalar
- `test_parse_user_invocable_false` - user-invocable flag
- `test_parse_name_from_directory` - Dir name fallback
- `test_parse_name_override` - Name in frontmatter
- `test_parse_no_frontmatter` - Error: missing `---`
- `test_parse_unclosed_frontmatter` - Error: unclosed `---`
- `test_validate_name_too_long` - Error: > 64 chars
- `test_validate_name_invalid_chars` - Error: uppercase/special
- `test_parse_hooks_to_json` - PostToolUse hooks

### Metadata (5 tests)
- `test_skill_dir_set_correctly` - skill_dir & source_path
- `test_is_forked` - context: fork detection
- `test_is_not_forked` - Default non-forked
- `test_empty_body` - Empty body after `---`
- `test_multiline_body` - Multiline content

### Discovery (10 tests)
- `test_discover_skills_from_project_dir` - Find in `.ragent/skills/`
- `test_discover_skills_empty_working_dir` - Empty dir → []
- `test_discover_skills_nonexistent_dir` - Missing dir → []
- `test_discover_skills_monorepo_nested` - Nested directories
- `test_discover_skills_multiple` - Find 3+ skills
- `test_discover_skills_skips_non_directories` - Ignore files
- `test_discover_skills_with_extra_files` - scripts/, templates/
- `test_discover_skills_extra_dirs` - Extra dirs param
- `test_discover_skills_extra_dirs_overridden_by_project` - Priority
- `test_discover_skills_extra_dirs_nonexistent` - Handle missing extra

---

## 2. ARGS.RS (27 tests)

### Parse Args (8 tests)
- `test_parse_empty` - "" → []
- `test_parse_single_arg` - "staging" → ["staging"]
- `test_parse_multiple_args` - "a b c" → ["a", "b", "c"]
- `test_parse_extra_whitespace` - Multiple spaces normalized
- `test_parse_double_quoted` - "hello world" handling
- `test_parse_single_quoted` - 'hello world' handling
- `test_parse_mixed_quotes` - Mix of quote types
- `test_parse_only_whitespace` - "   " → []

### Substitute Args (19 tests)
- `test_substitute_arguments_all` - $ARGUMENTS
- `test_substitute_arguments_multi_word` - $ARGUMENTS multi-word
- `test_substitute_indexed_args` - $ARGUMENTS[0], [1]
- `test_substitute_indexed_out_of_bounds` - $ARGUMENTS[5] → ""
- `test_substitute_positional_shorthand` - $0, $1, $2
- `test_substitute_positional_out_of_bounds` - $3 → ""
- `test_substitute_session_id` - ${RAGENT_SESSION_ID}
- `test_substitute_skill_dir` - ${RAGENT_SKILL_DIR}
- `test_substitute_all_variable_types` - All together
- `test_substitute_no_placeholders` - Plain text
- `test_substitute_empty_args` - Empty input
- `test_substitute_quoted_args` - "hello world" production
- `test_substitute_dollar_not_variable` - "$50" handling
- `test_substitute_preserves_multiline` - Newlines preserved
- `test_substitute_double_digit_index` - $10, $11
- `test_substitute_skill_dir_with_pathbuf` - PathBuf conversion

---

## 3. CONTEXT.RS (17 tests)

### Pattern Finding (9 tests)
- `test_find_no_patterns` - No !` → []
- `test_find_single_pattern` - Single !`echo hello`
- `test_find_multiple_patterns` - Multiple patterns
- `test_find_pattern_with_pipes` - !`cat | grep | wc`
- `test_find_pattern_multiline_text` - Across lines
- `test_find_ignores_empty_command` - !`` ignored
- `test_find_ignores_unclosed` - !`no closing ignored
- `test_find_ignores_regular_backticks` - Regular code ignored
- `test_pattern_offsets` - Byte offset correctness

### Execution (8 async tests)
- `test_inject_no_patterns` - No patterns → unchanged
- `test_inject_echo_command` - !`echo hello` → hello
- `test_inject_multiple_commands` - Multiple injections
- `test_inject_failing_command` - Failure → error message
- `test_inject_preserves_surrounding_text` - Context preserved
- `test_inject_working_dir` - Correct execution dir
- `test_inject_command_with_pipes` - Complex pipes work
- `test_inject_pr_summary_pattern` - Real-world pattern
- `test_inject_nonexistent_command` - Unknown → error

---

## 4. INVOKE.RS (14 tests)

### Invocation (6 async tests)
- `test_invoke_simple_skill` - Basic args substitution
- `test_invoke_with_dynamic_context` - !`echo` execution
- `test_invoke_with_args_and_context` - Combined
- `test_invoke_preserves_fork_metadata` - Fork metadata
- `test_invoke_no_args_no_context` - Plain text
- `test_invoke_session_id_substitution` - Session var

### Message Formatting (4 tests)
- `test_format_skill_message` - [Skill: /name] format
- `test_format_skill_message_multiline` - Multiline content
- `test_format_forked_result` - [Forked Skill Result: /name]
- `test_format_forked_result_multiline` - Multiline response

### Metadata (4 tests)
- `test_forked_skill_result_struct` - ForkedSkillResult fields
- `test_invoke_forked_skill_sets_metadata` - Fork invocation
- `test_invocation_default_agent_fallback` - None → "general"
- `test_invocation_agent_specified` - Explicit agent

---

## 5. MOD.RS (19 tests)

### Registration (6 tests)
- `test_registry_register_and_get` - Basic register/get
- `test_registry_get_missing` - Nonexistent → None
- `test_registry_scope_priority_higher_wins` - Project > Personal
- `test_registry_scope_priority_lower_rejected` - Lower priority blocked
- `test_registry_scope_priority_equal_replaces` - Same scope: replace
- `test_registry_bundled_overridden_by_project` - Override bundled

### Listing (5 tests)
- `test_registry_list_user_invocable` - user_invocable=true
- `test_registry_list_agent_invocable` - model_invocation enabled
- `test_registry_list_all_sorted` - Alphabetical order
- `test_registry_multiple_skills` - Multiple access
- `test_registry_empty` - Empty behavior

### Loading (8 tests)
- `test_registry_load_empty_dir` - Only bundled (4 skills)
- `test_registry_load_project_skills` - Project + bundled
- `test_registry_load_skips_dirs_without_skill_md` - Skip invalid
- `test_registry_load_skips_malformed_skills` - Bad YAML skipped
- `test_registry_load_monorepo_nested` - Root + nested
- `test_registry_load_project_overrides_bundled` - Custom "simplify"
- `test_registry_load_with_extra_dirs` - Extra directories

---

## 6. BUNDLED.RS (11 tests)

### Bundle Verification (11 tests)
- `test_bundled_skills_count` - Exactly 4 skills
- `test_bundled_skill_names` - simplify, batch, debug, loop
- `test_bundled_skills_scope` - All Bundled scope
- `test_bundled_skills_have_descriptions` - All have desc
- `test_bundled_skills_user_invocable` - All user-callable
- `test_simplify_skill` - Specific: git diff, not disabled
- `test_batch_skill` - Specific: $ARGUMENTS, disabled
- `test_debug_skill` - Specific: debugging instructions
- `test_loop_skill` - Specific: interval handling
- `test_bundled_skills_have_nonempty_bodies` - All have body
- `test_bundled_skills_have_allowed_tools` - All allow bash

---

## 7. TEST_AGENT_SYSTEM.RS (6 tests)

### System Prompt Integration (6 tests)
- `test_build_system_prompt_with_skills` - Skills in prompt
- `test_build_system_prompt_no_skills_registry` - No registry → no section
- `test_build_system_prompt_empty_skills_registry` - Empty → no section
- `test_build_system_prompt_skills_excludes_agent_only_disabled` - Filtered
- `test_build_system_prompt_agent_specific_skills` - Agent filter
- `test_build_system_prompt_skills_order_after_agents_md` - Placement

---

## Test Counts by Category

| Category | Count | Async | File |
|----------|-------|-------|------|
| Parsing | 11 | 0 | loader.rs |
| Metadata | 5 | 0 | loader.rs |
| Discovery | 10 | 0 | loader.rs |
| Parse Args | 8 | 0 | args.rs |
| Substitute | 19 | 0 | args.rs |
| Find Patterns | 9 | 0 | context.rs |
| Inject | 8 | 8 | context.rs |
| Invocation | 6 | 6 | invoke.rs |
| Formatting | 4 | 0 | invoke.rs |
| Metadata | 4 | 0 | invoke.rs |
| Registration | 6 | 0 | mod.rs |
| Listing | 5 | 0 | mod.rs |
| Loading | 8 | 0 | mod.rs |
| Bundle | 11 | 0 | bundled.rs |
| System | 6 | 0 | test_agent_system.rs |
| **TOTAL** | **119** | **14** | |

---

## Execution Command

To run all skill tests:
```bash
cargo test --lib skill::  # Inline tests
cargo test --test test_agent_system test_build_system_prompt  # Integration tests
```

To run specific module:
```bash
cargo test --lib skill::loader::tests
cargo test --lib skill::args::tests
cargo test --lib skill::context::tests
cargo test --lib skill::invoke::tests
cargo test --lib skill::tests  # registry (mod.rs)
cargo test --lib skill::bundled::tests
```

