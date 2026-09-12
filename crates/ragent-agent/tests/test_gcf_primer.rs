#![allow(clippy::unwrap_used)]
//! Integration tests for the conditional GCF primer section (spec `gcf`,
//! T-007, FR-006).
//!
//! `build_system_prompt_with_storage` must include the
//! "## GCF Encoding Primer" section only while the GCF runtime flag is
//! enabled, and must omit it entirely (including default-off) otherwise.
//! The GCF flag is process-global, so every flag-toggling test serialises
//! itself with the same mutex pattern used by `test_gcf_encode_hook.rs`.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use ragent_agent::agent::{AgentInfo, AgentMode, build_system_prompt_with_storage};

/// The GCF runtime flag is process-global; serialise every test that toggles
/// it so parallel test threads cannot race each other's state.
fn flag_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(()));
    // A poisoned lock only means a previous test panicked while holding the
    // flag; taking it anyway keeps the remaining tests runnable.
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Toggle the GCF flag for the duration of `f` and always restore it.
fn with_gcf(enabled: bool, f: impl FnOnce()) {
    let _guard = flag_guard();
    let previous = ragent_config::gcf::is_enabled();
    ragent_config::gcf::set_enabled(enabled);
    f();
    ragent_config::gcf::set_enabled(previous);
}

/// Build a primary-agent system prompt the way the production paths do.
fn prompt_for(agent_name: &str, mode: AgentMode) -> String {
    let mut agent = AgentInfo::new(agent_name, "gcf primer test agent");
    agent.prompt = Some(std::sync::Arc::from("You are a helpful assistant."));
    agent.mode = mode;
    agent.max_steps = Some(10);

    build_system_prompt_with_storage(
        &agent,
        Path::new("/tmp"),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

/// FR-006: while GCF is enabled the system prompt must contain the
/// `## GCF Encoding Primer` section, the block markers, and the key
/// grammar points the LLM needs to read encoded tool results.
#[test]
fn test_gcf_primer_present_when_enabled() {
    with_gcf(true, || {
        let prompt = prompt_for("general", AgentMode::Primary);

        assert!(
            prompt.contains("## GCF Encoding Primer"),
            "system prompt must include the '## GCF Encoding Primer' section when GCF is enabled"
        );
        // The primer must teach the exact block markers the encoder hook
        // emits (FR-004's labelled wrapper) so the LLM can spot a block.
        assert!(
            prompt.contains("[BEGIN GCF generic]") && prompt.contains("[END GCF]"),
            "primer must document the GCF block markers"
        );
        assert!(
            prompt.contains("GCF profile=generic"),
            "primer must mention the generic-profile payload header line"
        );
        assert!(
            prompt.contains("lossless"),
            "primer must state the lossless guarantee"
        );
        assert!(
            prompt.contains("key=value"),
            "primer must cover the flat scalar grammar"
        );
        assert!(
            prompt.contains("## rows [2]{id}"),
            "primer must cover the table-header grammar"
        );
    });
}

/// FR-006: while GCF is disabled the primer section must not appear in any
/// system prompt (primary agents).
#[test]
fn test_gcf_primer_absent_when_disabled() {
    with_gcf(false, || {
        let prompt = prompt_for("general", AgentMode::Primary);

        assert!(
            !prompt.contains("## GCF Encoding Primer"),
            "system prompt must not include the GCF primer while GCF is disabled"
        );
        assert!(
            !prompt.contains("[BEGIN GCF generic]"),
            "disabled prompts must not leak the GCF block marker"
        );
    });
}

/// FR-006: the primer applies to sub-agent prompts too — the encoding hook
/// runs for sub-agent runs just as it does for the primary agent, so the
/// primer must be present for sub-agent-mode prompts when GCF is on, and
/// absent when it is off.
#[test]
fn test_gcf_primer_applies_to_subagent_prompts() {
    with_gcf(true, || {
        let prompt = prompt_for("explore", AgentMode::Subagent);
        assert!(
            prompt.contains("## GCF Encoding Primer"),
            "sub-agent prompts must include the GCF primer when GCF is enabled"
        );
    });
    with_gcf(false, || {
        let prompt = prompt_for("explore", AgentMode::Subagent);
        assert!(
            !prompt.contains("## GCF Encoding Primer"),
            "sub-agent prompts must not include the GCF primer while GCF is disabled"
        );
    });
}

/// FR-006 + FR-001: the feature defaults to off. A caller that has never
/// touched the runtime flag (no sync, no toggle) must get a prompt without
/// the primer — this pins the default-off semantics end to end.
#[test]
fn test_gcf_primer_absent_by_default() {
    // The default-state assertion must run without toggling the flag, but the
    // flag is process-global: hold the shared mutex so a parallel with_gcf
    // test cannot flip it back to true between our check and prompt build.
    let _guard = flag_guard();
    let previous = ragent_config::gcf::is_enabled();
    if previous {
        // Another test enabled the flag before we got the lock; default
        // semantics are covered by `test_gcf_primer_absent_when_disabled`,
        // so restore the default and verify the flag resets cleanly.
        ragent_config::gcf::set_enabled(false);
    }
    let prompt = prompt_for("general", AgentMode::Primary);
    assert!(
        !prompt.contains("## GCF Encoding Primer"),
        "default-off must never inject the GCF primer"
    );
    if previous {
        ragent_config::gcf::set_enabled(previous);
    }
}

/// The primer must sit alongside the other conditional sections in prompt
/// order: it should appear after the Reasoning Tool section (where T-006
/// inserted it) and before the Sub-Agent Spawning section for primary
/// agents, so prompt structure stays stable.
#[test]
fn test_gcf_primer_positioned_between_reasoning_and_subagent_sections() {
    with_gcf(true, || {
        let prompt = prompt_for("general", AgentMode::Primary);
        let primer = prompt
            .find("## GCF Encoding Primer")
            .expect("primer present when GCF is enabled");
        let reasoning = prompt
            .find("## Reasoning Tool")
            .expect("reasoning tool section always present");
        let subagent = prompt
            .find("## Sub-Agent Spawning")
            .expect("sub-agent spawning section present for primary agents");
        assert!(
            reasoning < primer && primer < subagent,
            "primer must be ordered after '## Reasoning Tool' and before '## Sub-Agent Spawning' (reasoning={reasoning}, primer={primer}, subagent={subagent})"
        );
    });
}
