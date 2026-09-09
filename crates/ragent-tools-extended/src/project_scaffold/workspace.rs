//! ragent workspace initialisation for scaffolded projects (FR-004).
//!
//! Plans the fixed ragent workspace layer every `/new` scaffold receives on
//! top of the language recipes:
//!
//! - `.ragent/` (agents + config placeholders),
//! - `specs/` (spec directory placeholder),
//! - `log/` (runtime logs; gitignored, created as an empty directory),
//! - `.gitignore` (pre-populated from the language recipe + ragent artifacts),
//! - `AGENTS.md` (minimal project-guidelines stub).
//!
//! Pure planner: returns the artifact set (path + content) and the directory
//! list; the file emitter (T-007) performs the actual writes and applies the
//! FR-016 no-silent-overwrite policy there. No filesystem access in this
//! module. The `.gitignore` is rendered by the language recipe so the
//! registry stays the single source of truth (FR-017).

use super::recipes::LanguageRecipe;

/// One planned file: path relative to the scaffold root + full content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceArtifact {
    /// Path relative to the scaffold root, forward slashes, no leading `/`.
    pub path: &'static str,
    /// Full file content (UTF-8, LF line endings).
    pub content: String,
}

/// Directories the workspace initialiser creates (FR-004 layout).
///
/// The emitter creates these before writing artifacts; `log/` stays empty
/// and is gitignored via the generated `.gitignore`.
const WORKSPACE_DIRS: [&str; 3] = [".ragent/agents", "specs", "log"];

/// Placeholder for `.ragent/agents/README.md`.
const AGENTS_DIR_PLACEHOLDER: &str = "Custom ragent agent definitions live here (OASF JSON or \
     Markdown profiles). See the ragent documentation for the agent schema.\n";

/// Placeholder for `specs/README.md`.
const SPECS_DIR_PLACEHOLDER: &str = "Project specifications live in this directory. Manage them \
     with the ragent /spec slash commands.\n";

/// Directory list the emitter should create (FR-004).
pub fn workspace_dirs() -> &'static [&'static str] {
    &WORKSPACE_DIRS
}

/// Plan the FR-004 workspace artifact set for a scaffold.
///
/// Returns the files to emit (path + content) for `project_name` using the
/// language `recipe` for `.gitignore` and the `AGENTS.md` command stub. The
/// caller emits them via the T-007 no-silent-overwrite emitter after
/// creating [`workspace_dirs`].
pub fn workspace_artifacts(recipe: &LanguageRecipe, project_name: &str) -> Vec<WorkspaceArtifact> {
    vec![
        WorkspaceArtifact {
            path: ".gitignore",
            content: recipe.render_gitignore(),
        },
        WorkspaceArtifact {
            path: "AGENTS.md",
            content: agents_md_stub(recipe, project_name),
        },
        WorkspaceArtifact {
            path: ".ragent/config.json",
            content: config_placeholder(project_name),
        },
        WorkspaceArtifact {
            path: ".ragent/agents/README.md",
            content: AGENTS_DIR_PLACEHOLDER.to_owned(),
        },
        WorkspaceArtifact {
            path: "specs/README.md",
            content: SPECS_DIR_PLACEHOLDER.to_owned(),
        },
    ]
}

/// Render the minimal `AGENTS.md` project-guidelines stub (FR-004).
fn agents_md_stub(recipe: &LanguageRecipe, project_name: &str) -> String {
    format!(
        "# {project_name} - Agent Guidelines\n\
         \n\
         Guidelines for the ragent agent working in this repository.\n\
         Replace this stub with project-specific conventions as they emerge.\n\
         \n\
         ## Project\n\
         \n\
         - Language: {language}\n\
         \n\
         ## Build and Run\n\
         \n\
         - Run: `{run}`\n\
         - Test: `{test}`\n",
        project_name = project_name,
        language = recipe.language,
        run = recipe.run_command,
        test = recipe.test_command,
    )
}

/// Render the `.ragent/config.json` placeholder (valid JSON, not auto-loaded).
fn config_placeholder(project_name: &str) -> String {
    format!(
        "{{\n  \"project\": \"{project_name}\",\n  \"note\": \"ragent configuration placeholder \
         - replace with project settings\"\n}}\n"
    )
}
