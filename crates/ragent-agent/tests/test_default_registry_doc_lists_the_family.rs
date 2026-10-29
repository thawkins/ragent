//! Guard test for `create_default_registry`'s rustdoc (spec `agentssetup` G1).
//!
//! SPEC.md G1 recorded a concrete documentation drift: the rustdoc on
//! `create_default_registry` (`crates/ragent-agent/src/tool/mod.rs`) listed
//! `new_agent`, `cancel_agent` and `list_agents` but **not** `wait_agents`,
//! while the function registers all four. A doc that under-reports the registry
//! is worse than no list at all: a reader trusts it, and the tool it omits is
//! the one a caller needs to know about.
//!
//! T-027 fixed the prose and this guard keeps it fixed. Rather than asserting a
//! hand-copied list (which would be the same kind of artefact that drifted), the
//! test reads the rustdoc text directly out of `tool/mod.rs` with
//! `include_str!` and checks every sub-agent family tool the registry actually
//! registers is named in it. A future tool added to the family therefore fails
//! here until it is documented, and a documented tool that is later removed
//! fails too, in the other direction.
//!
//! Nothing here executes a tool, so no `ToolContext` is built.

use ragent_agent::tool::create_default_registry;

/// The `create_default_registry` rustdoc, read from the source at compile time.
///
/// `include_str!` resolves relative to this test file, and because the test is
/// a `#[path]`-included child of the `all` target root the path is still
/// anchored at the child file's directory (`tests/`), so `../src/tool/mod.rs`
/// is the crate source. Reading the text rather than the rendered HTML keeps
/// this a plain unit test with no `rustdoc` invocation.
const TOOL_MOD_SRC: &str = include_str!("../src/tool/mod.rs");

/// Extract the `create_default_registry` doc block from the module source.
///
/// The block is the contiguous run of `///` lines immediately above
/// `pub fn create_default_registry`. Scanning upward from the signature finds
/// it without depending on a line number, so an unrelated edit elsewhere in the
/// (large) module cannot break the guard.
fn registry_doc_block() -> String {
    let lines: Vec<&str> = TOOL_MOD_SRC.lines().collect();
    let sig = lines
        .iter()
        .position(|line| line.contains("pub fn create_default_registry()"))
        .expect("create_default_registry must exist in tool/mod.rs");

    // Walk upward over doc lines, tolerating the `#[must_use]` attribute that
    // sits between the doc block and the signature.
    let mut block: Vec<&str> = Vec::new();
    for line in lines[..sig].iter().rev() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("///") {
            block.push(trimmed);
            continue;
        }
        if trimmed.starts_with("#[") {
            continue;
        }
        break;
    }
    assert!(
        !block.is_empty(),
        "create_default_registry must carry a rustdoc block (G1)"
    );
    block.reverse();
    block.join("\n")
}

/// G1: the registry rustdoc names every sub-agent family tool the registry
/// registers.
///
/// The family is `new_agent`, `cancel_agent`, `list_agents` and `wait_agents`.
/// `wait_agents` is the one G1 recorded as missing, so it is asserted
/// explicitly and separately from the loop - a regression there must fail with
/// the name of the drift, not a generic "missing tool" message.
#[test]
fn test_registry_doc_names_every_sub_agent_family_tool() {
    let registry = create_default_registry();
    let registered: Vec<String> = registry.list();
    let doc = registry_doc_block();

    // Guard the guard: if the family tools stop being registered, this test
    // would pass vacuously, so assert they are all present first.
    for tool in ["new_agent", "cancel_agent", "list_agents", "wait_agents"] {
        assert!(
            registered.iter().any(|name| name == tool),
            "create_default_registry must register `{tool}`"
        );
    }

    assert!(
        doc.contains("wait_agents"),
        "G1: the create_default_registry rustdoc must name `wait_agents`; \
         the doc block was:\n{doc}"
    );

    for tool in ["new_agent", "cancel_agent", "list_agents", "wait_agents"] {
        assert!(
            doc.contains(tool),
            "G1: the create_default_registry rustdoc must name `{tool}`; \
             the doc block was:\n{doc}"
        );
    }
}

/// G1: the rustdoc describes the family as one contract, not as four unrelated
/// tools.
///
/// The drift G1 recorded was not only a missing name: the list gave no hint
/// that the four tools share the execution-contract vocabulary, so a reader
/// could not tell which one to reach for. The doc now names the contract, the
/// detached refusal and the honest cancel status, and this asserts those words
/// survive future edits.
#[test]
fn test_registry_doc_describes_the_family_contract() {
    let doc = registry_doc_block();

    for expected in ["execution contract", "detached", "FR-044", "FR-013"] {
        assert!(
            doc.contains(expected),
            "G1: the registry rustdoc must describe the family contract \
             (missing `{expected}`); the doc block was:\n{doc}"
        );
    }
}
