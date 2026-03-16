# Comprehensive Skill Test Audit - ragent

## Summary
Total test modules: 7
Total test functions: ~70+ tests
Test locations: 6 inline (src/skill/) + 1 external (tests/)

---

## 1. LOADER.RS - Parsing & Discovery Tests
**File:** `crates/ragent-core/src/skill/loader.rs`
**Purpose:** Frontmatter parsing and skill file discovery

### Frontmatter Parsing Tests (11 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_parse_minimal_frontmatter` | Minimal `---\n---\n` with body |
| `test_parse_full_frontmatter` | All fields: description, model, agent, context, allowed-tools, disable-model-invocation, argument-hint, name override |
| `test_parse_single_allowed_tool` | Single allowed-tool as scalar (not list) |
| `test_parse_user_invocable_false` | user-invocable: false flag |
| `test_parse_name_from_directory` | Skill name inferred from directory |
| `test_parse_name_override` | name field overrides directory name |
| `test_parse_no_frontmatter` | Error on content without `---` delimiter |
| `test_parse_unclosed_frontmatter` | Error on missing closing `---` |
| `test_validate_name_too_long` | Error when name > 64 chars |
| `test_validate_name_invalid_chars` | Error on uppercase/special chars in name |
| `test_parse_hooks_to_json` | PostToolUse hooks parsed to JSON |

### Metadata & Path Tests (4 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_skill_dir_set_correctly` | skill_dir and source_path set from file path |
| `test_is_forked` | context: fork flag detection |
| `test_is_not_forked` | Default (non-forked) detection |
| `test_empty_body` | Empty body after frontmatter |
| `test_multiline_body` | Multiline body with headers and paragraphs |

### Discovery Tests (10 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_discover_skills_from_project_dir` | Find skills in `.ragent/skills/` with all metadata |
| `test_discover_skills_empty_working_dir` | No skills in empty dir → empty list |
| `test_discover_skills_nonexistent_dir` | Nonexistent dir → empty list (graceful) |
| `test_discover_skills_monorepo_nested` | Nested `.ragent/skills/` in subdirectories |
| `test_discover_skills_multiple` | Multiple skills discovered together |
| `test_discover_skills_skips_non_directories` | Files in skills/ ignored (only dirs) |
| `test_discover_skills_with_extra_files` | Skills dir with scripts/, templates/ subdirs |
| `test_discover_skills_extra_dirs` | Extra skill directories via extra_dirs param |
| `test_discover_skills_extra_dirs_overridden_by_project` | Project skills take priority over extra_dirs |
| `test_discover_skills_extra_dirs_nonexistent` | Nonexistent extra_dirs handled gracefully |

### Edge Cases NOT Tested
- **Parsing edge cases:** 
  - Unicode/emoji in descriptions or bodies
  - Nested YAML structures (model override formats: `provider/model` vs `provider:model`)
  - Empty frontmatter fields (`description: ""`)
  - Very long descriptions or bodies
  - Special characters in descriptions (quotes, backslashes)
  - YAML lists vs scalar for single values (test_parse_single_allowed_tool only tests allowed-tools)
  
- **Discovery edge cases:**
  - Skills with symlinks
  - Read-only permission on skill files
  - Very deep nesting (more than monorepo_nested tests)
  - Skills directory with 1000+ subdirectories (performance)
  - Case sensitivity on case-insensitive filesystems

---

## 2. ARGS.RS - Argument Substitution Tests
**File:** `crates/ragent-core/src/skill/args.rs`
**Purpose:** Variable substitution for skill invocation

### Parse Args Tests (8 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_parse_empty` | Empty string → empty vec |
| `test_parse_single_arg` | "staging" → ["staging"] |
| `test_parse_multiple_args` | "a b c" → ["a", "b", "c"] |
| `test_parse_extra_whitespace` | Multiple spaces/tabs normalized |
| `test_parse_double_quoted` | "\"hello world\" foo" → ["hello world", "foo"] |
| `test_parse_single_quoted` | "'hello world' foo" → ["hello world", "foo"] |
| `test_parse_mixed_quotes` | Mix of single & double quotes |
| `test_parse_only_whitespace` | "   " → empty vec |

### Substitution Tests (11 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_substitute_arguments_all` | $ARGUMENTS → "staging" |
| `test_substitute_arguments_multi_word` | $ARGUMENTS → "staging prod" (multi-word) |
| `test_substitute_indexed_args` | $ARGUMENTS[0], $ARGUMENTS[1] |
| `test_substitute_indexed_out_of_bounds` | $ARGUMENTS[5] when only 1 arg → empty string |
| `test_substitute_positional_shorthand` | $0, $1, $2 substitution |
| `test_substitute_positional_out_of_bounds` | $3 when only 2 args → empty string |
| `test_substitute_session_id` | ${RAGENT_SESSION_ID} |
| `test_substitute_skill_dir` | ${RAGENT_SKILL_DIR} → full path |
| `test_substitute_all_variable_types` | Combination: $ARGUMENTS, $0, $ARGUMENTS[1], ${RAGENT_SESSION_ID}, ${RAGENT_SKILL_DIR} |
| `test_substitute_no_placeholders` | Plain text unchanged |
| `test_substitute_empty_args` | Empty args string with $0/$ARGUMENTS |
| `test_substitute_quoted_args` | "hello world" production → substituted individually |
| `test_substitute_dollar_not_variable` | "$50" parsed as $5 (out of bounds) + "0" |
| `test_substitute_preserves_multiline` | Newlines and structure preserved |
| `test_substitute_double_digit_index` | $10, $11 with 12 args |
| `test_substitute_skill_dir_with_pathbuf` | PathBuf conversion works |

### Edge Cases NOT Tested
- **Argument parsing:**
  - Escaped quotes within quoted args ("\"hello")
  - Newlines within quoted args
  - Unicode/emoji in arguments
  - Very long argument strings (>10MB)
  - Tab characters mixed with spaces
  
- **Substitution:**
  - Consecutive variables ($0$1)
  - Variables in variable names (${$ARGUMENTS[0]})
  - Escaped variables (\$ARGUMENTS)
  - Case sensitivity (${ragent_session_id} vs ${RAGENT_SESSION_ID})
  - Malformed indices ($ARGUMENTS[abc], $ARGUMENTS[-1], $ARGUMENTS[foo])
  - Very large indices ($ARGUMENTS[999999])
  - Empty variable name placeholders ($)

---

## 3. CONTEXT.RS - Dynamic Context Injection Tests
**File:** `crates/ragent-core/src/skill/context.rs`
**Purpose:** Execute `` !`command` `` placeholders

### Pattern Finding Tests (9 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_find_no_patterns` | No !` patterns → empty list |
| `test_find_single_pattern` | Single !`echo hello` found |
| `test_find_multiple_patterns` | Multiple patterns found in order |
| `test_find_pattern_with_pipes` | !`cat file \| grep \| wc` with pipes |
| `test_find_pattern_multiline_text` | Multiple patterns across lines |
| `test_find_ignores_empty_command` | !`` (empty) ignored |
| `test_find_ignores_unclosed` | !`no closing backtick ignored |
| `test_find_ignores_regular_backticks` | Regular code `backticks` not matched |
| `test_find_ignores_exclamation_without_backtick` | "This is great!" not matched |
| `test_pattern_offsets` | Correct byte offsets for replacement |

### Execution Tests (8 tests - async)

| Test Name | Coverage |
|-----------|----------|
| `test_inject_no_patterns` | No patterns → body unchanged |
| `test_inject_echo_command` | !`echo hello` → "hello" |
| `test_inject_multiple_commands` | Multiple injections work |
| `test_inject_failing_command` | Failed command → [command failed: stderr] |
| `test_inject_preserves_surrounding_text` | Before/after text untouched |
| `test_inject_working_dir` | Commands execute in correct working dir |
| `test_inject_command_with_pipes` | Complex command with pipes works |
| `test_inject_pr_summary_pattern` | Real-world PR context pattern |
| `test_inject_nonexistent_command` | Unknown command → error placeholder |

### Edge Cases NOT Tested
- **Pattern detection:**
  - Triple backticks in code fences (test mentions but doesn't verify complex nesting)
  - Backticks at line boundaries
  - Very long commands (>100KB)
  - Unicode in commands
  - Nested !` sequences
  
- **Execution:**
  - Commands that produce massive output (>1GB)
  - Commands with timeout exactly at 30s
  - Signal handling (SIGINT, SIGTERM)
  - Environment variable inheritance
  - Concurrent command execution (current implementation is sequential)
  - Commands with newlines in output handling
  - Non-UTF8 output from commands
  - Commands with file descriptors/handles left open

---

## 4. INVOKE.RS - Skill Invocation Tests
**File:** `crates/ragent-core/src/skill/invoke.rs`
**Purpose:** Combined args substitution + context injection for skill execution

### Invocation Tests (6 async tests)

| Test Name | Coverage |
|-----------|----------|
| `test_invoke_simple_skill` | Basic invocation: args substituted |
| `test_invoke_with_dynamic_context` | !`echo` commands executed |
| `test_invoke_with_args_and_context` | Both args and context in same skill |
| `test_invoke_preserves_fork_metadata` | Fork context, agent, model, allowed_tools preserved |
| `test_invoke_no_args_no_context` | Plain text skill unchanged |
| `test_invoke_session_id_substitution` | ${RAGENT_SESSION_ID} in skill |

### Message Formatting Tests (4 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_format_skill_message` | [Skill: /name]\n\ncontent format |
| `test_format_skill_message_multiline` | Multiline content formatted correctly |
| `test_format_forked_result` | [Forked Skill Result: /name]\n\nresponse format |
| `test_format_forked_result_multiline` | Multiline response formatted correctly |

### Metadata Tests (4 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_forked_skill_result_struct` | ForkedSkillResult fields set correctly |
| `test_invoke_forked_skill_sets_metadata` | Forked invocation with context/agent/model |
| `test_invocation_default_agent_fallback` | fork_agent: None → "general" |
| `test_invocation_agent_specified` | fork_agent explicitly set |

### Edge Cases NOT Tested
- **Invocation:**
  - Circular skill references (A calls B, B calls A)
  - Skills that call themselves recursively
  - Very large skill bodies (>100MB)
  - Skills with only context injection (no args)
  - Mixed quoted/unquoted args in complex skill
  - Model override format variations (provider/model vs provider:model)
  
- **Forked execution:**
  - Forked skill cancellation/timeout
  - Model override with invalid provider/model
  - Agent that doesn't exist
  - Nested forked skills (fork within fork)
  - Tool restrictions enforcement details
  - Session manager errors

---

## 5. MOD.RS - Registry Tests
**File:** `crates/ragent-core/src/skill/mod.rs`
**Purpose:** SkillRegistry management and scope priority

### Registration Tests (6 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_registry_register_and_get` | Basic register/get |
| `test_registry_get_missing` | get("nonexistent") → None |
| `test_registry_scope_priority_higher_wins` | Project > Personal (higher priority wins) |
| `test_registry_scope_priority_lower_rejected` | Lower scope rejected when higher exists |
| `test_registry_scope_priority_equal_replaces` | Same scope: newer replaces older |
| `test_registry_bundled_overridden_by_project` | Project overrides Bundled skill |

### Listing Tests (5 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_registry_list_user_invocable` | user_invocable=true skills only |
| `test_registry_list_agent_invocable` | disable_model_invocation=false skills only |
| `test_registry_list_all_sorted` | All skills sorted alphabetically |
| `test_registry_multiple_skills` | Multiple skills accessible |
| `test_registry_empty` | Empty registry behavior |

### Loading Tests (7 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_registry_load_empty_dir` | Load from empty dir → 4 bundled skills |
| `test_registry_load_project_skills` | Load project .ragent/skills + bundled |
| `test_registry_load_skips_dirs_without_skill_md` | Directories without SKILL.md skipped |
| `test_registry_load_skips_malformed_skills` | Bad YAML/parsing → skipped, good ones loaded |
| `test_registry_load_monorepo_nested` | Root + nested package skills |
| `test_registry_load_project_overrides_bundled` | Project "simplify" overrides bundled |
| `test_registry_load_with_extra_dirs` | Extra directories loaded as Personal scope |

### Edge Cases NOT Tested
- **Scope priority:**
  - More than 3 scope levels simultaneously
  - Skills with same name, different scopes, in different registries
  - Scope changes after registration
  
- **Loading:**
  - Concurrent load() calls
  - Very large monorepo (1000+ skills)
  - Corrupted SKILL.md files (missing permissions, truncated)
  - Symlink loops in skill directories
  - Skills with duplicate names in same scope

---

## 6. BUNDLED.RS - Bundled Skills Tests
**File:** `crates/ragent-core/src/skill/bundled.rs`
**Purpose:** Verify 4 bundled skills present and correct

### Bundle Tests (11 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_bundled_skills_count` | Exactly 4 bundled skills |
| `test_bundled_skill_names` | simplify, batch, debug, loop present |
| `test_bundled_skills_scope` | All have scope: Bundled |
| `test_bundled_skills_have_descriptions` | All have description field |
| `test_bundled_skills_user_invocable` | All user_invocable=true |
| `test_simplify_skill` | Correct description, contains "git diff", not disabled, no arg_hint |
| `test_batch_skill` | Correct description, $ARGUMENTS, disable_model_invocation=true, arg_hint="<instruction>" |
| `test_debug_skill` | Correct description, $ARGUMENTS, arg_hint="[description]" |
| `test_loop_skill` | Correct description, $ARGUMENTS, disable_model_invocation=true, arg_hint="[interval] <prompt>" |
| `test_bundled_skills_have_nonempty_bodies` | All have body content |
| `test_bundled_skills_have_allowed_tools` | All allow bash + others |

### Edge Cases NOT Tested
- **Bundled skill content:**
  - Instructions actually valid/understandable
  - Tool allowlists match documented capabilities
  - Disable_model_invocation enforced correctly
  - Model/agent/context fields (none tested)
  - Performance with very large bodies

---

## 7. TEST_AGENT_SYSTEM.RS - Integration Tests
**File:** `crates/ragent-core/tests/test_agent_system.rs`
**Purpose:** System prompt building with skills

### Skill Integration Tests (6 tests)

| Test Name | Coverage |
|-----------|----------|
| `test_build_system_prompt_with_skills` | Skills section in system prompt with descriptions |
| `test_build_system_prompt_no_skills_registry` | No registry → no skills section |
| `test_build_system_prompt_empty_skills_registry` | Empty registry → no skills section |
| `test_build_system_prompt_skills_excludes_agent_only_disabled` | disable_model_invocation=true skipped |
| `test_build_system_prompt_agent_specific_skills` | Agent with skills filter includes only specified |
| `test_build_system_prompt_skills_order_after_agents_md` | Skills section placement correct |

### Edge Cases NOT Tested
- **System prompt integration:**
  - Very large skill list impact on token count
  - Skills with multiline descriptions
  - Skill descriptions with special markdown
  - Skills appearing in multiple agents' allowlists
  - Model-specific skill handling

---

## COMPREHENSIVE GAP ANALYSIS

### Critical Gaps (HIGH PRIORITY)

1. **Frontmatter Parsing:**
   - No tests for nested YAML structures (allowed-tools as object?)
   - No unicode/emoji in fields
   - No validation of model format variations
   - No tests for malformed hooks structure

2. **Argument Parsing:**
   - No escaped quote handling
   - No very long argument strings
   - No test of consecutive variables
   - No malformed index validation

3. **Context Injection:**
   - No tests for commands with large output (>100MB)
   - No timeout edge case tests
   - No concurrent execution handling
   - No UTF-8 validation for command output

4. **Forked Execution:**
   - NO TESTS for actual forked session execution
   - No cancellation/timeout tests
   - No invalid agent resolution tests
   - No tool restriction enforcement tests

### Important Gaps (MEDIUM PRIORITY)

5. **Discovery:**
   - No symlink handling
   - No permission error handling
   - No case sensitivity testing
   - No performance testing with 1000+ skills

6. **Registry:**
   - No concurrent registration tests
   - No scope edge cases (3+ levels)
   - No large monorepo testing
   - No error recovery tests

7. **Bundled Skills:**
   - No verification that skill instructions are actually valid
   - No end-to-end execution tests
   - No model/agent/context field usage
   - No allowed_tools enforcement tests

### Minor Gaps (LOW PRIORITY)

8. **General Coverage:**
   - No tests for skill bodies with code fences and !`` sequences
   - No tests for skill names at boundary (64 chars exactly)
   - No stress tests with 100+ skills
   - No memory/leak tests for long-running operations

---

## Test Execution Summary

**Total Inline Tests:** ~58
- loader.rs: 25 tests
- args.rs: 27 tests  
- context.rs: 17 tests
- invoke.rs: 14 tests
- mod.rs: 19 tests
- bundled.rs: 11 tests

**Total External Tests:** ~6
- test_agent_system.rs: 6 skill-related tests

**Async Tests:** ~14 (context.rs + invoke.rs)

**Estimated Coverage:** ~75% of happy path, ~30% of edge cases
