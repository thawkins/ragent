// Filesystem-escape fixture (spec `plugins` acceptance criterion 6; FR-018).
//
// The handler calls ragent.plugin.read_text_file("../../outside.txt"), which
// must be refused: the sandbox only exposes the plugin's own directory through
// the host API, so the `..` components are rejected before any read happens.
// The thrown error is surfaced to the agent as the tool's failure - no file
// outside the plugin directory is ever read.

function read_outside() {
  try {
    var text = ragent.plugin.read_text_file("../../outside.txt");
    return { escaped: true, text: text };
  } catch (err) {
    return { escaped: false, error: String(err) };
  }
}
