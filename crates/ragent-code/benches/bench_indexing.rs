#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_code::parser::ParserRegistry;
use ragent_code::types::{CodeIndexConfig, SearchQuery};
use std::path::Path;
use tempfile::tempdir;

// ── Sample sources for each language ──────────────────────────────────────────

const RUST_SRC: &str = r#"
pub struct Config { name: String, value: i32 }
impl Config {
    pub fn new(name: &str) -> Self { Self { name: name.to_string(), value: 0 } }
    pub fn value(&self) -> i32 { self.value }
}
fn helper() -> bool { true }
"#;

const PYTHON_SRC: &str = r#"
class Config:
    def __init__(self, name: str):
        self.name = name

    def value(self) -> int:
        return 0

def helper():
    return True
"#;

const TS_SRC: &str = r#"
export interface Config { name: string; value: number }
export class ConfigImpl implements Config {
    name: string;
    value: number;
    constructor(name: string) { this.name = name; this.value = 0; }
}
export function helper(): boolean { return true; }
const LIMIT = 100;
"#;

const GO_SRC: &str = r#"
package main

type Config struct {
    Name  string
    Value int
}

func NewConfig(name string) *Config {
    return &Config{Name: name}
}

func helper() bool { return true }
"#;

const C_SRC: &str = r#"
#include <stdio.h>
typedef unsigned long size_t;
struct Config { char* name; int value; };
int helper(void) { return 1; }
void process(struct Config* c) { c->value++; }
"#;

const CPP_SRC: &str = r#"
#include <string>
class Config {
public:
    Config(const std::string& name) : name_(name), value_(0) {}
    int value() const { return value_; }
private:
    std::string name_;
    int value_;
};
namespace util { bool helper() { return true; } }
"#;

const JAVA_SRC: &str = r#"
package com.example;

public class Config {
    private String name;
    private int value;

    public Config(String name) {
        this.name = name;
        this.value = 0;
    }

    public int getValue() { return value; }
}
"#;

// ── Benchmarks ────────────────────────────────────────────────────────────────

fn bench_parse_single(c: &mut Criterion) {
    let registry = ParserRegistry::new();

    let cases: Vec<(&str, &str)> = vec![
        ("rust", RUST_SRC),
        ("python", PYTHON_SRC),
        ("typescript", TS_SRC),
        ("go", GO_SRC),
        ("c", C_SRC),
        ("cpp", CPP_SRC),
        ("java", JAVA_SRC),
    ];

    let mut group = c.benchmark_group("parse_single");
    for (lang, source) in &cases {
        let parser = registry.get(lang).expect(lang);
        group.bench_function(*lang, |b| {
            b.iter(|| parser.parse(source.as_bytes()).unwrap());
        });
    }
    group.finish();
}

fn bench_parse_repeated(c: &mut Criterion) {
    let registry = ParserRegistry::new();
    let parser = registry.get("rust").unwrap();

    let mut group = c.benchmark_group("parse_rust_repeated");
    for &count in &[10, 50, 100] {
        group.bench_function(format!("parse_x{count}"), |b| {
            b.iter(|| {
                for _ in 0..count {
                    let _ = parser.parse(RUST_SRC.as_bytes()).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_store_upsert(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("bench.rs"), RUST_SRC).unwrap();

    let mut group = c.benchmark_group("store_upsert");
    group.bench_function("index_rust_file", |b| {
        let config = CodeIndexConfig {
            project_root: dir.path().to_path_buf(),
            ..Default::default()
        };
        let index = ragent_code::CodeIndex::open_in_memory(&config).unwrap();
        b.iter(|| {
            index.index_file(Path::new("bench.rs")).unwrap();
        });
    });
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let files: Vec<(&str, &str)> = vec![
        ("test.rs", RUST_SRC),
        ("test.py", PYTHON_SRC),
        ("test.ts", TS_SRC),
        ("test.go", GO_SRC),
        ("test.c", C_SRC),
        ("test.cpp", CPP_SRC),
        ("Test.java", JAVA_SRC),
    ];

    for (name, src) in &files {
        std::fs::write(dir.path().join(name), src).unwrap();
    }

    let config = CodeIndexConfig {
        project_root: dir.path().to_path_buf(),
        ..Default::default()
    };
    let index = ragent_code::CodeIndex::open_in_memory(&config).unwrap();
    for (name, _) in &files {
        index.index_file(Path::new(name)).unwrap();
    }

    let mut group = c.benchmark_group("search");
    group.bench_function("search_Config", |b| {
        b.iter(|| {
            let q = SearchQuery::new("Config");
            index.search(&q).unwrap()
        });
    });
    group.bench_function("search_helper", |b| {
        b.iter(|| {
            let q = SearchQuery::new("helper");
            index.search(&q).unwrap()
        });
    });
    group.finish();
}

fn bench_full_index_small(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("main.rs"), RUST_SRC).unwrap();
    std::fs::write(root.join("lib.py"), PYTHON_SRC).unwrap();
    std::fs::write(root.join("app.ts"), TS_SRC).unwrap();
    std::fs::write(root.join("main.go"), GO_SRC).unwrap();
    std::fs::write(root.join("util.c"), C_SRC).unwrap();
    std::fs::write(root.join("util.cpp"), CPP_SRC).unwrap();
    std::fs::write(root.join("App.java"), JAVA_SRC).unwrap();

    let mut group = c.benchmark_group("full_index");
    group.bench_function("7_files_7_langs", |b| {
        b.iter(|| {
            let config = CodeIndexConfig {
                enabled: true,
                project_root: root.to_path_buf(),
                ..Default::default()
            };
            let index = ragent_code::CodeIndex::open_in_memory(&config).unwrap();
            index.full_reindex().unwrap();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_single,
    bench_parse_repeated,
    bench_store_upsert,
    bench_search,
    bench_full_index_small,
);
criterion_main!(benches);
