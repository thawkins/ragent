#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/openscad.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::openscad::OpenScadParser;
use ragent_codeindex::types::SymbolKind;

fn parse_scad(source: &str) -> ParsedFile {
    let parser = OpenScadParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_module_item() {
    let src = "module bracket(size=10) { cube(size); }";
    let pf = parse_scad(src);
    // Should have the module definition plus the cube call inside creates
    // a module_call reference, but may also produce an assignment symbol.
    assert!(!pf.symbols.is_empty());
    let module = pf.symbols.iter().find(|s| s.name == "bracket").unwrap();
    assert_eq!(module.kind, SymbolKind::Module);
    assert_eq!(module.start_line, 1);
}

#[test]
fn test_function_item() {
    let src = "function deg2rad(d) = d * 180 / PI;";
    let pf = parse_scad(src);
    assert_eq!(pf.symbols.len(), 1);
    let s = &pf.symbols[0];
    assert_eq!(s.name, "deg2rad");
    assert_eq!(s.kind, SymbolKind::Function);
}

#[test]
fn test_var_declaration() {
    let src = "tolerance = 0.2;";
    let pf = parse_scad(src);
    assert_eq!(pf.symbols.len(), 1);
    let s = &pf.symbols[0];
    assert_eq!(s.name, "tolerance");
    assert_eq!(s.kind, SymbolKind::Constant);
}

#[test]
fn test_include_use() {
    let src = r"include <MCAD/stepper.scad>
use <utils.scad>";
    let pf = parse_scad(src);
    assert_eq!(pf.imports.len(), 2);
    assert_eq!(pf.imports[0].kind, "include");
    assert_eq!(pf.imports[1].kind, "use");
}

#[test]
fn test_module_call_reference() {
    let src = "sphere(r=10);";
    let pf = parse_scad(src);
    assert!(!pf.references.is_empty());
    assert_eq!(pf.references[0].symbol_name, "sphere");
    assert_eq!(pf.references[0].kind, "call");
}

#[test]
fn test_nested_module() {
    let src = r"
module housing() {
    difference() {
        cube(20, center=true);
        sphere(r=9);
    }
}
";
    let pf = parse_scad(src);
    // housing module + references for difference, cube, sphere
    assert!(!pf.symbols.is_empty());
    assert_eq!(pf.symbols[0].name, "housing");
    assert!(!pf.references.is_empty());
}

#[test]
fn test_special_variable_skipped() {
    // $fn, $fa, $fs are OpenSCAD special variables — should not be
    // extracted as user-defined constants.
    let src = "$fn = 64;";
    let pf = parse_scad(src);
    assert!(pf.symbols.is_empty());
}

#[test]
fn test_empty_source() {
    let pf = parse_scad("");
    assert!(pf.symbols.is_empty());
    assert!(pf.imports.is_empty());
}
