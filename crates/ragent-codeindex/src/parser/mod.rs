//! Tree-sitter–based source code parsing and symbol extraction.
//!
//! This module defines the `LanguageParser` trait and a [`ParserRegistry`]
//! that dispatches parsing to language-specific implementations.

pub mod c_cpp;
pub mod cmake;
pub mod go;
pub mod gradle;
pub mod gradle_kts;
pub mod hcl;
pub mod java;
pub mod maven;
pub mod openscad;
pub mod python;
pub mod rust;
pub mod typescript;

use crate::types::{ImportEntry, Symbol, SymbolRef};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// The output of parsing a single source file.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    /// Symbols extracted from the file (functions, structs, traits, etc.).
    pub symbols: Vec<Symbol>,
    /// Import / use statements found in the file.
    pub imports: Vec<ImportEntry>,
    /// References to symbols from other files/modules.
    pub references: Vec<SymbolRef>,
    /// The tree-sitter parse tree (retained for incremental re-parsing).
    pub tree: Option<tree_sitter::Tree>,
}

impl ParsedFile {
    /// Create an empty parsed file.
    pub fn empty() -> Self {
        Self {
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            tree: None,
        }
    }
}

/// Trait for language-specific parsers.
///
/// Each implementation knows how to parse one language's source code
/// into structured symbols using tree-sitter.
pub trait LanguageParser: Send + Sync {
    /// The language identifier (e.g. `"rust"`, `"python"`).
    fn language_id(&self) -> &'static str;

    /// Parse source code and extract symbols, imports, and references.
    fn parse(&self, source: &[u8]) -> Result<ParsedFile>;
}

/// Registry of language parsers, keyed by language identifier.
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn LanguageParser>>,
}

impl ParserRegistry {
    /// Create a new registry with all built-in language parsers.
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Arc<dyn LanguageParser>> = HashMap::new();

        let rust_parser = Arc::new(rust::RustParser::new());
        parsers.insert("rust".to_string(), rust_parser);

        let python_parser = Arc::new(python::PythonParser::new());
        parsers.insert("python".to_string(), python_parser);

        let ts_parser = Arc::new(typescript::TypeScriptParser::new(
            typescript::TsVariant::TypeScript,
        ));
        parsers.insert("typescript".to_string(), ts_parser);

        let tsx_parser = Arc::new(typescript::TypeScriptParser::new(
            typescript::TsVariant::Tsx,
        ));
        parsers.insert("tsx".to_string(), tsx_parser);

        let js_parser = Arc::new(typescript::TypeScriptParser::new(
            typescript::TsVariant::JavaScript,
        ));
        parsers.insert("javascript".to_string(), js_parser);

        let jsx_parser = Arc::new(typescript::TypeScriptParser::new(
            typescript::TsVariant::Jsx,
        ));
        parsers.insert("jsx".to_string(), jsx_parser);

        let go_parser = Arc::new(go::GoParser::new());
        parsers.insert("go".to_string(), go_parser);

        let c_parser = Arc::new(c_cpp::CParser::new());
        parsers.insert("c".to_string(), c_parser.clone());
        parsers.insert("c_header".to_string(), c_parser);

        let cpp_parser = Arc::new(c_cpp::CppParser::new());
        parsers.insert("cpp".to_string(), cpp_parser.clone());
        parsers.insert("cpp_header".to_string(), cpp_parser);

        let java_parser = Arc::new(java::JavaParser::new());
        parsers.insert("java".to_string(), java_parser);

        let hcl_parser = Arc::new(hcl::HclParser::new());
        parsers.insert("terraform".to_string(), hcl_parser);

        let openscad_parser = Arc::new(openscad::OpenScadParser::new());
        parsers.insert("openscad".to_string(), openscad_parser);

        let cmake_parser = Arc::new(cmake::CmakeParser::new());
        parsers.insert("cmake".to_string(), cmake_parser);

        let gradle_parser = Arc::new(gradle::GradleParser::new());
        parsers.insert("gradle".to_string(), gradle_parser);

        let gradle_kts_parser = Arc::new(gradle_kts::GradleKtsParser::new());
        parsers.insert("gradle_kts".to_string(), gradle_kts_parser);

        let maven_parser = Arc::new(maven::MavenParser::new());
        parsers.insert("maven".to_string(), maven_parser);

        Self { parsers }
    }

    /// Look up a parser by language id.
    pub fn get(&self, language: &str) -> Option<Arc<dyn LanguageParser>> {
        self.parsers.get(language).cloned()
    }

    /// List all supported language ids.
    pub fn supported_languages(&self) -> Vec<&str> {
        self.parsers.keys().map(|s| s.as_str()).collect()
    }

    /// Parse a file given its source code and detected language.
    ///
    /// Returns `None` if no parser is registered for the language.
    pub fn parse(&self, language: &str, source: &[u8]) -> Option<Result<ParsedFile>> {
        self.get(language).map(|parser| parser.parse(source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_rust() {
        let reg = ParserRegistry::new();
        assert!(reg.get("rust").is_some());
        assert!(reg.get("python").is_some());
        assert!(reg.get("openscad").is_some());
        assert!(reg.get("terraform").is_some());
        assert!(reg.get("nonexistent_lang").is_none());
    }

    #[test]
    fn test_supported_languages() {
        let reg = ParserRegistry::new();
        let langs = reg.supported_languages();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"openscad"));
        assert!(langs.contains(&"terraform"));
        assert!(langs.contains(&"cmake"));
        assert!(langs.contains(&"gradle"));
        assert!(langs.contains(&"gradle_kts"));
        assert!(langs.contains(&"maven"));
    }

    #[test]
    fn test_parse_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse("rust", b"fn main() {}");
        assert!(result.is_some());
        let parsed = result.unwrap().unwrap();
        assert!(!parsed.symbols.is_empty());
    }

    #[test]
    fn test_parse_openscad_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse("openscad", b"module foo() { cube(10); }");
        assert!(result.is_some());
        let parsed = result.unwrap().unwrap();
        assert!(!parsed.symbols.is_empty());
    }

    #[test]
    fn test_parse_terraform_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse(
            "terraform",
            b"resource \"aws_instance\" \"web\" { ami = \"ami-123\" }",
        );
        assert!(result.is_some());
        let parsed = result.unwrap().unwrap();
        assert!(!parsed.symbols.is_empty());
    }

    #[test]
    fn test_parse_cmake_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse("cmake", b"function(my_func) endfunction()");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_gradle_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse("gradle", b"plugins { id 'java' }");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_gradle_kts_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse("gradle_kts", b"plugins { java }");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_maven_dispatch() {
        let reg = ParserRegistry::new();
        let result = reg.parse(
            "maven",
            b"<?xml version=\"1.0\"?><project><artifactId>test</artifactId></project>",
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_unknown_language() {
        let reg = ParserRegistry::new();
        assert!(reg.parse("brainfuck", b"+++").is_none());
    }
}
