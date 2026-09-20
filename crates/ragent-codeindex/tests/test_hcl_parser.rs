//! External tests for `tests` from `crates/ragent-codeindex/src/parser/hcl.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::hcl::HclParser;
use ragent_codeindex::types::SymbolKind;

fn parse_hcl(source: &str) -> ParsedFile {
    let parser = HclParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_resource_block() {
    let src = r#"
resource "aws_instance" "web" {
  ami           = "ami-12345"
  instance_type = "t2.micro"
}
"#;
    let pf = parse_hcl(src);
    assert!(!pf.symbols.is_empty());
    let blocks: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .collect();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].name, "resource.aws_instance.web");
}

#[test]
fn test_variable_block() {
    let src = r#"
variable "instance_count" {
  default = 2
}
"#;
    let pf = parse_hcl(src);
    let vars: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Constant)
        .collect();
    assert!(!vars.is_empty());
    assert_eq!(vars[0].name, "variable.instance_count");
}

#[test]
fn test_output_block() {
    let src = r#"
output "instance_ip" {
  value = aws_instance.web.public_ip
}
"#;
    let pf = parse_hcl(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Field));
}

#[test]
fn test_locals_block() {
    let src = r#"
locals {
  env  = "production"
  name = "myapp"
}
"#;
    let pf = parse_hcl(src);
    assert!(
        pf.symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .count()
            >= 2,
        "Expected at least 2 locals constants, got {:?}",
        pf.symbols
    );
}

#[test]
fn test_module_block() {
    let src = r#"
module "vpc" {
  source = "./modules/vpc"
}
"#;
    let pf = parse_hcl(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Module));
}

#[test]
fn test_provider_block() {
    let src = r#"
provider "aws" {
  region = "us-east-1"
}
"#;
    let pf = parse_hcl(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Module));
}

#[test]
fn test_terraform_block() {
    let src = r#"
terraform {
  required_version = ">= 1.0"
}
"#;
    let pf = parse_hcl(src);
    assert!(
        pf.symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Module && s.name.starts_with("terraform"))
    );
}

#[test]
fn test_data_block() {
    let src = r#"
data "aws_ami" "ubuntu" {
  most_recent = true
}
"#;
    let pf = parse_hcl(src);
    let data_blocks: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class && s.name.starts_with("data."))
        .collect();
    assert_eq!(data_blocks.len(), 1);
    assert_eq!(data_blocks[0].name, "data.aws_ami.ubuntu");
}

#[test]
fn test_empty_source() {
    let pf = parse_hcl("");
    assert!(pf.symbols.is_empty());
    assert!(pf.imports.is_empty());
}
