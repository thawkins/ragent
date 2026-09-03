#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/maven.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::maven::MavenParser;
use ragent_codeindex::types::SymbolKind;

fn parse_maven(source: &str) -> ParsedFile {
    let parser = MavenParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_simple_pom() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <groupId>com.example</groupId>
  <artifactId>my-app</artifactId>
  <version>1.0.0</version>
  <packaging>jar</packaging>
</project>"#;
    let pf = parse_maven(src);
    assert!(
        pf.symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Module && s.name.starts_with("project"))
    );
}

#[test]
fn test_dependencies() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <dependencies>
    <dependency>
      <groupId>junit</groupId>
      <artifactId>junit</artifactId>
      <version>4.13.2</version>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>"#;
    let pf = parse_maven(src);
    let mut dep_imports = pf.imports.iter().filter(|i| i.kind == "dependency");
    assert_eq!(dep_imports.clone().count(), 1);
    assert_eq!(dep_imports.next().unwrap().imported_name, "junit");
}

#[test]
fn test_modules() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <modules>
    <module>core</module>
    <module>web</module>
  </modules>
</project>"#;
    let pf = parse_maven(src);
    assert_eq!(pf.imports.iter().filter(|i| i.kind == "module").count(), 2);
}

#[test]
fn test_plugins() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <build>
    <plugins>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-compiler-plugin</artifactId>
        <version>3.11.0</version>
      </plugin>
    </plugins>
  </build>
</project>"#;
    let pf = parse_maven(src);
    assert!(pf.symbols.iter().any(|s| s.name.starts_with("plugin:")));
}

#[test]
fn test_profile() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <profiles>
    <profile>
      <id>release</id>
    </profile>
  </profiles>
</project>"#;
    let pf = parse_maven(src);
    let profiles: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.name.starts_with("profile:"))
        .collect();
    assert!(!profiles.is_empty());
    assert_eq!(profiles[0].name, "profile:release");
}

#[test]
fn test_parent_import() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
  <parent>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>3.0.0</version>
  </parent>
</project>"#;
    let pf = parse_maven(src);
    let parent_imports: Vec<_> = pf.imports.iter().filter(|i| i.kind == "parent").collect();
    assert_eq!(parent_imports.len(), 1);
    assert_eq!(
        parent_imports[0].imported_name,
        "spring-boot-starter-parent"
    );
}

#[test]
fn test_empty_source() {
    let pf = parse_maven("");
    // Even empty input may produce a tree with an error node.
    // Just verify no crash.
    assert!(pf.symbols.is_empty() || pf.symbols.len() < 5);
}
