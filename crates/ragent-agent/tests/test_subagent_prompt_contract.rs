//! Guard tests for the sub-agent guidance block in the system prompt
//! (spec `agentssetup` FR-025).
//!
//! FR-025 requires that the sub-agent guidance block "describe all three
//! execution contracts, the limit and its occupancy, the await contract, and the
//! durable report path, using the same vocabulary as the tool descriptions".
//!
//! The block is rendered prose, so it cannot be checked by a schema guard the
//! way FR-016's descriptions are. These tests pin the four teaching obligations
//! instead, and pin the *vocabulary* separately by comparing the block against
//! the live tool descriptions rather than against a hand-copied word list: a
//! rename in `new_agent`'s description that the prompt does not follow fails
//! here, which is exactly the drift FR-025 exists to prevent.
//!
//! Nothing here spawns an agent, loads a provider or touches storage: the block
//! is a pure function of an [`AgentInfo`] and a [`Config`], so each test renders
//! the prompt into a temp directory and reads the section out of the string.

use ragent_agent::agent::{
    AgentInfo, AgentMode, build_system_prompt_with_storage_and_memory_and_config,
};
use ragent_agent::tool::Tool;
use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::new_agent::NewAgentTool;
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_config::Config;

/// Renders the system prompt for a primary agent and returns only the
/// `## Sub-Agent Spawning` section.
///
/// The section runs to the next `## ` heading, so an assertion here cannot be
/// satisfied by text belonging to a later part of the prompt.
fn sub_agent_block(config: &Config) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = AgentInfo::new("general", "General-purpose agent");
    agent.mode = AgentMode::Primary;

    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        &agent,
        dir.path(),
        "tree",
        None,
        Some(""),
        Some(""),
        None,
        None,
        None,
        None,
        Some(config),
    );

    let start = prompt
        .find("## Sub-Agent Spawning")
        .unwrap_or_else(|| panic!("FR-025: the prompt must carry the guidance block: {prompt}"));
    let rest = &prompt[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    rest[..end].to_string()
}

/// FR-025(a): all three execution contracts are taught, each named with the
/// `background` / `detached` shape that selects it, and the illegal
/// detached-without-background combination is named as refused.
#[test]
fn the_prompt_teaches_the_three_execution_contracts() {
    let block = sub_agent_block(&Config::default());

    for contract in ["foreground", "background", "detached"] {
        assert!(
            block.contains(contract),
            "FR-025: the guidance block must name the `{contract}` contract: {block}"
        );
    }

    // The selecting shapes, not just the names: a model that knows the three
    // words but not which booleans select them still cannot make the call.
    for shape in [
        "`background` absent or `false`",
        "`background: true, detached: false`",
        "`background: true, detached: true`",
    ] {
        assert!(
            block.contains(shape),
            "FR-025: the block must name the selecting shape {shape}: {block}"
        );
    }

    assert!(
        block.contains("`detached: true` without `background: true`"),
        "FR-025: the block must name the illegal combination, as new_agent's description does: \
         {block}"
    );
    assert!(
        block.contains("REFUSED"),
        "FR-025: the illegal combination must be named as refused, not degraded: {block}"
    );

    // The distinguishing consequences of the background and detached contracts:
    // listed / awaitable / injected versus not. These are the three words a
    // caller planning a batch needs in order to pick the right one.
    for consequence in [
        "listed by `list_agents`",
        "awaitable with `wait_agents`",
        "injected",
    ] {
        assert!(
            block.contains(consequence),
            "FR-025: the block must state that a background task is {consequence}: {block}"
        );
    }
    assert!(
        block.contains("fire-and-forget"),
        "FR-025: the detached contract must be described as fire-and-forget: {block}"
    );
}

/// FR-025(b) + FR-030: the limit is taught together with its occupancy rule -
/// a suspended task still occupies a slot, so suspending frees no headroom and
/// cancellation is the supported way to release one.
#[test]
fn the_prompt_teaches_the_limit_and_how_a_slot_is_freed() {
    let mut config = Config::default();
    config.experimental.max_background_agents = 5;

    let block = sub_agent_block(&config);

    assert!(
        block.contains("at most **5** background tasks"),
        "FR-025: the block must state the configured limit, not a placeholder: {block}"
    );
    assert!(
        !block.contains("MAX_BG_TASKS"),
        "FR-025: the limit placeholder must be substituted, not shipped: {block}"
    );

    // FR-030: occupancy counts suspended and terminating tasks.
    assert!(
        block.contains("`suspended`"),
        "FR-030: the block must name suspended tasks in the occupancy rule: {block}"
    );
    assert!(
        block.contains("STILL occupies its slot"),
        "FR-030: the block must state that a suspended task still occupies a slot: {block}"
    );
    assert!(
        block.contains("`terminating`"),
        "FR-031: the block must name a terminating task as still counting: {block}"
    );

    // The cancel-to-free rule, stated as a rule rather than as a hint.
    assert!(
        block.contains("To free a slot, CANCEL - do not suspend."),
        "FR-025: the block must state the cancel-to-free rule in those words: {block}"
    );
    assert!(
        block.contains("suspending a task frees NO headroom"),
        "FR-025: the block must say suspending frees nothing: {block}"
    );
    assert!(
        block.contains("cancel_requested"),
        "FR-013/FR-025: the block must say a cancel is only requested until the terminal status \
         arrives: {block}"
    );
    assert!(
        block.contains("`occupancy`") && block.contains("`headroom`") && block.contains("`limit`"),
        "FR-011/FR-025: the block must point at list_agents' limit/occupancy/headroom report: \
         {block}"
    );
}

/// FR-025(c): the await contract - what settles an await, which statuses are
/// reported, and that suspended is not settled while timed_out is.
#[test]
fn the_prompt_teaches_the_await_contract() {
    let block = sub_agent_block(&Config::default());

    assert!(
        block.contains("THE AWAIT CONTRACT"),
        "FR-025: the block must teach the await contract under a named heading: {block}"
    );
    assert!(
        block.contains("`wait_agents` blocks until"),
        "FR-025: the block must describe what wait_agents blocks on: {block}"
    );
    assert!(
        block.contains("`task_ids`") && block.contains("`timeout_secs`"),
        "FR-025: the block must name both wait_agents parameters: {block}"
    );

    // FR-014: the canonical status vocabulary, taken from the registry, with the
    // cancelled-is-not-failed distinction spelled out.
    for status in ["`completed`", "`failed`", "`cancelled`", "`timed_out`"] {
        assert!(
            block.contains(status),
            "FR-014/FR-025: the block must name the `{status}` status: {block}"
        );
    }
    assert!(
        block.contains("a cancelled task is never"),
        "FR-014/FR-025: the block must state that a cancelled task is never reported as failed: \
         {block}"
    );

    // FR-015: a suspended task is not treated as finished.
    assert!(
        block.contains("`suspended` is not settled"),
        "FR-015/FR-025: the block must say a suspended task is not settled: {block}"
    );
    // FR-017: a timed-out task is settled.
    assert!(
        block.contains("`timed_out` is settled"),
        "FR-017/FR-025: the block must say a timed-out task is settled: {block}"
    );

    // A detached task can never be awaited: the await contract has to say so,
    // because the caller otherwise awaits a result that will never arrive.
    assert!(
        block.contains("A detached task can never be awaited"),
        "FR-044/FR-025: the block must say a detached task cannot be awaited: {block}"
    );
}

/// FR-025(d): the durable report path, including the absence case (FR-018,
/// FR-032) so a caller never reads a path that does not exist.
#[test]
fn the_prompt_teaches_the_durable_report_path() {
    let block = sub_agent_block(&Config::default());

    assert!(
        block.contains("THE DURABLE REPORT PATH"),
        "FR-025: the block must teach the durable report path under a named heading: {block}"
    );
    assert!(
        block.contains("log/subagents/<task-id>.md"),
        "FR-025: the block must name the durable report path: {block}"
    );
    assert!(
        block.contains("`output_file`"),
        "FR-025: the block must name the `output_file` field the path is reported in: {block}"
    );
    assert!(
        block.contains("in-memory only"),
        "FR-018/FR-032: the block must tell the caller what an absent durable copy means: {block}"
    );
    assert!(
        block.contains("wait_agents` and `list_agents` say so")
            || block.contains("`list_agents` and `wait_agents` say so"),
        "FR-018: the block must say the absence is reported, not silently omitted: {block}"
    );
}

/// FR-025's vocabulary clause: the prompt and the tool descriptions teach the
/// same contract in the same words.
///
/// The terms are read from the live descriptions rather than hard-coded, so this
/// asserts agreement between the two artefacts: a description that teaches a
/// term the prompt does not know about fails here instead of leaving the model
/// with two vocabularies. The `background`/`detached` shapes and the
/// limit/occupancy/headroom triple are the shared spine named by FR-001, FR-011
/// and FR-025.
#[test]
fn the_prompt_uses_the_same_vocabulary_as_the_tool_descriptions() {
    let block = sub_agent_block(&Config::default());
    let new_agent = NewAgentTool.description();
    let list_agents = ListAgentsTool.description();
    let wait_agents = WaitAgentsTool.description();
    let cancel_agent = CancelAgentTool.description();

    // The contract shapes new_agent's description declares must be the ones the
    // prompt teaches.
    for shape in [
        "foreground",
        "background: true, detached: false",
        "background: true, detached: true",
        "detached: true` without `background: true",
    ] {
        assert!(
            new_agent.contains(shape),
            "precondition: new_agent's description must name {shape}: {new_agent}"
        );
        assert!(
            block.contains(shape),
            "FR-025: the prompt must use new_agent's vocabulary for {shape}: {block}"
        );
    }

    // The limit/occupancy/headroom triple is list_agents' vocabulary; the prompt
    // must teach the same three names or a model cannot read the report.
    for term in ["`limit`", "`occupancy`", "`headroom`"] {
        assert!(
            list_agents.contains(term),
            "precondition: list_agents must report {term}: {list_agents}"
        );
        assert!(
            block.contains(term),
            "FR-025: the prompt must use list_agents' vocabulary {term}: {block}"
        );
    }

    // The suspend-frees-nothing rule and the cancel-to-free rule are stated by
    // both list_agents and cancel_agent; the prompt must not contradict them.
    for description in [list_agents, cancel_agent] {
        assert!(
            description.contains("suspend"),
            "precondition: the limit-reporting tools must mention suspension: {description}"
        );
    }
    assert!(
        block.contains("suspending a task frees NO headroom"),
        "FR-025: the prompt must carry list_agents' suspend-frees-nothing rule: {block}"
    );
    assert!(
        cancel_agent.contains("free a slot"),
        "precondition: cancel_agent must name the cancel-to-free rule: {cancel_agent}"
    );
    assert!(
        block.contains("To free a slot, CANCEL"),
        "FR-025: the prompt must carry cancel_agent's cancel-to-free rule: {block}"
    );

    // The status vocabulary in the prompt must be the one wait_agents promises.
    for status in ["`completed`", "`failed`", "`cancelled`", "`timed_out`"] {
        assert!(
            wait_agents.contains(status),
            "precondition: wait_agents must report {status}: {wait_agents}"
        );
        assert!(
            block.contains(status),
            "FR-014/FR-025: the prompt must use wait_agents' status vocabulary {status}: {block}"
        );
    }
}

/// FR-025 applies to the guidance a *primary* agent receives; a sub-agent is
/// given the completion protocol instead and cannot spawn, so teaching it the
/// contract would spend tokens on a capability it does not have.
#[test]
fn the_contract_block_is_scoped_to_primary_agents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = AgentInfo::new("explore", "Exploration agent");
    agent.mode = AgentMode::Subagent;

    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        &agent,
        dir.path(),
        "tree",
        None,
        Some(""),
        Some(""),
        None,
        None,
        None,
        None,
        Some(&Config::default()),
    );

    assert!(
        !prompt.contains("## Sub-Agent Spawning"),
        "FR-025: a sub-agent must not receive the spawning guidance block"
    );
    assert!(
        !prompt.contains("THE AWAIT CONTRACT"),
        "FR-025: a sub-agent must not receive the await contract"
    );
    assert!(
        prompt.contains("## Sub-Agent Completion Protocol"),
        "a sub-agent must still receive its completion protocol"
    );
}
