//! Criterion benchmarks for cursor operations and text editing.
//!
//! Covers COMPLIANCE.md Section 5.C:
//! - cursor_byte_pos_at_char_index on strings of 1k, 10k, 100k chars (ASCII + multibyte)
//! - insert_char_at_cursor, insert_text_at_cursor, delete_prev_char, delete_next_char

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

#[path = "../tests/support/mod.rs"]
mod support;

/// Generate a string of approximately `char_count` characters.
/// Mix of ASCII and multibyte (emoji, CJK) to stress byte/char offset logic.
fn generate_mixed_string(char_count: usize) -> String {
    // Pattern: 7 ASCII chars + 1 emoji (🦀) + 1 CJK (中) + 1 accented (é) = 10 chars
    let pattern = "abcdefg🦀中é";
    let pattern_chars: usize = pattern.chars().count(); // 10
    let repeats = char_count / pattern_chars;
    let remainder = char_count % pattern_chars;
    let mut s = String::with_capacity(repeats * pattern.len() + remainder);
    for _ in 0..repeats {
        s.push_str(pattern);
    }
    // Fill remainder with ASCII
    for _ in 0..remainder {
        s.push('x');
    }
    s
}

/// Generate an ASCII-only string of `char_count` characters.
fn generate_ascii_string(char_count: usize) -> String {
    "a".repeat(char_count)
}

// =========================================================================
// cursor_byte_pos_at_char_index
// =========================================================================

fn bench_cursor_byte_pos(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor_byte_pos_at_char_index");

    for &len in &[1_000, 10_000, 100_000] {
        let mixed = generate_mixed_string(len);
        let ascii = generate_ascii_string(len);

        // Benchmark: look up position at the middle of the string
        let mid = len / 2;

        let mut app = support::make_app();
        app.input = mixed.clone();
        group.bench_with_input(BenchmarkId::new("mixed_mid", len), &mid, |b, &idx| {
            b.iter(|| app.cursor_byte_pos_at_char_index(idx));
        });

        app.input = ascii.clone();
        group.bench_with_input(BenchmarkId::new("ascii_mid", len), &mid, |b, &idx| {
            b.iter(|| app.cursor_byte_pos_at_char_index(idx));
        });

        // Benchmark: look up position at the end (worst case for linear scan)
        app.input = mixed;
        group.bench_with_input(BenchmarkId::new("mixed_end", len), &len, |b, &idx| {
            b.iter(|| app.cursor_byte_pos_at_char_index(idx));
        });
    }
    group.finish();
}

// =========================================================================
// insert / delete operations
// =========================================================================

fn bench_insert_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_char_at_cursor");

    for &len in &[1_000, 10_000, 100_000] {
        let base = generate_mixed_string(len);

        group.bench_with_input(BenchmarkId::new("mid", len), &base, |b, input| {
            let mut app = support::make_app();
            b.iter(|| {
                app.input.clone_from(input);
                app.input_cursor = len / 2;
                app.insert_char_at_cursor('Z');
            });
        });

        group.bench_with_input(BenchmarkId::new("end", len), &base, |b, input| {
            let mut app = support::make_app();
            b.iter(|| {
                app.input.clone_from(input);
                app.input_cursor = len;
                app.insert_char_at_cursor('Z');
            });
        });
    }
    group.finish();
}

fn bench_insert_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_text_at_cursor");
    let paste = "Hello 🌍 world! 日本語テスト text.";

    for &len in &[1_000, 10_000, 100_000] {
        let base = generate_mixed_string(len);

        group.bench_with_input(BenchmarkId::new("mid", len), &base, |b, input| {
            let mut app = support::make_app();
            b.iter(|| {
                app.input.clone_from(input);
                app.input_cursor = len / 2;
                app.insert_text_at_cursor(paste);
            });
        });
    }
    group.finish();
}

fn bench_delete_prev_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_prev_char");

    for &len in &[1_000, 10_000, 100_000] {
        let base = generate_mixed_string(len);

        group.bench_with_input(BenchmarkId::new("mid", len), &base, |b, input| {
            let mut app = support::make_app();
            b.iter(|| {
                app.input.clone_from(input);
                app.input_cursor = len / 2;
                app.delete_prev_char();
            });
        });
    }
    group.finish();
}

fn bench_delete_next_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_next_char");

    for &len in &[1_000, 10_000, 100_000] {
        let base = generate_mixed_string(len);

        group.bench_with_input(BenchmarkId::new("mid", len), &base, |b, input| {
            let mut app = support::make_app();
            b.iter(|| {
                app.input.clone_from(input);
                app.input_cursor = len / 2;
                app.delete_next_char();
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cursor_byte_pos,
    bench_insert_char,
    bench_insert_text,
    bench_delete_prev_char,
    bench_delete_next_char
);
criterion_main!(benches);
