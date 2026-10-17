//! Extra coverage tests for the CMake parser
//! (`crates/ragent-codeindex/src/parser/cmake.rs`).
//!
//! The relocated inline tests cover `function`, `macro`, a normal command, the
//! two import-like commands, and a `foreach` loop.  These tests drive the
//! remaining extraction branches: `block()`, `while()`, `if()` conditionals
//! (all with a nested body), a normal command with no identifier, and the
//! argument-list helpers.

use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::cmake::CmakeParser;
use ragent_codeindex::types::SymbolKind;

#[path = "support/mod.rs"]
mod support;

fn parse_cmake(source: &str) -> ParsedFile {
    support::parse_with(&CmakeParser::new(), source)
}

#[test]
fn test_block_def_extracts_module() {
    let src = r#"
block()
  message(STATUS "inside block")
endblock()
"#;
    let pf = parse_cmake(src);
    let block = pf
        .symbols
        .iter()
        .find(|s| s.name == "block")
        .expect("block() should be extracted");
    assert_eq!(block.kind, SymbolKind::Module);
    assert_eq!(block.signature.as_deref(), Some("block()"));
    // The nested normal command inside the body is walked and referenced.
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "message"),
        "nested command inside block body should be a reference"
    );
}

#[test]
fn test_while_loop_extracts_module() {
    let src = r"
while(count LESS 3)
  message(${count})
endwhile()
";
    let pf = parse_cmake(src);
    let w = pf
        .symbols
        .iter()
        .find(|s| s.name == "while")
        .expect("while() should be extracted");
    assert_eq!(w.kind, SymbolKind::Module);
    assert_eq!(w.signature.as_deref(), Some("while(count LESS 3)"));
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "message"),
        "nested command inside while body should be a reference"
    );
}

#[test]
fn test_if_condition_extracts_module() {
    let src = r#"
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  message("debug build")
endif()
"#;
    let pf = parse_cmake(src);
    let cond = pf
        .symbols
        .iter()
        .find(|s| s.name == "if")
        .expect("if() should be extracted");
    assert_eq!(cond.kind, SymbolKind::Module);
    assert_eq!(
        cond.signature.as_deref(),
        Some("if(CMAKE_BUILD_TYPE STREQUAL \"Debug\")")
    );
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "message"),
        "nested command inside if body should be a reference"
    );
}

#[test]
fn test_normal_command_unknown_name_is_still_a_reference() {
    let src = "custom_helper(arg1 arg2)";
    let pf = parse_cmake(src);
    let r = pf
        .references
        .iter()
        .find(|r| r.symbol_name == "custom_helper")
        .expect("normal command should be a call reference");
    assert_eq!(r.kind, "call");
}

#[test]
fn test_include_uses_first_argument_as_module() {
    let src = "include(MyModule)";
    let pf = parse_cmake(src);
    let imp = pf
        .imports
        .iter()
        .find(|i| i.kind == "include")
        .expect("include() should produce an import");
    assert_eq!(imp.source_module, "MyModule");
    assert_eq!(imp.imported_name, "MyModule");
}

#[test]
fn test_nested_function_body_walks_inner_command() {
    let src = r"
function(outer)
  add_library(inner STATIC a.cpp)
endfunction()
";
    let pf = parse_cmake(src);
    assert!(pf.symbols.iter().any(|s| s.name == "outer"));
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "add_library"),
        "command inside function body should be referenced"
    );
}
