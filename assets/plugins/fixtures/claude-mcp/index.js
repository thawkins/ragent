// Unsupported-capability fixture (spec `plugins` acceptance criterion 13; FR-025).
//
// The manifest carries an `mcp_servers` section, which the in-process host API
// cannot satisfy. It must be recorded as the unsupported capability
// "mcp server transports" and reported in `/plugins list` and `/plugins test`
// output rather than silently dropped.

function lookup_docs(args) {
  var topic = args && args.topic ? String(args.topic) : "unknown";
  return { topic: topic, found: false };
}
