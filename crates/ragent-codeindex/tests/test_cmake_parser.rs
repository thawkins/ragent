//! External tests for `tests` from `crates/ragent-codeindex/src/parser/cmake.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::cmake::CmakeParser;
use ragent_codeindex::types::SymbolKind;

fn parse_cmake(source: &str) -> ParsedFile {
    let parser = CmakeParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_function_def() {
    let src = r#"
function(add_custom target)
  message(STATUS "Building ${target}")
endfunction()
"#;
    let pf = parse_cmake(src);
    let funcs: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "add_custom");
}

#[test]
fn test_macro_def() {
    let src = r#"
macro(assert_test name)
  message("Testing ${name}")
endmacro()
"#;
    let pf = parse_cmake(src);
    let macros: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Macro)
        .collect();
    assert_eq!(macros.len(), 1);
    assert_eq!(macros[0].name, "assert_test");
}

#[test]
fn test_normal_command_reference() {
    let src = "add_library(mylib STATIC src.cpp)";
    let pf = parse_cmake(src);
    assert!(!pf.references.is_empty());
    assert_eq!(pf.references[0].symbol_name, "add_library");
    assert_eq!(pf.references[0].kind, "call");
}

#[test]
fn test_include_import() {
    let src = "include(GNUInstallDirs)";
    let pf = parse_cmake(src);
    assert!(!pf.imports.is_empty());
    assert_eq!(pf.imports[0].kind, "include");
}

#[test]
fn test_add_subdirectory_import() {
    let src = "add_subdirectory(libs)";
    let pf = parse_cmake(src);
    assert!(!pf.imports.is_empty());
    assert_eq!(pf.imports[0].kind, "add_subdirectory");
}

#[test]
fn test_foreach_loop() {
    let src = r"
foreach(item IN ITEMS a b c)
  message(${item})
endforeach()
";
    let pf = parse_cmake(src);
    assert_eq!(pf.symbols.iter().filter(|s| s.name == "foreach").count(), 1);
}

#[test]
fn test_empty_source() {
    let pf = parse_cmake("");
    assert!(pf.symbols.is_empty());
    assert!(pf.imports.is_empty());
}
