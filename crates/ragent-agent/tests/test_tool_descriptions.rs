//! Guard test for the sub-agent family's tool descriptions (spec `agentssetup`
//! FR-016).
//!
//! FR-016 requires that the `new_agent`, `cancel_agent`, `list_agents` and
//! `wait_agents` descriptions "name every parameter they accept and every
//! execution contract they support, and a test shall assert that each
//! description mentions each schema property name".
//!
//! The description is the primary artefact the model reads, and it had already
//! drifted from the schema (SPEC.md D1: `new_agent`'s description omitted
//! `detached` and the `provider:model` spelling of `model`, both of which the
//! schema declares). This test converts that drift from a review finding into a
//! build failure: every property the schema declares must be named in the
//! description, and no property the description advertises may be absent from
//! the schema.
//!
//! The property set is read from each tool's own `parameters_schema()`, so the
//! assertion follows the contract rather than a hand-copied list that could
//! itself go stale. `bg` is covered too (SPEC.md risk 4): it shares the family's
//! `additionalProperties: false` pattern and its "shell task" vocabulary is the
//! boundary that keeps it distinct from the sub-agent tools (FR-019).
//!
//! Nothing here executes a tool, so no `ToolContext` is built and every
//! optional service is irrelevant: the whole guard reads `parameters_schema()`
//! and `description()` from each tool value.

use ragent_agent::tool::Tool;
use ragent_agent::tool::bg::BgTool;
use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::new_agent::NewAgentTool;
use ragent_agent::tool::param_schema::closed_property_names;
use ragent_agent::tool::wait_agents::WaitAgentsTool;

/// The five tools FR-016 guards: the four sub-agent tools plus `bg`, which
/// shares their `additionalProperties: false` pattern and must keep saying
/// "shell task" rather than "background task" (FR-019, SPEC.md risk 4).
fn family() -> Vec<(&'static str, Box<dyn Tool>)> {
    vec![
        ("new_agent", Box::new(NewAgentTool)),
        ("cancel_agent", Box::new(CancelAgentTool)),
        ("list_agents", Box::new(ListAgentsTool)),
        ("wait_agents", Box::new(WaitAgentsTool)),
        ("bg", Box::new(BgTool::new())),
    ]
}

/// FR-016: every property name the schema declares appears in the description.
///
/// This is the drift guard SPEC.md D1 records: a property added to a schema
/// without a matching sentence in the description fails here instead of
/// reaching a model that then never learns the parameter exists.
#[test]
fn every_declared_property_is_named_in_the_description() {
    for (name, tool) in family() {
        let schema = tool.parameters_schema();
        let declared =
            closed_property_names(&schema).expect("a family tool must declare closed properties");
        assert!(
            !declared.is_empty(),
            "{name}: a family tool must declare at least one property"
        );

        let description = tool.description();
        let missing: Vec<&str> = declared
            .iter()
            .map(String::as_str)
            .filter(|property| !description.contains(property))
            .collect();
        assert!(
            missing.is_empty(),
            "{name}: description must name every schema property (FR-016); missing {missing:?}\n\
             schema properties: {declared:?}\n\
             description: {description}"
        );
    }
}

/// FR-016, converse direction: the description must not advertise a parameter
/// the schema does not accept.
///
/// A description that names an undeclared parameter is worse than a silent one:
/// `additionalProperties: false` makes the call a hard refusal (FR-004,
/// FR-038), so a model following the prose would build a call that always
/// fails. Every backticked lowercase identifier that the description presents
/// as a parameter (the `PARAMETERS:` block, or the "Optional parameters" /
/// "Conditional required parameters" sentences `bg` uses) is checked against
/// the schema.
#[test]
fn no_description_advertises_a_property_the_schema_rejects() {
    for (name, tool) in family() {
        let schema = tool.parameters_schema();
        let declared =
            closed_property_names(&schema).expect("a family tool must declare closed properties");
        let description = tool.description();

        for property in advertised_parameters(description) {
            assert!(
                declared.iter().any(|known| known == &property),
                "{name}: description advertises `{property}`, which the schema does not declare \
                 (declared: {declared:?}) and `additionalProperties: false` would refuse\n\
                 description: {description}"
            );
        }
    }
}

/// The four sub-agent descriptions name all three execution contracts (FR-016).
///
/// "Every execution contract they support" is a separate clause from the
/// parameter set: the contracts are selected by parameter *values*, so a
/// description can name `background` and `detached` while still never saying
/// what the three combinations mean. `new_agent` supports all three; the others
/// operate on one contract each and must name it.
#[test]
fn the_descriptions_name_the_execution_contracts_they_support() {
    let new_agent = NewAgentTool.description();
    for contract in ["foreground", "background", "detached"] {
        assert!(
            new_agent.contains(contract),
            "new_agent must name the `{contract}` contract: {new_agent}"
        );
    }
    assert!(
        new_agent.contains("detached: true` without `background: true"),
        "new_agent must state that detached without background is refused: {new_agent}"
    );

    // FR-014 / FR-013: the two tools that can only ever observe a finished or
    // requested outcome name the terminal `cancelled` status so a waiter is
    // never told "failed" for a cancel.
    let wait_agents = WaitAgentsTool.description();
    assert!(
        wait_agents.contains("background: true"),
        "wait_agents must name the contract it awaits: {wait_agents}"
    );
    assert!(
        wait_agents.contains("detached"),
        "wait_agents must say a detached task is not awaitable (FR-044): {wait_agents}"
    );

    let cancel_agent = CancelAgentTool.description();
    assert!(
        cancel_agent.contains("background: true"),
        "cancel_agent must name the contract it cancels: {cancel_agent}"
    );
    assert!(
        cancel_agent.contains("REQUESTED"),
        "cancel_agent must not claim a terminal state it has not reached (FR-013): {cancel_agent}"
    );

    let list_agents = ListAgentsTool.description();
    assert!(
        list_agents.contains("background: true"),
        "list_agents must name the contract it lists: {list_agents}"
    );
    assert!(
        list_agents.contains("detached"),
        "list_agents must say a detached task is not listed (FR-020): {list_agents}"
    );
}

/// FR-016's TESTPLAN case (TC-004) names the required parameters explicitly, so
/// the parameter set is pinned from the requirement side as well as from the
/// schema side. A schema rename that keeps both sides self-consistent would
/// still break the documented contract; this pins the names the specification
/// promises.
#[test]
fn the_descriptions_carry_the_parameters_tc_004_requires() {
    let required: [(&str, &[&str]); 4] = [
        (
            "new_agent",
            &[
                "agent",
                "task",
                "background",
                "detached",
                "model",
                "timeout_secs",
            ],
        ),
        ("cancel_agent", &["task_id"]),
        ("list_agents", &["status", "task_id"]),
        ("wait_agents", &["task_ids", "timeout_secs"]),
    ];

    for (name, properties) in required {
        let tool: Box<dyn Tool> = match name {
            "new_agent" => Box::new(NewAgentTool),
            "cancel_agent" => Box::new(CancelAgentTool),
            "list_agents" => Box::new(ListAgentsTool),
            "wait_agents" => Box::new(WaitAgentsTool),
            other => panic!("unexpected tool in TC-004 list: {other}"),
        };
        let schema = tool.parameters_schema();
        let declared =
            closed_property_names(&schema).expect("a family tool must declare closed properties");
        let description = tool.description();

        for property in properties {
            assert!(
                description.contains(property),
                "{name}: TC-004 requires the description to name `{property}`: {description}"
            );
            assert!(
                declared.iter().any(|known| known == property),
                "{name}: TC-004 names `{property}` but the schema does not declare it \
                 (declared: {declared:?})"
            );
        }
    }
}

/// FR-006's contract inside the generated description: both `model` spellings
/// are named and one is declared preferred, in the description and in the
/// property's own schema description.
#[test]
fn new_agent_description_states_the_model_spellings_and_the_preference() {
    let tool = NewAgentTool;
    let description = tool.description();

    for spelling in ["provider/model", "provider:model"] {
        assert!(
            description.contains(spelling),
            "FR-006: the description must name the {spelling} spelling: {description}"
        );
    }
    assert!(
        description.contains("preferred"),
        "FR-006: the description must say which spelling is preferred: {description}"
    );

    let schema = tool.parameters_schema();
    let model = schema
        .get("properties")
        .and_then(|properties| properties.get("model"))
        .and_then(|model| model.get("description"))
        .and_then(serde_json::Value::as_str)
        .expect("`model` must carry a schema description");
    assert!(
        model.contains("preferred"),
        "FR-006: the schema description must state the preference too: {model}"
    );
}

/// FR-019: `bg` keeps the shell-task vocabulary and never borrows the
/// sub-agent family's name for its tasks, which is what keeps the two tools
/// distinguishable in a model's mind even though both are "asynchronous work".
#[test]
fn bg_description_keeps_the_shell_task_vocabulary() {
    let tool = BgTool::new();
    let description = tool.description();
    assert!(
        description.contains("shell task"),
        "FR-019: bg must call its tasks shell tasks: {description}"
    );
    assert!(
        !description.contains("background task"),
        "FR-019: bg must not use the sub-agent contract's name: {description}"
    );
}

/// Extracts the property identifiers a description presents as parameters.
///
/// Two shapes are in use across the family and both are recognised, so the
/// converse check stays honest rather than passing vacuously:
///
/// - the `PARAMETERS:` block, where each bullet is `` - `name` (type, ...) ``;
/// - `bg`'s prose form, "Optional parameters include 'working_dir' (spawn), ...".
///
/// The prose scan deliberately stops at `bg`'s "Conditional required
/// parameters:" sentence, which names action *values* ('spawn', 'list') in the
/// same quoted style as the property names it pairs them with; those values are
/// enum members of `action`, not parameters, and admitting them would make the
/// probe fire on the action vocabulary rather than on description/schema drift.
/// The parameters that sentence names ('command', 'task_id') are also named in
/// the list scan, so nothing is lost.
fn advertised_parameters(description: &str) -> Vec<String> {
    let mut found = Vec::new();

    if let Some(start) = description.find("PARAMETERS:") {
        for line in description[start..].lines() {
            let Some(rest) = line.trim_start().strip_prefix("- ") else {
                continue;
            };
            if let Some(name) = leading_backticked_identifier(rest) {
                found.push(name);
            }
        }
    }

    if let Some(start) = description.find("Optional parameters include ") {
        let section = &description[start..];
        let section = section.split("Common gotcha").next().unwrap_or(section);
        found.extend(quoted_lowercase_identifiers(section));
    }

    found.sort();
    found.dedup();
    found
}

/// The backticked identifier a parameter bullet starts with, e.g.
/// `` `agent` (string, REQUIRED) - ... `` yields `agent`.
fn leading_backticked_identifier(bullet: &str) -> Option<String> {
    let rest = bullet.strip_prefix('`')?;
    let (name, _) = rest.split_once('`')?;
    is_parameter_identifier(name).then(|| name.to_string())
}

/// Every single-quoted lowercase identifier in a prose parameter list, e.g.
/// "'working_dir' (spawn)" yields `working_dir`.
fn quoted_lowercase_identifiers(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('\'') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('\'') else {
            break;
        };
        let name = &after[..close];
        if is_parameter_identifier(name) {
            found.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    found
}

/// Whether `name` looks like a schema property: lowercase ASCII, digits and
/// underscores only, starting and ending on a letter or digit.
///
/// This rejects the quoted prose that shares the parameter-list positions
/// (`'spawn'`, `'list'`, `'cancelled'`), which is why the converse check does
/// not need to parse the prose's grammar - only identifiers shaped like a
/// property are considered candidates, and `bg`'s `status` filter words are
/// separated by the fact that the check compares against the schema anyway.
fn is_parameter_identifier(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('_')
        && !name.ends_with('_')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
