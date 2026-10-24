//! Guard: a plugin-contributed tool is namespaced, so a tool the plugin calls
//! `bash` cannot resurrect the retired spelling (spec `uniformtools` FR-032).
//!
//! A plugin writes its tool names in JavaScript (`ragent.register_tool({ name })`),
//! which no compiler in this workspace can rename when the FR-023 rename retires a
//! ragent spelling. The bridge answers that the same way the MCP bridge does: the
//! registry name is `plugin_<plugin-id>_<tool>`, which is never a bare ragent name.
//!
//! This file pins the property at the bridge, through the real registration path,
//! so it fails if the prefix is ever dropped - the FR-032 backstop in
//! `register_plugin_tools` would then report a refusal, and the case below would see
//! a missing tool rather than a namespaced one.

use std::collections::BTreeSet;
use std::sync::Arc;

use ragent_plugins::{LifecycleState, PluginManager, PluginToolAdapter};
use ragent_types::tool_bridge::{check_dynamic_bridge, plugin_tool_name, plugin_tool_prefix};
use ragent_types::tool_rename::TOOL_RENAMES;

mod support;

use support::TempTree;

/// Write a Codex-dialect plugin whose entry registers one tool, `tool_name`.
fn write_plugin_registering(tree: &TempTree, id: &str, tool_name: &str) {
    let dir = tree.store().join(id);
    std::fs::create_dir_all(&dir).expect("plugin dir creatable");
    std::fs::write(
        dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest writable");
    std::fs::write(
        dir.join("index.js"),
        format!(
            "ragent.register_tool({{ name: \"{tool_name}\", description: \"d\", \
             parameters: {{}}, handler: function(){{ return \"ok\"; }} }});"
        ),
    )
    .expect("entry writable");
}

/// Enable `id` and register its tools, returning the registered names.
fn enable_and_register(manager: &PluginManager, id: &str) -> Vec<String> {
    manager
        .register_plugin_tools(id, &BTreeSet::new(), |_: Arc<PluginToolAdapter>| {})
        .expect("registration succeeds")
}

#[test]
fn a_plugin_tool_named_after_a_retired_spelling_registers_under_its_namespace() {
    let tree = TempTree::new("fr032-retired-name");
    write_plugin_registering(&tree, "codex-shell", "bash");
    let mut manager = tree.manager();
    assert_eq!(
        manager.enable("codex-shell").expect("enable").state,
        LifecycleState::Loaded
    );

    let names = enable_and_register(&manager, "codex-shell");
    assert_eq!(names, vec!["plugin_codex-shell_bash".to_string()]);
    assert_ne!(
        names[0], "bash",
        "the plugin's `bash` tool must not occupy the retired spelling"
    );
    // The backstop agrees: the namespaced form may be bridged, while the bare form
    // would be refused with `Bash` named.
    let prefix = plugin_tool_prefix("codex-shell");
    assert!(check_dynamic_bridge("plugin `codex-shell`", &prefix, &names[0]).is_ok());
    let refusal = check_dynamic_bridge("plugin `codex-shell`", &prefix, "bash")
        .expect_err("the bare spelling would be refused");
    assert!(
        refusal.sentence().contains("`Bash`"),
        "{}",
        refusal.sentence()
    );
}

#[test]
fn no_plugin_tool_can_be_registered_under_any_retired_spelling() {
    // The exhaustive form: for all thirteen pairs, the bridged name is neither the
    // retired nor the adopted bare spelling.
    for pair in TOOL_RENAMES {
        for tool_name in [pair.before, pair.after] {
            let name = plugin_tool_name("p", tool_name);
            assert!(name.starts_with("plugin_p_"), "`{name}` must be namespaced");
            assert!(
                !ragent_types::tool_rename::Rename::is_retired(&name),
                "`{name}` must not be a retired spelling"
            );
            assert!(
                check_dynamic_bridge("plugin `p`", &plugin_tool_prefix("p"), &name).is_ok(),
                "`{name}` must pass the bridge check"
            );
        }
    }
}
