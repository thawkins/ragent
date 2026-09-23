//! Stack overlay system (FR-007, T-004).
//!
//! Layers an optional `--stack` value on top of the FR-005/FR-006 base
//! layout: the framework dependency is added to the language manifest and
//! the hello-world source is overlain with a minimal framework-specific
//! binary starter (e.g. an HTTP route handler for `axum`). For a binary app
//! type the base source's `main` entry point is replaced by the starter's own
//! entry point, so the overlaid project builds and runs as generated; any
//! non-`main` base content (a library body) is retained above the starter.
//!
//! Known stacks are defined in [`STACK_RECIPES`], scoped per language so
//! `--stack axum` only applies to Rust projects. Unknown stacks resolve to
//! [`StackOverlay::Unknown`]; the caller warns and continues with the base
//! layout (FR-007: "warn and continue").
//!
//! Pure logic — no filesystem access. [`apply_stack_overlay`] mutates an
//! existing [`AppLayout`] in place so the emitted file count and FR-016
//! semantics stay identical to the base layout.

use super::flags::Language;
use super::layout::AppLayout;
use super::recipes::LanguageRecipe;

/// Framework dependency + starter snippet for one known stack (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackRecipe {
    /// Canonical `--stack` value (lowercase, as matched case-insensitively).
    pub stack: &'static str,
    /// Language this recipe applies to; other languages never match.
    pub language: Language,
    /// Exact dependency line appended to the language manifest file.
    pub dependency: &'static str,
    /// Import/use block appended above the source body.
    pub import_snippet: &'static str,
    /// Framework-specific starter appended to the source body.
    pub starter_snippet: &'static str,
}

impl StackRecipe {
    /// The dependency line including its trailing newline, for manifest
    /// appends. TOML manifests (Rust) get a `[dependencies]` section header
    /// inserted before the first dependency line when the generated manifest
    /// does not already declare one.
    fn dependency_line(&self, manifest_content: &str) -> String {
        let mut line = self.dependency.to_owned();
        line.push('\n');
        if self.is_toml_manifest() && !manifest_content.contains("[dependencies]") {
            line.insert_str(0, "[dependencies]\n");
        }
        line
    }

    /// Whether the recipe's manifest is TOML (currently Rust's `Cargo.toml`),
    /// the only manifest format that needs a section header for dependency
    /// appends.
    fn is_toml_manifest(&self) -> bool {
        // Rust is the only stack-capable language with a TOML manifest.
        self.language == Language::Rust
    }

    /// Rendered overlay source: import block, then the framework starter.
    ///
    /// The base source's `main` entry point is dropped before the starter is
    /// appended. Every starter snippet is a complete binary entry point in its
    /// own right (it declares `fn main`), so keeping the base
    /// `println!("Hello, world!")` entry point would emit **two** `main`
    /// functions into a single file - a program that only compiles once the
    /// base function is deleted, and that never reaches the framework starter.
    /// Removing the base entry point keeps exactly one `main` (the framework
    /// one), so an overlaid command-line or TUI project builds and shows the
    /// expected output as generated. Non-`main` base content (e.g. a library
    /// body) is retained underneath the starter.
    fn overlay_source(&self, base_source: &str) -> String {
        let base_body = strip_entry_point(base_source);
        let mut content = String::with_capacity(
            self.import_snippet.len() + base_body.len() + self.starter_snippet.len(),
        );
        content.push_str(self.import_snippet);
        content.push_str(&base_body);
        if !base_body.is_empty() && !base_body.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(self.starter_snippet);
        content
    }
}

/// Remove the `main` entry-point function from a base scaffold source.
///
/// Used by [`StackRecipe::overlay_source`] so a framework starter that
/// declares its own `fn main` replaces the base entry point instead of
/// duplicating it. Returns the source unchanged when it declares no
/// `fn main` (e.g. a library body), and the source with that one function
/// removed when it does. The entry point is located by its signature (the
/// `main` identifier must be followed by `(`), and its body is removed by
/// brace-balanced scanning that ignores braces inside string literals and
/// comments, so neither a longer name (`fn main_helper`) nor an escaped brace
/// in a literal can end the block early.
fn strip_entry_point(base_source: &str) -> String {
    let Some(fn_start) = find_entry_point(base_source) else {
        return base_source.to_owned();
    };
    let Some(body_open) = base_source[fn_start..].find('{').map(|off| fn_start + off) else {
        return base_source.to_owned();
    };
    let Some(close) = matching_brace(base_source, body_open) else {
        return base_source.to_owned();
    };

    // Drop the function body plus the single newline that follows it, if any,
    // leaving the surrounding (non-entry-point) base content intact.
    let mut end = close + 1;
    if base_source[end..].starts_with('\n') {
        end += 1;
    }

    let mut stripped = String::with_capacity(base_source.len());
    stripped.push_str(&base_source[..fn_start]);
    stripped.push_str(&base_source[end..]);
    stripped
}

/// Locate the byte offset of the `fn` keyword of the `main` entry point.
///
/// The `main` identifier must be followed (after optional whitespace) by `(`,
/// so a longer name such as `fn main_helper` or `fn mainloop` is never
/// mistaken for the entry point.
fn find_entry_point(source: &str) -> Option<usize> {
    source.match_indices("fn main").find_map(|(start, _)| {
        let after = &source[start + "fn main".len()..];
        after.trim_start().starts_with('(').then_some(start)
    })
}

/// Return the offset of the `}` that closes the brace opened at `open`.
///
/// Braces inside `"..."` string literals and `//` / `/* ... */` comments are
/// ignored, so a literal such as `"{{ escaped }}"` cannot end the block early.
/// Returns `None` when the block is never closed.
fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    while i < bytes.len() {
        let byte = bytes[i];
        let next = bytes.get(i + 1).copied();
        if in_line_comment {
            in_line_comment = byte != b'\n';
        } else if in_block_comment {
            if byte == b'*' && next == Some(b'/') {
                in_block_comment = false;
                i += 1;
            }
        } else if in_string {
            if byte == b'\\' {
                i += 1; // skip the escaped byte
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'/' if next == Some(b'/') => {
                    in_line_comment = true;
                    i += 1;
                }
                b'/' if next == Some(b'*') => {
                    in_block_comment = true;
                    i += 1;
                }
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Known-stack registry (FR-007): one entry per supported framework.
///
/// `/new help` stack examples derive from this table (NFR-001), so adding a
/// stack here extends the whole surface.
pub static STACK_RECIPES: &[StackRecipe] = &[
    StackRecipe {
        stack: "axum",
        language: Language::Rust,
        dependency: "axum = \"0.7\"\ntokio = { version = \"1\", features = [\"macros\", \
                     \"rt-multi-thread\"] }",
        import_snippet: "use axum::routing::get;\nuse axum::Router;\n",
        starter_snippet: "async fn hello_route() -> &'static str {\n    \
                          \"Hello, world!\"\n}\n\n\
                          fn main() {\n    \
                          let runtime = tokio::runtime::Runtime::new()\
                          .expect(\"tokio runtime\");\n    \
                          runtime.block_on(async {\n        \
                          let app = Router::new().route(\"/\", get(hello_route));\n        \
                          let listener = tokio::net::TcpListener::bind(\"127.0.0.1:8080\")\
                          .await.expect(\"bind 127.0.0.1:8080\");\n        \
                          axum::serve(listener, app).await.expect(\"server\");\n    });\n}\n",
    },
    StackRecipe {
        stack: "warp",
        language: Language::Rust,
        dependency: "warp = \"0.3\"\ntokio = { version = \"1\", features = [\"macros\", \
                     \"rt-multi-thread\"] }",
        import_snippet: "use warp::Filter;\n",
        starter_snippet: "fn main() {\n    \
                          let runtime = tokio::runtime::Runtime::new().expect(\"tokio runtime\");\n    \
                          runtime.block_on(async {\n        \
                          let hello = warp::path::end()\n            \
                          .map(|| \"Hello, world!\");\n        \
                          warp::serve(hello).run(([127, 0, 0, 1], 8080)).await;\n    });\n}\n",
    },
    StackRecipe {
        stack: "raylib",
        language: Language::Rust,
        dependency: "raylib = \"5\"",
        import_snippet: "use raylib::prelude::*;\n",
        starter_snippet: "fn main() {\n    \
                          let (mut rl, thread) = raylib::init()\n        \
                          .size(640, 480)\n        \
                          .title(\"Hello, world!\")\n        \
                          .build();\n    \
                          while !rl.window_should_close() {\n        \
                          let mut d = rl.begin_drawing(&thread);\n        \
                          d.clear_background(Color::WHITE);\n        \
                          d.draw_text(\"Hello, world!\", 12, 12, 20, Color::BLACK);\n    }\n}\n",
    },
    StackRecipe {
        stack: "gtk4",
        language: Language::Rust,
        dependency: "gtk4 = \"0.9\"",
        import_snippet: "use gtk4::prelude::*;\nuse gtk4::Application;\n",
        starter_snippet: "fn main() {\n    \
                          let app = Application::builder()\n        \
                          .application_id(\"com.example.hello\")\n        \
                          .build();\n    \
                          app.connect_activate(|app| {\n        \
                          let window = gtk4::Window::new(Some(app));\n        \
                          window.set_title(Some(\"Hello, world!\"));\n        \
                          window.set_default_size(640, 480);\n        \
                          window.present();\n    });\n    \
                          app.run();\n}\n",
    },
    StackRecipe {
        stack: "ratatui",
        language: Language::Rust,
        dependency: "ratatui = \"0.29\"\ncrossterm = \"0.28\"",
        import_snippet: "use ratatui::{Terminal, backend::CrosstermBackend, \
                         widgets::Paragraph};\nuse \
                         ratatui::crossterm::event::{self, Event, KeyEventKind};\n",
        starter_snippet: "fn main() {\n    \
                          let mut terminal = \
                          Terminal::new(CrosstermBackend::new(std::io::stdout()))\n        \
                          .expect(\"failed to create terminal\");\n    \
                          loop {\n        \
                          terminal\n            \
                          .draw(|frame| {\n                \
                          frame.render_widget(\n                    \
                          Paragraph::new(\"Hello, world!\"),\n                    \
                          frame.area(),\n                );\n            })\n            \
                          .expect(\"failed to draw\");\n        \
                          if let Event::Key(key) = event::read().expect(\"event\") {\n            \
                          if key.kind == KeyEventKind::Press {\n                break;\n            }\n        \
                          }\n    }\n}\n",
    },
];

/// Resolve the requested stack value against the known-stack registry
/// (FR-007).
///
/// Matching is case-insensitive against the canonical stack value and scoped
/// to `language`; a value that only exists for another language resolves to
/// [`StackOverlay::Unknown`] (the caller warns and continues). `None` when no
/// `--stack` value was supplied.
pub fn resolve_stack_overlay(stack: Option<&str>, language: Language) -> StackOverlay {
    let Some(requested) = stack else {
        return StackOverlay::None;
    };
    let requested_lower = requested.to_ascii_lowercase();
    match STACK_RECIPES
        .iter()
        .find(|recipe| recipe.language == language && recipe.stack == requested_lower)
    {
        Some(recipe) => StackOverlay::Applied(recipe),
        None => StackOverlay::Unknown(requested.to_owned()),
    }
}

/// Outcome of resolving the `--stack` value (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackOverlay {
    /// No `--stack` value supplied: base layout unchanged.
    None,
    /// Known stack for this language: apply this recipe.
    Applied(&'static StackRecipe),
    /// Unknown stack (or known only for another language): caller warns and
    /// continues with the base layout.
    Unknown(String),
}

/// Apply a known [`StackRecipe`] to the planned base layout (FR-007).
///
/// Mutates `layout` in place:
///
/// - the dependency line is appended to the manifest file planned by
///   [`plan_app_layout`](super::layout::plan_app_layout) (matched by
///   `recipe.manifest_path`, never hardcoded);
/// - every non-manifest source file is replaced with the overlay source:
///   import block + base hello-world body + framework starter snippet,
///   keeping the base path and `{name}` rendering.
///
/// Infallible: layouts without a source file only receive the manifest
/// dependency append.
pub fn apply_stack_overlay(layout: &mut AppLayout, recipe: &LanguageRecipe, stack: &StackRecipe) {
    for file in &mut layout.files {
        if file.path == recipe.manifest_path {
            file.content.push_str(&stack.dependency_line(&file.content));
        } else {
            file.content = stack.overlay_source(&file.content);
        }
    }
}
