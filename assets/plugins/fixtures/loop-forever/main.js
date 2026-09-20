// Infinite-loop fixture (spec `plugins` acceptance criterion 5; FR-017/FR-026).
//
// Enabling this plugin must be contained by the entry budget: the sandbox
// interrupt handler aborts the loop roughly `plugins.max_entry_ms` after entry
// execution starts, the plugin is marked `errored` with a timeout cause, and the
// TUI stays responsive. The process must never hang or crash.

while (true) {}
