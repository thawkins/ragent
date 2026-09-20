// Placeholder entry for the malformed-manifest fixture (acceptance criterion 4
// refusal path; FR-026). The manifest JSON is truncated, so this file is never
// reached: discovery fails at parse time and no code executes.

ragent.message.info("bad-manifest entry ran - this must never execute");
