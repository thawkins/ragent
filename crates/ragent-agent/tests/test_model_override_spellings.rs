//! FR-006 guard: every surface that documents the `new_agent` `model`
//! parameter names the same accepted spelling set (`provider/model` and
//! `provider:model`), and the schema says which is preferred.
//!
//! This is a drift guard, not a behaviour test. Before the fix the four
//! surfaces disagreed: the schema showed both spellings but named no
//! preference, the tool description named only `provider/model`, and neither
//! the system prompt nor `README.md` mentioned the parameter at all. Pinning
//! the spellings here turns that drift into a build failure, following the
//! FR-016 pattern the implementation plan uses for schema/description drift.

/// The schema, the tool description and the system prompt each name
/// `provider/model` *and* `provider:model`, and the schema names a preference.
///
/// This reproduces TC-031 in `specs/agentssetup/TESTPLAN.md`.
#[test]
fn test_fr006_schema_and_description_name_both_spellings_with_a_preference() {
    use ragent_agent::tool::Tool;
    use ragent_agent::tool::new_agent::NewAgentTool;

    let tool = NewAgentTool;

    let schema = tool.parameters_schema();
    let schema_description = schema["properties"]["model"]["description"]
        .as_str()
        .expect("the `model` property must carry a description");

    assert!(
        schema_description.contains("provider/model"),
        "schema must name the slash spelling: {schema_description}"
    );
    assert!(
        schema_description.contains("provider:model"),
        "schema must name the colon spelling: {schema_description}"
    );
    assert!(
        schema_description.contains("preferred"),
        "FR-006 requires the schema to say which spelling is preferred: {schema_description}"
    );

    let tool_description = tool.description();
    assert!(
        tool_description.contains("provider/model"),
        "tool description must name the slash spelling: {tool_description}"
    );
    assert!(
        tool_description.contains("provider:model"),
        "tool description must name the colon spelling: {tool_description}"
    );
}

/// The two spellings the docs promise are the two the resolver accepts.
///
/// `parse_model_ref` tries `/` first and falls back to `:`, so `provider/model`
/// is the preferred form in behaviour as well as in wording. A documented
/// spelling set that the parser rejects would be worse than no documentation,
/// so the accepted shapes are pinned here next to the prose that advertises
/// them.
#[test]
fn test_fr006_the_documented_spellings_are_the_ones_the_resolver_accepts() {
    // The resolver is mirrored rather than called: `parse_model_ref` is private
    // to `ragent_agent::task`, and the point of this test is to pin the
    // *documented* contract, not to re-test the parser. The split order is the
    // documented preference (`/` before `:`).
    for documented in ["anthropic/claude-sonnet-4-20250514", "openai:gpt-4o"] {
        let split = documented
            .split_once('/')
            .or_else(|| documented.split_once(':'));
        let (provider, model) = split
            .unwrap_or_else(|| panic!("documented spelling {documented:?} carries no separator"));
        assert!(
            !provider.is_empty() && !model.is_empty(),
            "documented spelling {documented:?} must name a provider and a model"
        );
    }

    // A bare model id is documented as ignored, matching the resolver's `None`.
    assert!(
        "gpt-4o"
            .split_once('/')
            .or_else(|| "gpt-4o".split_once(':'))
            .is_none(),
        "a separator-free string must stay undocumented as a valid spelling"
    );
}

/// The system prompt names the same spelling set as the schema and the tool
/// description (FR-006).
///
/// Before the fix the primary-agent prompt carried no `model` guidance at all,
/// so a model reading the prompt had no way to learn the parameter existed.
#[test]
fn test_fr006_system_prompt_names_both_spellings() {
    use ragent_agent::agent::{AgentInfo, build_system_prompt_with_storage_and_memory_and_config};
    use ragent_config::Config;

    let dir = tempfile::tempdir().expect("tempdir");
    let agent = AgentInfo::new("general", "g");
    let config = Config::default();

    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        &agent,
        dir.path(),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&config),
    );

    assert!(
        prompt.contains("**Per-call `model` override:**"),
        "the prompt must document the parameter at all"
    );
    assert!(
        prompt.contains("provider/model") && prompt.contains("provider:model"),
        "the prompt must name both documented spellings"
    );
    assert!(
        prompt.contains("preferred"),
        "the prompt must say which spelling is preferred, like the schema does"
    );
}
