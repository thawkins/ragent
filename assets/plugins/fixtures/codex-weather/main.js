// Codex-dialect fixture plugin (spec `plugins` acceptance criterion 1-3).
//
// The tool is declared in codex-plugin.json; dispatch resolves the handler from
// the global function named after the tool (tool_adapter fallback path). The
// top-level message proves entry execution is observable (FR-023: this message
// must NOT appear while the plugin is only installed, never enabled).

ragent.message.info("entry ran");

function get_weather(args) {
  var city = args && args.city ? String(args.city) : "unknown";
  return {
    city: city,
    conditions: "clear skies",
    temperature_c: 17
  };
}
