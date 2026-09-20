// Host-API version-refusal fixture (spec `plugins` acceptance criterion 9; FR-019).
//
// The manifest declares api_version 99, newer than the host's v1, so enable and
// test both refuse with a version-mismatch report and this entry point is never
// executed.

ragent.message.info("future-api entry ran - this must never execute");
