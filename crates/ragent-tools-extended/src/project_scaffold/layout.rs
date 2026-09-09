//! App-type layout mapping (FR-006).
//!
//! Composes a [`LanguageRecipe`] with an [`AppType`] and a project slug into
//! the concrete code-artifact plan (manifest + hello-world source) that the
//! emitter milestone writes. Pure functions — no filesystem access.
//!
//! FR-006 semantics:
//!
//! - `library` — library layout: no binary entrypoint; an exported module
//!   with a public function (`src/lib.rs`, `src/{name}/__init__.py`,
//!   `{name}.go`, `src/index.ts`);
//! - `cmdline` — console-entry layout;
//! - `tui` — terminal-UI starter;
//! - `gui` — GUI starter appropriate to the language.

use super::flags::AppType;
use super::recipes::LanguageRecipe;

/// One planned code artifact: rendered path plus rendered content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Rendered path relative to the scaffold root (forward slashes).
    pub path: String,
    /// Rendered file content (UTF-8, LF line endings).
    pub content: String,
}

/// The code-artifact layout for one (language, app type) pair (FR-006).
///
/// Files are ordered: the language manifest first, then the app-type
/// hello-world source(s). Application-language recipes define a source for
/// every app type, so their planned layout carries the manifest plus exactly
/// one source. Data/DSL format recipes (TOML, YAML, SQL, build-system
/// definitions, ...) define only `cmdline` and `library` sample sources;
/// for the omitted `tui` / `gui` types the layout degrades to manifest-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppLayout {
    /// App type this layout was planned for.
    pub app_type: AppType,
    /// Planned files: manifest first, then the app-type source.
    pub files: Vec<PlannedFile>,
}

impl AppLayout {
    /// True for library layouts (FR-006: no binary entrypoint).
    pub fn is_library(&self) -> bool {
        self.app_type == AppType::Library
    }
}

/// Plan the FR-006 code-artifact layout for `app_type` in `recipe`.
///
/// The manifest is planned first (with `{name}` substituted by
/// `project_name`), followed by the app-type hello-world source with
/// `{name}` substituted in both path and content. Infallible: when a recipe
/// defines no source for `app_type` (data/DSL formats omit `tui` and `gui`)
/// the layout degrades to manifest-only.
pub fn plan_app_layout(
    recipe: &LanguageRecipe,
    app_type: AppType,
    project_name: &str,
) -> AppLayout {
    let mut files = vec![PlannedFile {
        path: recipe.manifest_path.to_owned(),
        content: recipe.render_manifest(project_name),
    }];
    if let Some(source) = recipe.source_for(app_type) {
        files.push(PlannedFile {
            path: recipe.render_source_path(source, project_name),
            content: recipe.render_source_content(source, project_name),
        });
    }
    AppLayout { app_type, files }
}
