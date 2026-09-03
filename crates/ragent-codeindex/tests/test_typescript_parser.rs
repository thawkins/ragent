#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/typescript.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::typescript::{TsVariant, TypeScriptParser};
use ragent_codeindex::types::{SymbolKind, Visibility};

fn parse_ts(source: &str) -> ParsedFile {
    let p = TypeScriptParser::new(TsVariant::TypeScript);
    p.parse(source.as_bytes()).unwrap()
}

fn parse_js(source: &str) -> ParsedFile {
    let p = TypeScriptParser::new(TsVariant::JavaScript);
    p.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_function_declaration() {
    let parsed = parse_ts("function greet(name: string): void { console.log(name); }");
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "greet");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
}

#[test]
fn test_exported_function() {
    let parsed = parse_ts("export function hello(): string { return 'hi'; }");
    assert_eq!(parsed.symbols[0].visibility, Visibility::Public);
    assert!(
        parsed.symbols[0]
            .signature
            .as_ref()
            .unwrap()
            .contains("export")
    );
}

#[test]
fn test_class_with_methods() {
    let source = r#"
export class Dog {
    constructor(name: string) {
        this.name = name;
    }
    bark(): string {
        return "Woof!";
    }
}
"#;
    let parsed = parse_ts(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Dog"), "got: {names:?}");
    assert!(names.contains(&"constructor"), "got: {names:?}");
    assert!(names.contains(&"bark"), "got: {names:?}");

    let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
    assert_eq!(dog.kind, SymbolKind::Class);
    assert_eq!(dog.visibility, Visibility::Public);
}

#[test]
fn test_interface() {
    let source = r"
export interface User {
    name: string;
    age: number;
}
";
    let parsed = parse_ts(source);
    let user = parsed.symbols.iter().find(|s| s.name == "User").unwrap();
    assert_eq!(user.kind, SymbolKind::Interface);
    assert_eq!(user.visibility, Visibility::Public);
}

#[test]
fn test_type_alias() {
    let parsed = parse_ts("export type Result<T> = T | Error;");
    assert_eq!(parsed.symbols[0].name, "Result");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::TypeAlias);
}

#[test]
fn test_arrow_function() {
    let source = "export const add = (a: number, b: number): number => a + b;";
    let parsed = parse_ts(source);
    let add = parsed.symbols.iter().find(|s| s.name == "add").unwrap();
    assert_eq!(add.kind, SymbolKind::Function);
    assert_eq!(add.visibility, Visibility::Public);
}

#[test]
fn test_imports() {
    let source = r"
import { useState, useEffect } from 'react';
import axios from 'axios';
";
    let parsed = parse_ts(source);
    assert!(
        parsed.imports.len() >= 2,
        "got {} imports",
        parsed.imports.len()
    );

    let react = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "useState")
        .unwrap();
    assert_eq!(react.source_module, "react");
}

#[test]
fn test_javascript_function() {
    let parsed = parse_js("function hello() { return 'hi'; }");
    assert_eq!(parsed.symbols[0].name, "hello");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
}

#[test]
fn test_empty_source() {
    let parsed = parse_ts("");
    assert!(parsed.symbols.is_empty());
}

#[test]
fn test_enum_ts() {
    let source = r#"
export enum Direction {
    Up = "UP",
    Down = "DOWN",
}
"#;
    let parsed = parse_ts(source);
    let dir = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Direction")
        .unwrap();
    assert_eq!(dir.kind, SymbolKind::Enum);
    assert_eq!(dir.visibility, Visibility::Public);
}
