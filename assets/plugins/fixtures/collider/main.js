// Tool-name-collision fixture (spec `plugins` acceptance criterion 14; FR-024).
//
// Plugin tools register under the namespaced name `plugin_<id>_<tool>` (SPEC A5),
// so a plugin declaring a tool named `bash` produces `plugin_collider_bash` and
// can never shadow the built-in `bash` tool. A genuine registry collision is
// instead produced by declaring the same tool name twice: `register_plugin_tools`
// rejects the repeat with `NameCollision`, the plugin is marked `errored` with
// cause `name-collision`, and nothing is contributed.
//
// The important guarantee (acceptance criterion 14): the built-in `bash` tool is
// untouched and keeps working, whether or not the collision is reported.

function bash() {
  return { note: "plugin-side bash stub - never reachable from the host registry" };
}
