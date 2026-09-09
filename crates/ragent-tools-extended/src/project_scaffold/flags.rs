//! Scaffold flag types, parsing, validation, and error enum.
//!
//! Pure logic only — no filesystem access. Covers FR-002 (empty-directory
//! guard decision), FR-003 (required flag validation), and FR-009 (mutual
//! exclusion of hosting flags) for spec `newproj`.
//!
//! Directory emptiness is decided here on a [`FilePathEntry`] list so the
//! decision layer stays testable without I/O; the caller gathers entries.

use std::fmt;

/// Supported scaffold languages (FR-017 registry; FR-003 accepted values).
///
/// The set covers every application language in the codeindex scanner's
/// `SUPPORTED_LANGUAGES` list (`ragent-codeindex`): the compiled/scripted
/// application languages, plus data/config/DSL formats (TOML, YAML, JSON,
/// XML, ...), markup/styles (HTML, CSS, SCSS), shell interpreters, HDL
/// (Verilog, VHDL), and build-system formats (CMake, Gradle, Maven, Nix,
/// HCL). Application languages get full four-app-type hello-world recipes;
/// data/DSL formats get a cmdline+library stub recipe (notes manifest plus
/// a sample source), so every scanned codebase language has a scaffold.
/// Dialect aliases in the scanner id list (`tsx`, `jsx`, `c_header`,
/// `cpp_header`) map to their parent language via [`match_language_value`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Rust (`cargo`-based projects).
    Rust,
    /// Python 3 (`python3`-run projects).
    Python,
    /// Go (`go run .`-based projects).
    Go,
    /// TypeScript (`npm`-based projects).
    TypeScript,
    /// JavaScript (`node`-based projects).
    JavaScript,
    /// C (`cc`-based projects).
    C,
    /// C++ (`c++`-based projects).
    Cpp,
    /// Java (`javac`-based projects).
    Java,
    /// Kotlin (`kotlinc`-based projects).
    Kotlin,
    /// Ruby (`ruby`-based projects).
    Ruby,
    /// Swift (`swift`-based projects).
    Swift,
    /// C# (`dotnet`-based projects).
    CSharp,
    /// Lua (`lua`-based projects).
    Lua,
    /// Zig (`zig`-based projects).
    Zig,
    /// Nim (`nim`-based projects).
    Nim,
    /// Elixir (`mix`-based projects).
    Elixir,
    /// Erlang (`erl`-based projects).
    Erlang,
    /// Haskell (`ghc`/`cabal`-based projects).
    Haskell,
    /// OCaml (`ocaml`/`dune`-based projects).
    Ocaml,
    /// R (`Rscript`-based projects).
    R,
    /// Dart (`dart`-based projects).
    Dart,
    /// PHP (`php`-based projects).
    Php,
    /// Perl (`perl`-based projects).
    Perl,
    /// Shell (POSIX/Bash scripts run via `sh`/`bash`).
    Shell,
    /// Zsh scripts.
    Zsh,
    /// Fish scripts.
    Fish,
    /// TOML configuration/data files.
    Toml,
    /// YAML configuration/data files.
    Yaml,
    /// JSON data files.
    Json,
    /// XML documents.
    Xml,
    /// HTML documents (`index.html` entry page).
    Html,
    /// CSS stylesheets.
    Css,
    /// SCSS stylesheets.
    Scss,
    /// SQL scripts.
    Sql,
    /// Markdown documents.
    Markdown,
    /// Protocol Buffers schema files.
    Protobuf,
    /// Verilog HDL sources.
    Verilog,
    /// VHDL HDL sources.
    Vhdl,
    /// Terraform HCL infrastructure definitions.
    Terraform,
    /// OpenSCAD 3D-model sources.
    OpenScad,
    /// CMake build definitions.
    Cmake,
    /// Gradle Groovy-DSL build definitions.
    Gradle,
    /// Gradle Kotlin-DSL build definitions.
    GradleKts,
    /// Maven `pom.xml` build definitions.
    Maven,
    /// Nix expressions.
    Nix,
    /// HashiCorp HCL configurations.
    Hcl,
}

impl Language {
    /// Canonical flag value for this language.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Go => "go",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Ruby => "ruby",
            Self::Swift => "swift",
            Self::CSharp => "csharp",
            Self::Lua => "lua",
            Self::Zig => "zig",
            Self::Nim => "nim",
            Self::Elixir => "elixir",
            Self::Erlang => "erlang",
            Self::Haskell => "haskell",
            Self::Ocaml => "ocaml",
            Self::R => "r",
            Self::Dart => "dart",
            Self::Php => "php",
            Self::Perl => "perl",
            Self::Shell => "shell",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Css => "css",
            Self::Scss => "scss",
            Self::Sql => "sql",
            Self::Markdown => "markdown",
            Self::Protobuf => "protobuf",
            Self::Verilog => "verilog",
            Self::Vhdl => "vhdl",
            Self::Terraform => "terraform",
            Self::OpenScad => "openscad",
            Self::Cmake => "cmake",
            Self::Gradle => "gradle",
            Self::GradleKts => "gradle_kts",
            Self::Maven => "maven",
            Self::Nix => "nix",
            Self::Hcl => "hcl",
        }
    }

    /// All canonical values, in registry order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Rust,
            Self::Python,
            Self::Go,
            Self::TypeScript,
            Self::JavaScript,
            Self::C,
            Self::Cpp,
            Self::Java,
            Self::Kotlin,
            Self::Ruby,
            Self::Swift,
            Self::CSharp,
            Self::Lua,
            Self::Zig,
            Self::Nim,
            Self::Elixir,
            Self::Erlang,
            Self::Haskell,
            Self::Ocaml,
            Self::R,
            Self::Dart,
            Self::Php,
            Self::Perl,
            Self::Shell,
            Self::Zsh,
            Self::Fish,
            Self::Toml,
            Self::Yaml,
            Self::Json,
            Self::Xml,
            Self::Html,
            Self::Css,
            Self::Scss,
            Self::Sql,
            Self::Markdown,
            Self::Protobuf,
            Self::Verilog,
            Self::Vhdl,
            Self::Terraform,
            Self::OpenScad,
            Self::Cmake,
            Self::Gradle,
            Self::GradleKts,
            Self::Maven,
            Self::Nix,
            Self::Hcl,
        ]
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Application types (FR-006; FR-003 accepted values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppType {
    /// Library layout: no binary entrypoint.
    Library,
    /// Console-entry layout.
    Cmdline,
    /// Terminal-UI starter.
    Tui,
    /// GUI starter.
    Gui,
}

impl AppType {
    /// Canonical flag value for this app type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Cmdline => "cmdline",
            Self::Tui => "tui",
            Self::Gui => "gui",
        }
    }

    /// All canonical values, in registry order.
    pub fn all() -> &'static [Self] {
        &[Self::Library, Self::Cmdline, Self::Tui, Self::Gui]
    }
}

impl fmt::Display for AppType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Hosting target for the remote-init flow (FR-008/FR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostingTarget {
    /// GitHub remote creation + push.
    GitHub,
    /// GitLab remote creation + push.
    GitLab,
}

/// A fully validated scaffold request.
///
/// Construction goes through [`parse_flags`] (parsing + validation); direct
/// construction is impossible because all fields are private and every
/// variant is produced by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldRequest {
    language: Language,
    app_type: AppType,
    stack: Option<String>,
    hosting: Option<HostingTarget>,
    is_help: bool,
}

impl ScaffoldRequest {
    /// Selected scaffold language.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Selected application type.
    pub fn app_type(&self) -> AppType {
        self.app_type
    }

    /// Optional stack value (raw string; overlay matching happens later).
    pub fn stack(&self) -> Option<&str> {
        self.stack.as_deref()
    }

    /// Hosting target if any hosting flag was supplied.
    pub fn hosting(&self) -> Option<HostingTarget> {
        self.hosting
    }

    /// True when the invocation was `/new help` (help surface request).
    pub fn is_help(&self) -> bool {
        self.is_help
    }
}

/// Errors produced while parsing and validating a `/new` invocation and while
/// emitting scaffold files.
///
/// Every variant carries enough context to render the FR-003 usage message,
/// the FR-009 conflict message, or the FR-016 emission failure report without
/// re-reading the raw arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaffoldError {
    /// Directory is not empty (FR-002). Carries the offending entry names,
    /// excluding the ragent artifact allowlist (`.ragent/`, `log/`, `target/`).
    DirectoryNotEmpty(Vec<String>),
    /// `--language` missing (FR-003).
    MissingLanguage,
    /// `--type` missing (FR-003).
    MissingAppType,
    /// Unknown `--language` value (FR-003).
    UnknownLanguage(String),
    /// Unknown `--type` value (FR-003).
    UnknownAppType(String),
    /// Unknown flag (FR-003 usage surface).
    UnknownFlag(String),
    /// Stray positional argument that is neither a flag nor `help` (FR-003).
    UnexpectedArgument(String),
    /// Both `--github` and `--gitlab` supplied (FR-009).
    HostingConflict,
    /// The target directory exists but cannot be inspected (not a directory,
    /// or a read error). The FR-002 guard refuses rather than scaffold into
    /// something it cannot verify as empty.
    TargetUnreadable(String),
    /// Emitting a scaffold file failed (FR-016 emitter). Carries the file path
    /// and the OS error message; the scaffold aborts at that file.
    EmissionFailed {
        /// Scaffold-relative path of the file that failed to emit.
        path: String,
        /// Underlying OS error message.
        source: String,
    },
}

impl fmt::Display for ScaffoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryNotEmpty(entries) => {
                write!(
                    f,
                    "directory is not empty; refusing to scaffold. Offending entries: {}",
                    entries.join(", ")
                )
            }
            Self::MissingLanguage => {
                write!(f, "missing required flag --language")
            }
            Self::MissingAppType => {
                write!(f, "missing required flag --type")
            }
            Self::UnknownLanguage(value) => {
                write!(
                    f,
                    "unknown --language value '{value}' (expected one of: {})",
                    language_value_list()
                )
            }
            Self::UnknownAppType(value) => {
                write!(
                    f,
                    "unknown --type value '{value}' (expected one of: {})",
                    app_type_value_list()
                )
            }
            Self::UnknownFlag(flag) => {
                write!(f, "unknown flag '{flag}'")
            }
            Self::UnexpectedArgument(arg) => {
                write!(
                    f,
                    "unexpected argument '{arg}' (only 'help' is accepted as a positional word)"
                )
            }
            Self::HostingConflict => {
                write!(
                    f,
                    "--github and --gitlab are mutually exclusive; supply at most one"
                )
            }
            Self::TargetUnreadable(detail) => {
                write!(
                    f,
                    "cannot read target directory ({detail}); refusing to scaffold"
                )
            }
            Self::EmissionFailed { path, source } => {
                write!(f, "failed to emit scaffold file '{path}': {source}")
            }
        }
    }
}

impl std::error::Error for ScaffoldError {}

/// Comma-separated canonical `--language` values (FR-003 usage text).
pub fn language_value_list() -> String {
    Language::all()
        .iter()
        .map(|l| l.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Comma-separated canonical `--type` values (FR-003 usage text).
pub fn app_type_value_list() -> String {
    AppType::all()
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ragent-owned artifacts ignored by the FR-002 empty-directory guard.
pub const ARTIFACT_ALLOWLIST: [&str; 3] = [".ragent", "log", "target"];

/// A single filesystem entry considered by the emptiness decision.
///
/// The caller (I/O layer) gathers these; this module only decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePathEntry {
    /// Entry name as it appears in the directory (no parent path).
    pub name: String,
    /// True when the entry is a directory.
    pub is_dir: bool,
}

/// Decide whether the target directory is scaffoldable (FR-002).
///
/// Returns `Ok(())` when the directory is empty or contains only ragent
/// artifacts; otherwise `Err` listing every non-allowlisted entry name
/// (directories suffixed with `/` for readability).
///
/// # Errors
///
/// [`ScaffoldError::DirectoryNotEmpty`] when at least one non-allowlisted
/// entry is present.
pub fn validate_target_directory(entries: &[FilePathEntry]) -> Result<(), ScaffoldError> {
    let offending: Vec<String> = entries
        .iter()
        .filter(|e| !ARTIFACT_ALLOWLIST.contains(&e.name.as_str()))
        .map(|e| {
            if e.is_dir {
                format!("{}/", e.name)
            } else {
                e.name.clone()
            }
        })
        .collect();
    if offending.is_empty() {
        Ok(())
    } else {
        Err(ScaffoldError::DirectoryNotEmpty(offending))
    }
}

/// True when the argument list is a help-surface request (`/new` bare or
/// `/new help`). Bare `/new` shows the same usage surface per FR-012.
///
/// Note: bare `/new` is treated as help at this layer, which keeps flag
/// validation (FR-003) from rejecting it. The command surface distinguishes
/// the two renderings when needed.
pub fn is_help(args: &[&str]) -> bool {
    args.is_empty() || args == ["help"]
}

/// Parse and validate `/new` arguments into a [`ScaffoldRequest`].
///
/// `args` are the tokens after the `/new` verb, e.g.
/// `["--language", "rust", "--type", "cmdline", "--github"]`.
///
/// # Errors
///
/// Returns the first validation failure in argument order:
/// unknown flag, unknown `--language`/`--type` value, duplicate flag,
/// missing flag value, missing required flags (FR-003), stray positional
/// argument, or the FR-009 hosting conflict. Validation happens before any
/// filesystem access by contract — the caller must not have mutated anything
/// before calling this function.
pub fn parse_flags(args: &[&str]) -> Result<ScaffoldRequest, ScaffoldError> {
    if is_help(args) {
        return Ok(ScaffoldRequest {
            language: Language::Rust,
            app_type: AppType::Cmdline,
            stack: None,
            hosting: None,
            is_help: true,
        });
    }

    let mut language: Option<Language> = None;
    let mut app_type: Option<AppType> = None;
    let mut stack: Option<String> = None;
    let mut github = false;
    let mut gitlab = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        match arg {
            "--language" => {
                let value = next_value(args, &mut i, arg)?;
                if language.is_some() {
                    return Err(ScaffoldError::UnknownLanguage(format!(
                        "--language given twice ('{value}')"
                    )));
                }
                language = Some(match_language_value(value)?);
            }
            "--type" => {
                let value = next_value(args, &mut i, arg)?;
                if app_type.is_some() {
                    return Err(ScaffoldError::UnknownAppType(format!(
                        "--type given twice ('{value}')"
                    )));
                }
                app_type = Some(match_app_type_value(value)?);
            }
            "--stack" => {
                let value = next_value(args, &mut i, arg)?;
                if stack.is_some() {
                    return Err(ScaffoldError::UnknownFlag(format!(
                        "--stack given twice ('{value}')"
                    )));
                }
                stack = Some(value.to_owned());
            }
            "--github" => {
                if github {
                    return Err(ScaffoldError::UnknownFlag(
                        "--github given twice".to_owned(),
                    ));
                }
                github = true;
            }
            "--gitlab" => {
                if gitlab {
                    return Err(ScaffoldError::UnknownFlag(
                        "--gitlab given twice".to_owned(),
                    ));
                }
                gitlab = true;
            }
            "help" => {
                return Err(ScaffoldError::UnexpectedArgument(format!(
                    "'{arg}' cannot be combined with flags; use '/new' or '/new help' alone"
                )));
            }
            other if other.starts_with("--") => {
                return Err(ScaffoldError::UnknownFlag(other.to_owned()));
            }
            other => {
                return Err(ScaffoldError::UnexpectedArgument(other.to_owned()));
            }
        }
        i += 1;
    }

    if github && gitlab {
        return Err(ScaffoldError::HostingConflict);
    }

    // FR-003: required flags must be present and valid before any work.
    let language = language.ok_or(ScaffoldError::MissingLanguage)?;
    let app_type = app_type.ok_or(ScaffoldError::MissingAppType)?;

    Ok(ScaffoldRequest {
        language,
        app_type,
        stack,
        hosting: if github {
            Some(HostingTarget::GitHub)
        } else if gitlab {
            Some(HostingTarget::GitLab)
        } else {
            None
        },
        is_help: false,
    })
}

/// Consume the value token following a flag, erroring when absent.
fn next_value<'a>(
    args: &'a [&'a str],
    i: &mut usize,
    flag: &str,
) -> Result<&'a str, ScaffoldError> {
    *i += 1;
    match args.get(*i) {
        Some(value) => Ok(value),
        None => Err(ScaffoldError::UnknownFlag(format!(
            "{flag} requires a value"
        ))),
    }
}

/// Map a `--language` value to [`Language`] (FR-003 accepted values).
fn match_language_value(value: &str) -> Result<Language, ScaffoldError> {
    match value.to_ascii_lowercase().as_str() {
        "rust" => Ok(Language::Rust),
        "python" => Ok(Language::Python),
        "go" => Ok(Language::Go),
        "typescript" | "ts" | "tsx" => Ok(Language::TypeScript),
        "javascript" | "js" | "jsx" => Ok(Language::JavaScript),
        "c" | "c_header" => Ok(Language::C),
        "cpp" | "c++" | "cpp_header" => Ok(Language::Cpp),
        "java" => Ok(Language::Java),
        "kotlin" | "kt" => Ok(Language::Kotlin),
        "ruby" | "rb" => Ok(Language::Ruby),
        "swift" => Ok(Language::Swift),
        "csharp" | "cs" | "c#" => Ok(Language::CSharp),
        "lua" => Ok(Language::Lua),
        "zig" => Ok(Language::Zig),
        "nim" => Ok(Language::Nim),
        "elixir" | "ex" | "exs" => Ok(Language::Elixir),
        "erlang" | "erl" => Ok(Language::Erlang),
        "haskell" | "hs" => Ok(Language::Haskell),
        "ocaml" | "ml" => Ok(Language::Ocaml),
        "r" => Ok(Language::R),
        "dart" => Ok(Language::Dart),
        "php" => Ok(Language::Php),
        "perl" => Ok(Language::Perl),
        "shell" | "sh" | "bash" => Ok(Language::Shell),
        "zsh" => Ok(Language::Zsh),
        "fish" => Ok(Language::Fish),
        "toml" => Ok(Language::Toml),
        "yaml" | "yml" => Ok(Language::Yaml),
        "json" => Ok(Language::Json),
        "xml" => Ok(Language::Xml),
        "html" => Ok(Language::Html),
        "css" => Ok(Language::Css),
        "scss" => Ok(Language::Scss),
        "sql" => Ok(Language::Sql),
        "markdown" => Ok(Language::Markdown),
        "protobuf" => Ok(Language::Protobuf),
        "verilog" | "sv" => Ok(Language::Verilog),
        "vhdl" | "vhd" => Ok(Language::Vhdl),
        "terraform" | "tf" => Ok(Language::Terraform),
        "openscad" | "scad" => Ok(Language::OpenScad),
        "cmake" => Ok(Language::Cmake),
        "gradle" => Ok(Language::Gradle),
        "gradle_kts" | "kts" => Ok(Language::GradleKts),
        "maven" => Ok(Language::Maven),
        "nix" => Ok(Language::Nix),
        "hcl" => Ok(Language::Hcl),
        other => Err(ScaffoldError::UnknownLanguage(other.to_owned())),
    }
}

/// Map a `--type` value to [`AppType`] (FR-003 accepted values).
fn match_app_type_value(value: &str) -> Result<AppType, ScaffoldError> {
    match value.to_ascii_lowercase().as_str() {
        "library" => Ok(AppType::Library),
        "cmdline" => Ok(AppType::Cmdline),
        "tui" => Ok(AppType::Tui),
        "gui" => Ok(AppType::Gui),
        other => Err(ScaffoldError::UnknownAppType(other.to_owned())),
    }
}
