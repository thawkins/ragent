//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod <fixture>;`, so a shared helper file is a module of
// several children. Under one-binary-per-crate that loads it more than once,
// which clippy::duplicate_mod flags; hoisting it would require editing every
// child file. See AGENTS.md ("Test Layout").

#[path = "test_c_cpp_parser.rs"]
mod test_c_cpp_parser;
#[path = "test_cmake_parser.rs"]
mod test_cmake_parser;
#[path = "test_cmake_parser_extra.rs"]
mod test_cmake_parser_extra;
#[path = "test_codeindex.rs"]
mod test_codeindex;
#[path = "test_fts_diag.rs"]
mod test_fts_diag;
#[path = "test_go_parser.rs"]
mod test_go_parser;
#[path = "test_gradle_kts_parser.rs"]
mod test_gradle_kts_parser;
#[path = "test_gradle_kts_parser_extra.rs"]
mod test_gradle_kts_parser_extra;
#[path = "test_gradle_parser.rs"]
mod test_gradle_parser;
#[path = "test_gradle_parser_extra.rs"]
mod test_gradle_parser_extra;
#[path = "test_graph_build.rs"]
mod test_graph_build;
#[path = "test_graph_build_async.rs"]
mod test_graph_build_async;
#[path = "test_graph_communities.rs"]
mod test_graph_communities;
#[path = "test_graph_edges.rs"]
mod test_graph_edges;
#[path = "test_graph_edges_derive_extra.rs"]
mod test_graph_edges_derive_extra;
#[path = "test_graph_export.rs"]
mod test_graph_export;
#[path = "test_graph_readonly.rs"]
mod test_graph_readonly;
#[path = "test_graph_resolve.rs"]
mod test_graph_resolve;
#[path = "test_graph_resolve_regression.rs"]
mod test_graph_resolve_regression;
#[path = "test_graph_status.rs"]
mod test_graph_status;
#[path = "test_graph_store.rs"]
mod test_graph_store;
#[path = "test_graph_symbol_graph.rs"]
mod test_graph_symbol_graph;
#[path = "test_graph_traverse.rs"]
mod test_graph_traverse;
#[path = "test_graph_types.rs"]
mod test_graph_types;
#[path = "test_hcl_parser.rs"]
mod test_hcl_parser;
#[path = "test_java_parser.rs"]
mod test_java_parser;
#[path = "test_java_parser_extra.rs"]
mod test_java_parser_extra;
#[path = "test_m4_integration.rs"]
mod test_m4_integration;
#[path = "test_m6_data_layer.rs"]
mod test_m6_data_layer;
#[path = "test_m6_structural_bounds.rs"]
mod test_m6_structural_bounds;
#[path = "test_maven_parser.rs"]
mod test_maven_parser;
#[path = "test_ms03_codeindex_guards.rs"]
mod test_ms03_codeindex_guards;
#[path = "test_ms04_fts_symlink.rs"]
mod test_ms04_fts_symlink;
#[path = "test_openscad_parser.rs"]
mod test_openscad_parser;
#[path = "test_parse_store_integration.rs"]
mod test_parse_store_integration;
#[path = "test_parser_registry.rs"]
mod test_parser_registry;
#[path = "test_python_parser.rs"]
mod test_python_parser;
#[path = "test_rust_parser.rs"]
mod test_rust_parser;
#[path = "test_scan_store_integration.rs"]
mod test_scan_store_integration;
#[path = "test_scanner.rs"]
mod test_scanner;
#[path = "test_search.rs"]
mod test_search;
#[path = "test_search_integration.rs"]
mod test_search_integration;
#[path = "test_status_graph_fields.rs"]
mod test_status_graph_fields;
#[path = "test_store.rs"]
mod test_store;
#[path = "test_tree_cache.rs"]
mod test_tree_cache;
#[path = "test_types_display.rs"]
mod test_types_display;
#[path = "test_typescript_parser.rs"]
mod test_typescript_parser;
#[path = "test_watcher.rs"]
mod test_watcher;
#[path = "test_worker.rs"]
mod test_worker;
