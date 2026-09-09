//! Per-language scaffold recipes (FR-005, FR-017).
//!
//! A [`LanguageRecipe`] bundles everything later milestones need to render a
//! runnable hello-world artifact set for one language:
//!
//! - the manifest file (e.g. `Cargo.toml`, `pyproject.toml`) and its content,
//! - the per-`AppType` source file (path + content) for hello-world output,
//! - the default run command and test command,
//! - gitignore content covering build artifacts plus ragent's `.ragent/`,
//!   `log/`, `target/` entries (FR-004).
//!
//! Pure data + pure functions: no filesystem access, no I/O. The renderer
//! milestone (T-003+) consumes [`recipe_for`] output; the `/new help` surface
//! derives its value lists from [`Language::all`] (registry single source).

use super::flags::{AppType, Language};

/// Canonical hello-world source for one language + app type.
///
/// Paths and contents are static template text; `{name}` placeholders are
/// substituted with the project slug at render time via
/// [`LanguageRecipe::render_source_path`] / [`render_source_content`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFile {
    /// Path template relative to the scaffold root (forward slashes).
    pub path: &'static str,
    /// Full file content template (UTF-8, LF line endings).
    pub content: &'static str,
}

/// Per-language scaffold recipe: everything needed to render the FR-004
/// layout and FR-005 hello-world set for one language.
#[derive(Debug, Clone, Copy)]
pub struct LanguageRecipe {
    /// Canonical language value (matches `--language` flag values).
    pub language: Language,
    /// Manifest file path relative to scaffold root (e.g. `Cargo.toml`).
    pub manifest_path: &'static str,
    /// Manifest content template. `{name}` becomes the project slug.
    pub manifest_template: &'static str,
    /// Hello-world source per app type.
    pub sources: &'static [(AppType, SourceFile)],
    /// Command used to run the project (info + README + run-once support).
    pub run_command: &'static str,
    /// Command used to run the language's default test runner.
    pub test_command: &'static str,
    /// Gitignore body for build artifacts (ragent entries appended at render).
    pub gitignore_body: &'static str,
}

impl LanguageRecipe {
    /// Hello-world source file for an app type, if the recipe defines one.
    pub fn source_for(&self, app_type: AppType) -> Option<&SourceFile> {
        self.sources
            .iter()
            .find(|(t, _)| *t == app_type)
            .map(|(_, file)| file)
    }

    /// Manifest content with `{name}` substituted by the project slug.
    pub fn render_manifest(&self, project_name: &str) -> String {
        self.manifest_template.replace("{name}", project_name)
    }

    /// Source path template with `{name}` substituted by the project slug.
    pub fn render_source_path(&self, file: &SourceFile, project_name: &str) -> String {
        file.path.replace("{name}", project_name)
    }

    /// Source content template with `{name}` substituted by the project slug.
    pub fn render_source_content(&self, file: &SourceFile, project_name: &str) -> String {
        file.content.replace("{name}", project_name)
    }

    /// Gitignore content: language artifacts plus ragent artifacts (FR-004).
    pub fn render_gitignore(&self) -> String {
        format!("{}\n.ragent/\nlog/\ntarget/\n", self.gitignore_body)
    }
}

/// Look up a language's recipe in the registry (FR-017).
pub fn recipe_for(language: Language) -> Option<&'static LanguageRecipe> {
    REGISTRY.iter().find(|recipe| recipe.language == language)
}

// ------------------------------------------------------------------- Rust ---

const RUST_MANIFEST: &str = "[package]\n\
name = \"{name}\"\n\
version = \"0.1.0\"\n\
edition = \"2024\"\n";

const RUST_MAIN_RS: &str = "fn main() {\n    println!(\"Hello, world!\");\n}\n";

const RUST_LIB_RS: &str = "/// Returns a greeting from the library.\n\
pub fn hello() -> String {\n\
    \"Hello, world!\".to_owned()\n\
}\n";

const RUST_TUI_MAIN: &str = "fn main() {\n\
    // TUI starter: terminal UI entry point.\n\
    println!(\"Hello, world! (tui starter)\");\n\
}\n";

const RUST_GUI_MAIN: &str = "fn main() {\n\
    // GUI starter: GUI framework entry point.\n\
    println!(\"Hello, world! (gui starter)\");\n\
}\n";

// ----------------------------------------------------------------- Python ---

const PYTHON_MANIFEST: &str = "[project]\n\
name = \"{name}\"\n\
version = \"0.1.0\"\n\
requires-python = \">=3.10\"\n";

const PYTHON_MAIN: &str = "def main() -> None:\n    print(\"Hello, world!\")\n\n\n\
if __name__ == \"__main__\":\n    main()\n";

const PYTHON_LIB: &str = "\"\"\"{name} library package.\"\"\"\n\n\ndef hello() -> str:\n\
    \"\"\"Return a greeting from the library.\"\"\"\n\
    return \"Hello, world!\"\n";

const PYTHON_TUI_MAIN: &str = "def main() -> None:\n\
    print(\"Hello, world! (tui starter)\")\n\n\n\
if __name__ == \"__main__\":\n    main()\n";

const PYTHON_GUI_MAIN: &str = "def main() -> None:\n\
    print(\"Hello, world! (gui starter)\")\n\n\n\
if __name__ == \"__main__\":\n    main()\n";

// --------------------------------------------------------------------- Go ---

const GO_MANIFEST: &str = "module {name}\n\ngo 1.24\n";

const GO_MAIN: &str = "package main\n\nimport \"fmt\"\n\n\
func main() {\n\tfmt.Println(\"Hello, world!\")\n}\n";

const GO_LIB: &str = "package {name}\n\n// Hello returns a greeting from the library.\n\
func Hello() string {\n\treturn \"Hello, world!\"\n}\n";

const GO_TUI_MAIN: &str = "package main\n\nimport \"fmt\"\n\n\
func main() {\n\tfmt.Println(\"Hello, world! (tui starter)\")\n}\n";

const GO_GUI_MAIN: &str = "package main\n\nimport \"fmt\"\n\n\
func main() {\n\tfmt.Println(\"Hello, world! (gui starter)\")\n}\n";

// ------------------------------------------------------------- TypeScript ---

const TS_MANIFEST: &str = "{\n  \"name\": \"{name}\",\n  \"version\": \"0.1.0\",\n\
  \"devDependencies\": {\n    \"typescript\": \"^5.0.0\"\n  }\n}\n";

const TS_MAIN: &str = "function main(): void {\n\
  console.log(\"Hello, world!\");\n}\n\nmain();\n";

const TS_LIB: &str = "export function hello(): string {\n  return \"Hello, world!\";\n}\n";

const TS_TUI_MAIN: &str = "function main(): void {\n\
  console.log(\"Hello, world! (tui starter)\");\n}\n\nmain();\n";

const TS_GUI_MAIN: &str = "function main(): void {\n\
  console.log(\"Hello, world! (gui starter)\");\n}\n\nmain();\n";

// ------------------------------------------------------------- JavaScript ---

const JS_MANIFEST: &str = "{\n  \"name\": \"{name}\",\n  \"version\": \"0.1.0\",\n\
  \"type\": \"commonjs\"\n}\n";

const JS_MAIN: &str = "function main() {\n  console.log(\"Hello, world!\");\n}\n\n\
main();\n";

const JS_LIB: &str = "function hello() {\n  return \"Hello, world!\";\n}\n\n\
module.exports = { hello };\n";

const JS_TUI_MAIN: &str = "function main() {\n\
  console.log(\"Hello, world! (tui starter)\");\n}\n\nmain();\n";

const JS_GUI_MAIN: &str = "function main() {\n\
  console.log(\"Hello, world! (gui starter)\");\n}\n\nmain();\n";

// --------------------------------------------------------------------- C ---

const C_MANIFEST: &str = "# {name} — C project. Build with cc or your build\n\
# system of choice (cmake, make).\n";

const C_MAIN: &str = "#include <stdio.h>\n\n\
int main(void) {\n    printf(\"Hello, world!\\n\");\n    return 0;\n}\n";

const C_LIB: &str = "/* Returns a greeting from the library. */\n\
const char *hello(void) {\n    return \"Hello, world!\";\n}\n";

const C_TUI_MAIN: &str = "#include <stdio.h>\n\n\
int main(void) {\n    /* TUI starter: terminal UI entry point. */\n    \
printf(\"Hello, world! (tui starter)\\n\");\n    return 0;\n}\n";

const C_GUI_MAIN: &str = "#include <stdio.h>\n\n\
int main(void) {\n    /* GUI starter: GUI framework entry point. */\n    \
printf(\"Hello, world! (gui starter)\\n\");\n    return 0;\n}\n";

// ------------------------------------------------------------------- C++ ---

const CPP_MANIFEST: &str = "# {name} — C++ project. Build with c++ or your\n\
# build system of choice (cmake, make).\n";

const CPP_MAIN: &str = "#include <iostream>\n\n\
int main() {\n    std::cout << \"Hello, world!\" << std::endl;\n    return 0;\n}\n";

const CPP_LIB: &str = "// Returns a greeting from the library.\n\
inline const char *hello() {\n    return \"Hello, world!\";\n}\n";

const CPP_TUI_MAIN: &str = "#include <iostream>\n\n\
int main() {\n    // TUI starter: terminal UI entry point.\n    \
std::cout << \"Hello, world! (tui starter)\" << std::endl;\n    return 0;\n}\n";

const CPP_GUI_MAIN: &str = "#include <iostream>\n\n\
int main() {\n    // GUI starter: GUI framework entry point.\n    \
std::cout << \"Hello, world! (gui starter)\" << std::endl;\n    return 0;\n}\n";

// ------------------------------------------------------------------- Java ---

const JAVA_MANIFEST: &str = "# {name} — Java project. Build with javac or your\n\
# build tool of choice (maven, gradle).\n";

const JAVA_MAIN: &str = "public class Main {\n    public static void main(String[] args) {\n    \
    System.out.println(\"Hello, world!\");\n    }\n}\n";

const JAVA_LIB: &str = "public class Hello {\n    public static String hello() {\n    \
    return \"Hello, world!\";\n    }\n}\n";

const JAVA_TUI_MAIN: &str = "public class Main {\n    public static void main(String[] args) {\n    \
    // TUI starter: terminal UI entry point.\n    \
    System.out.println(\"Hello, world! (tui starter)\");\n    }\n}\n";

const JAVA_GUI_MAIN: &str = "public class Main {\n    public static void main(String[] args) {\n    \
    // GUI starter: GUI framework entry point.\n    \
    System.out.println(\"Hello, world! (gui starter)\");\n    }\n}\n";

// ----------------------------------------------------------------- Kotlin ---

const KOTLIN_MANIFEST: &str = "# {name} — Kotlin project. Build with kotlinc or\n\
# your build tool of choice (gradle).\n";

const KOTLIN_MAIN: &str = "fun main() {\n    println(\"Hello, world!\")\n}\n";

const KOTLIN_LIB: &str = "// Returns a greeting from the library.\n\
fun hello(): String {\n    return \"Hello, world!\"\n}\n";

const KOTLIN_TUI_MAIN: &str = "fun main() {\n    \
// TUI starter: terminal UI entry point.\n    \
println(\"Hello, world! (tui starter)\")\n}\n";

const KOTLIN_GUI_MAIN: &str = "fun main() {\n    \
// GUI starter: GUI framework entry point.\n    \
println(\"Hello, world! (gui starter)\")\n}\n";

// ------------------------------------------------------------------- Ruby ---

const RUBY_MANIFEST: &str = "# {name} — Ruby project. Dependencies go in a\n\
# Gemfile; run with ruby.\n";

const RUBY_MAIN: &str = "def main\n  puts 'Hello, world!'\nend\n\nmain\n";

const RUBY_LIB: &str = "# Returns a greeting from the library.\n\
def hello\n  'Hello, world!'\nend\n";

const RUBY_TUI_MAIN: &str = "def main\n  # TUI starter: terminal UI entry point.\n  \
puts 'Hello, world! (tui starter)'\nend\n\nmain\n";

const RUBY_GUI_MAIN: &str = "def main\n  # GUI starter: GUI framework entry point.\n  \
puts 'Hello, world! (gui starter)'\nend\n\nmain\n";

// ------------------------------------------------------------------ Swift ---

const SWIFT_MANIFEST: &str = "// {name} — Swift project.\n";

const SWIFT_MAIN: &str = "func main() {\n    print(\"Hello, world!\")\n}\n\nmain()\n";

const SWIFT_LIB: &str = "// Returns a greeting from the library.\n\
func hello() -> String {\n    return \"Hello, world!\"\n}\n";

const SWIFT_TUI_MAIN: &str = "func main() {\n    \
// TUI starter: terminal UI entry point.\n    \
print(\"Hello, world! (tui starter)\")\n}\n\nmain()\n";

const SWIFT_GUI_MAIN: &str = "func main() {\n    \
// GUI starter: GUI framework entry point.\n    \
print(\"Hello, world! (gui starter)\")\n}\n\nmain()\n";

// ------------------------------------------------------------------- C# ---

const CSHARP_MANIFEST: &str = "# {name} — C# project. Build with dotnet or your\n\
# build tool of choice.\n";

const CSHARP_MAIN: &str = "class Program {\n    static void Main() {\n    \
    System.Console.WriteLine(\"Hello, world!\");\n    }\n}\n";

const CSHARP_LIB: &str = "public class Hello {\n    public static string Hello() {\n    \
    return \"Hello, world!\";\n    }\n}\n";

const CSHARP_TUI_MAIN: &str = "class Program {\n    static void Main() {\n    \
    // TUI starter: terminal UI entry point.\n    \
    System.Console.WriteLine(\"Hello, world! (tui starter)\");\n    }\n}\n";

const CSHARP_GUI_MAIN: &str = "class Program {\n    static void Main() {\n    \
    // GUI starter: GUI framework entry point.\n    \
    System.Console.WriteLine(\"Hello, world! (gui starter)\");\n    }\n}\n";

// -------------------------------------------------------------------- Lua ---

const LUA_MANIFEST: &str = "-- {name} — Lua project. Dependencies go in a\n\
-- rockspec; run with lua.\n";

const LUA_MAIN: &str = "local function main()\n  print(\"Hello, world!\")\nend\n\n\
main()\n";

const LUA_LIB: &str = "-- Returns a greeting from the library.\n\
local function hello()\n  return \"Hello, world!\"\nend\n\nreturn { hello = hello }\n";

const LUA_TUI_MAIN: &str = "local function main()\n  \
-- TUI starter: terminal UI entry point.\n  \
print(\"Hello, world! (tui starter)\")\nend\n\nmain()\n";

const LUA_GUI_MAIN: &str = "local function main()\n  \
-- GUI starter: GUI framework entry point.\n  \
print(\"Hello, world! (gui starter)\")\nend\n\nmain()\n";

// --------------------------------------------------------------------- Zig ---

const ZIG_MANIFEST: &str = "// {name} — Zig project. Build with `zig build`.\n";

const ZIG_MAIN: &str = "const std = @import(\"std\");\n\n\
pub fn main() !void {\n    std.debug.print(\"Hello, world!\\n\", .{});\n}\n";

const ZIG_LIB: &str = "/// Returns a greeting from the library.\n\
pub fn hello() []const u8 {\n    return \"Hello, world!\";\n}\n";

const ZIG_TUI_MAIN: &str = "const std = @import(\"std\");\n\n\
pub fn main() !void {\n    \
// TUI starter: terminal UI entry point.\n    \
std.debug.print(\"Hello, world! (tui starter)\\n\", .{});\n}\n";

const ZIG_GUI_MAIN: &str = "const std = @import(\"std\");\n\n\
pub fn main() !void {\n    \
// GUI starter: GUI framework entry point.\n    \
std.debug.print(\"Hello, world! (gui starter)\\n\", .{});\n}\n";

// --------------------------------------------------------------------- Nim ---

const NIM_MANIFEST: &str = "# {name} — Nim project. Build with `nim c`.\n";

const NIM_MAIN: &str = "proc main() =\n  echo \"Hello, world!\"\n\nmain()\n";

const NIM_LIB: &str = "# Returns a greeting from the library.\n\
proc hello*(): string =\n  \"Hello, world!\"\n";

const NIM_TUI_MAIN: &str = "proc main() =\n  \
# TUI starter: terminal UI entry point.\n  \
echo \"Hello, world! (tui starter)\"\n\nmain()\n";

const NIM_GUI_MAIN: &str = "proc main() =\n  \
# GUI starter: GUI framework entry point.\n  \
echo \"Hello, world! (gui starter)\"\n\nmain()\n";

// ----------------------------------------------------------------- Elixir ---

const ELIXIR_MANIFEST: &str = "# {name} — Elixir project. Scaffolding only:\n\
# run `mix new` for a full mix project layout.\n";

const ELIXIR_MAIN: &str = "defmodule Main do\n  def run do\n    \
IO.puts(\"Hello, world!\")\n  end\nend\n\nMain.run()\n";

const ELIXIR_LIB: &str = "defmodule Hello do\n  @spec hello() :: String.t()\n  \
def hello do\n    \"Hello, world!\"\n  end\nend\n";

const ELIXIR_TUI_MAIN: &str = "defmodule Main do\n  def run do\n    \
# TUI starter: terminal UI entry point.\n    \
IO.puts(\"Hello, world! (tui starter)\")\n  end\nend\n\nMain.run()\n";

const ELIXIR_GUI_MAIN: &str = "defmodule Main do\n  def run do\n    \
# GUI starter: GUI framework entry point.\n    \
IO.puts(\"Hello, world! (gui starter)\")\n  end\nend\n\nMain.run()\n";

// ----------------------------------------------------------------- Erlang ---

const ERLANG_MANIFEST: &str = "%% {name} — Erlang project. Build with rebar3\n\
%% or escript.\n";

const ERLANG_MAIN: &str = "main() ->\n    io:format(\"Hello, world!~n\").\n";

const ERLANG_LIB: &str = "%% Returns a greeting from the library.\n\
hello() ->\n    \"Hello, world!\".\n";

const ERLANG_TUI_MAIN: &str = "main() ->\n    %% TUI starter: terminal UI entry point.\n    \
io:format(\"Hello, world! (tui starter)~n\").\n";

const ERLANG_GUI_MAIN: &str = "main() ->\n    %% GUI starter: GUI framework entry point.\n    \
io:format(\"Hello, world! (gui starter)~n\").\n";

// ---------------------------------------------------------------- Haskell ---

const HASKELL_MANIFEST: &str = "-- {name} — Haskell project. Build with cabal\n\
-- or ghc.\n";

const HASKELL_MAIN: &str = "main :: IO ()\nmain = putStrLn \"Hello, world!\"\n";

const HASKELL_LIB: &str = "module Hello (hello) where\n\n\
-- | Returns a greeting from the library.\n\
hello :: String\nhello = \"Hello, world!\"\n";

const HASKELL_TUI_MAIN: &str = "main :: IO ()\nmain = do\n  \
-- TUI starter: terminal UI entry point.\n  \
putStrLn \"Hello, world! (tui starter)\"\n";

const HASKELL_GUI_MAIN: &str = "main :: IO ()\nmain = do\n  \
-- GUI starter: GUI framework entry point.\n  \
putStrLn \"Hello, world! (gui starter)\"\n";

// ------------------------------------------------------------------- OCaml ---

const OCAML_MANIFEST: &str = "(* {name} — OCaml project. Build with dune or\n\
 ocamlfind. *)\n";

const OCAML_MAIN: &str = "let () = print_endline \"Hello, world!\"\n";

const OCAML_LIB: &str = "(* Returns a greeting from the library. *)\n\
let hello () = \"Hello, world!\"\n";

const OCAML_TUI_MAIN: &str = "(* TUI starter: terminal UI entry point. *)\n\
let () = print_endline \"Hello, world! (tui starter)\"\n";

const OCAML_GUI_MAIN: &str = "(* GUI starter: GUI framework entry point. *)\n\
let () = print_endline \"Hello, world! (gui starter)\"\n";

// ---------------------------------------------------------------------- R ---

const R_MANIFEST: &str = "# {name} — R project. Dependencies go in a\n\
# DESCRIPTION file; run with Rscript.\n";

const R_MAIN: &str = "main <- function() {\n  cat(\"Hello, world!\\n\")\n}\n\nmain()\n";

const R_LIB: &str = "# Returns a greeting from the library.\n\
hello <- function() {\n  \"Hello, world!\"\n}\n";

const R_TUI_MAIN: &str = "main <- function() {\n  \
# TUI starter: terminal UI entry point.\n  \
cat(\"Hello, world! (tui starter)\\n\")\n}\n\nmain()\n";

const R_GUI_MAIN: &str = "main <- function() {\n  \
# GUI starter: GUI framework entry point.\n  \
cat(\"Hello, world! (gui starter)\\n\")\n}\n\nmain()\n";

// ------------------------------------------------------------------- Dart ---

const DART_MANIFEST: &str = "# {name} — Dart project. For a full package\n\
# layout, run `dart create`.\n";

const DART_MAIN: &str = "void main() {\n  print('Hello, world!');\n}\n";

const DART_LIB: &str = "// Returns a greeting from the library.\n\
String hello() {\n  return 'Hello, world!';\n}\n";

const DART_TUI_MAIN: &str = "void main() {\n  \
// TUI starter: terminal UI entry point.\n  \
print('Hello, world! (tui starter)');\n}\n";

const DART_GUI_MAIN: &str = "void main() {\n  \
// GUI starter: GUI framework entry point.\n  \
print('Hello, world! (gui starter)');\n}\n";

// -------------------------------------------------------------------- PHP ---

const PHP_MANIFEST: &str = "<?php\n// {name} - PHP project. Dependencies go in\n\
// composer.json; run with php.\n";

const PHP_MAIN: &str = "<?php\ndeclare(strict_types=1);\n\n\
function main(): void {\n    echo \"Hello, world!\\n\";\n}\n\nmain();\n";

const PHP_LIB: &str = "<?php\ndeclare(strict_types=1);\n\n\
/** Returns a greeting from the library. */\n\
function hello(): string {\n    return 'Hello, world!';\n}\n";

const PHP_TUI_MAIN: &str = "<?php\ndeclare(strict_types=1);\n\n\
function main(): void {\n    // TUI starter: terminal UI entry point.\n    \
echo \"Hello, world! (tui starter)\\n\";\n}\n\nmain();\n";

const PHP_GUI_MAIN: &str = "<?php\ndeclare(strict_types=1);\n\n\
function main(): void {\n    // GUI starter: GUI framework entry point.\n    \
echo \"Hello, world! (gui starter)\\n\";\n}\n\nmain();\n";

// ------------------------------------------------------------------- Perl ---

const PERL_MANIFEST: &str = "# {name} — Perl project. Dependencies go in a\n\
# cpanfile; run with perl.\n";

const PERL_MAIN: &str = "use strict;\nuse warnings;\n\n\
sub main {\n    print \"Hello, world!\\n\";\n}\n\nmain();\n";

const PERL_LIB: &str = "use strict;\nuse warnings;\n\n\
# Returns a greeting from the library.\n\
sub hello {\n    return 'Hello, world!';\n}\n\n1;\n";

const PERL_TUI_MAIN: &str = "use strict;\nuse warnings;\n\n\
sub main {\n    # TUI starter: terminal UI entry point.\n    \
print \"Hello, world! (tui starter)\\n\";\n}\n\nmain();\n";

const PERL_GUI_MAIN: &str = "use strict;\nuse warnings;\n\n\
sub main {\n    # GUI starter: GUI framework entry point.\n    \
print \"Hello, world! (gui starter)\\n\";\n}\n\nmain();\n";

// ------------------------------------------------------------------- Shell ---

const SHELL_MANIFEST: &str = "# {name} - shell project. Run with `sh` or `bash`.\n";

const SHELL_MAIN: &str = "#!/bin/sh\n\
# Console entry point.\n\
echo \"Hello, world!\"\n";

const SHELL_LIB: &str = "#!/bin/sh\n\
# Returns a greeting from the library.\n\
hello() {\n\
    echo \"Hello, world!\"\n\
}\n";

const SHELL_TUI_MAIN: &str = "#!/bin/sh\n\
# TUI starter: terminal UI entry point.\n\
echo \"Hello, world! (tui starter)\"\n";

const SHELL_GUI_MAIN: &str = "#!/bin/sh\n\
# GUI starter: GUI framework entry point.\n\
echo \"Hello, world! (gui starter)\"\n";

// --------------------------------------------------------------------- Zsh ---

const ZSH_MANIFEST: &str = "# {name} - zsh project. Run with `zsh`.\n";

const ZSH_MAIN: &str = "#!/bin/zsh\n\
# Console entry point.\n\
echo \"Hello, world!\"\n";

const ZSH_LIB: &str = "#!/bin/zsh\n\
# Returns a greeting from the library.\n\
hello() {\n\
    echo \"Hello, world!\"\n\
}\n";

const ZSH_TUI_MAIN: &str = "#!/bin/zsh\n\
# TUI starter: terminal UI entry point.\n\
echo \"Hello, world! (tui starter)\"\n";

const ZSH_GUI_MAIN: &str = "#!/bin/zsh\n\
# GUI starter: GUI framework entry point.\n\
echo \"Hello, world! (gui starter)\"\n";

// -------------------------------------------------------------------- Fish ---

const FISH_MANIFEST: &str = "# {name} - fish project. Run with `fish`.\n";

const FISH_MAIN: &str = "#!/usr/bin/env fish\n\
# Console entry point.\n\
echo \"Hello, world!\"\n";

const FISH_LIB: &str = "#!/usr/bin/env fish\n\
# Returns a greeting from the library.\n\
function hello\n\
    echo \"Hello, world!\"\n\
end\n";

const FISH_TUI_MAIN: &str = "#!/usr/bin/env fish\n\
# TUI starter: terminal UI entry point.\n\
echo \"Hello, world! (tui starter)\"\n";

const FISH_GUI_MAIN: &str = "#!/usr/bin/env fish\n\
# GUI starter: GUI framework entry point.\n\
echo \"Hello, world! (gui starter)\"\n";

// -------------------------------------------------------------------- TOML ---

const TOML_MANIFEST: &str = "# {name} - TOML configuration sample.\n\
# Edit this file or replace it with your project's data.\n";

const TOML_CMDLINE_SAMPLE: &str = "# Sample TOML document: Hello, world! greeting config.\n\
[greeting]\n\
message = \"Hello, world!\"\n\
target = \"world\"\n";

const TOML_LIB_SAMPLE: &str = "# Library sample TOML document.\n\
[library]\n\
name = \"{name}\"\n\
greeting = \"Hello, world!\"\n";

// -------------------------------------------------------------------- YAML ---

const YAML_MANIFEST: &str = "# {name} - YAML configuration sample.\n\
# Edit this file or replace it with your project's data.\n";

const YAML_CMDLINE_SAMPLE: &str = "# Sample YAML document: Hello, world! greeting config.\n\
greeting:\n\
  message: Hello, world!\n\
  target: world\n";

const YAML_LIB_SAMPLE: &str = "# Library sample YAML document.\n\
library:\n\
  name: {name}\n\
  greeting: Hello, world!\n";

// -------------------------------------------------------------------- JSON ---

const JSON_MANIFEST: &str = "{\n\
  \"_comment\": \"{name} - JSON data sample. Replace with your project's data.\",\n\
  \"greeting\": \"Hello, world!\"\n\
}\n";

const JSON_CMDLINE_SAMPLE: &str = "{\n\
  \"message\": \"Hello, world!\",\n\
  \"target\": \"world\"\n\
}\n";

const JSON_LIB_SAMPLE: &str = "{\n\
  \"library\": {\n\
    \"name\": \"{name}\",\n\
    \"greeting\": \"Hello, world!\"\n\
  }\n\
}\n";

// --------------------------------------------------------------------- XML ---

const XML_MANIFEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!-- {name} - XML document sample. -->\n\
<greeting message=\"Hello, world!\" />\n";

const XML_CMDLINE_SAMPLE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!-- Sample XML document: Hello, world! greeting. -->\n\
<greeting message=\"Hello, world!\" target=\"world\" />\n";

const XML_LIB_SAMPLE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!-- Library sample XML document. -->\n\
<library name=\"{name}\">\n\
  <greeting>Hello, world!</greeting>\n\
</library>\n";

// -------------------------------------------------------------------- HTML ---

const HTML_MANIFEST: &str = "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <title>{name}</title>\n\
</head>\n\
<body>\n\
  <h1>Hello, world!</h1>\n\
  <p>{name} - open index.html in a browser.</p>\n\
</body>\n\
</html>\n";

const HTML_CMDLINE_SAMPLE: &str = "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <title>Hello</title>\n\
</head>\n\
<body>\n\
  <!-- Sample page: Hello, world! greeting. -->\n\
  <h1>Hello, world!</h1>\n\
</body>\n\
</html>\n";

const HTML_LIB_SAMPLE: &str = "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <title>{name} library</title>\n\
</head>\n\
<body>\n\
  <!-- Library sample page. -->\n\
  <h1>Hello, world!</h1>\n\
</body>\n\
</html>\n";

// --------------------------------------------------------------------- CSS ---

const CSS_MANIFEST: &str = "/* {name} - CSS stylesheet sample. */\n";

const CSS_CMDLINE_SAMPLE: &str = "/* Sample stylesheet: Hello, world! banner. */\n\
.hello {\n\
  color: rebeccapurple;\n\
}\n";

const CSS_LIB_SAMPLE: &str = "/* Library sample stylesheet. */\n\
/* {name} - Hello, world! */\n\
.library {\n\
  color: rebeccapurple;\n\
}\n";

// -------------------------------------------------------------------- SCSS ---

const SCSS_MANIFEST: &str = "// {name} - SCSS stylesheet sample.\n";

const SCSS_CMDLINE_SAMPLE: &str = "// Sample SCSS: Hello, world! banner.\n\
.hello {\n\
  color: rebeccapurple;\n\
  &:hover {\n\
    color: black;\n\
  }\n\
}\n";

const SCSS_LIB_SAMPLE: &str = "// Library sample SCSS.\n\
// {name} - Hello, world!\n\
.library {\n\
  color: rebeccapurple;\n\
}\n";

// --------------------------------------------------------------------- SQL ---

const SQL_MANIFEST: &str = "-- {name} - SQL script sample.\n";

const SQL_CMDLINE_SAMPLE: &str = "-- Sample SQL script: Hello, world! via SELECT.\n\
SELECT 'Hello, world!' AS greeting;\n";

const SQL_LIB_SAMPLE: &str = "-- Library sample SQL script.\n\
-- {name}: Hello, world!\n\
CREATE TABLE greeting (\n\
  message TEXT NOT NULL\n\
);\n";

// ---------------------------------------------------------------- Markdown ---

const MARKDOWN_MANIFEST: &str = "# {name}\n\n\
Project notes live here. `README.md` is generated separately by the\n\
scaffolder's documentation layer.\n";

const MARKDOWN_CMDLINE_SAMPLE: &str = "# Hello, world!\n\n\
Sample Markdown document with a greeting.\n";

const MARKDOWN_LIB_SAMPLE: &str = "# {name} library\n\n\
Sample library documentation. Says: Hello, world!\n";

// ---------------------------------------------------------------- Protobuf ---

const PROTOBUF_MANIFEST: &str = "// {name} - Protocol Buffers schema sample.\n";

const PROTOBUF_CMDLINE_SAMPLE: &str = "syntax = \"proto3\";\n\
\n\
// Sample schema: Hello, world! greeting.\n\
message Greeting {\n\
  string message = 1;\n\
}\n";

const PROTOBUF_LIB_SAMPLE: &str = "syntax = \"proto3\";\n\
\n\
// Library sample schema for {name}.\n\
message LibraryGreeting {\n\
  string message = 1; // Hello, world!\n\
}\n";

// ----------------------------------------------------------------- Verilog ---

const VERILOG_MANIFEST: &str = "// {name} - Verilog project. Simulate with iverilog.\n";

const VERILOG_CMDLINE_SAMPLE: &str = "// Sample Verilog module: Hello, world! via simulation output.\n\
module hello;\n\
  initial begin\n\
    $display(\"Hello, world!\");\n\
    $finish;\n\
  end\n\
endmodule\n";

const VERILOG_LIB_SAMPLE: &str = "// Library sample Verilog module.\n\
// {name}: prints Hello, world! when enabled.\n\
module hello_lib;\n\
  initial $display(\"Hello, world!\");\n\
endmodule\n";

// -------------------------------------------------------------------- VHDL ---

const VHDL_MANIFEST: &str = "-- {name} - VHDL project. Simulate with ghdl.\n";

const VHDL_CMDLINE_SAMPLE: &str = "-- Sample VHDL entity: Hello, world! via simulation output.\n\
entity hello is\n\
end hello;\n\
\n\
architecture behavior of hello is\n\
begin\n\
  process\n\
  begin\n\
    report \"Hello, world!\";\n\
    wait;\n\
  end process;\n\
end behavior;\n";

const VHDL_LIB_SAMPLE: &str = "-- Library sample VHDL entity.\n\
-- {name}: reports Hello, world! when simulated.\n\
entity hello_lib is\n\
end hello_lib;\n\
\n\
architecture behavior of hello_lib is\n\
begin\n\
  process\n\
  begin\n\
    report \"Hello, world!\";\n\
    wait;\n\
  end process;\n\
end behavior;\n";

// --------------------------------------------------------------- Terraform ---

const TERRAFORM_MANIFEST: &str = "# {name} - Terraform configuration.\n\
# Run `terraform init && terraform plan` to preview.\n";

const TERRAFORM_CMDLINE_SAMPLE: &str = "# Sample Terraform config: Hello, world! via local output.\n\
output \"greeting\" {\n\
  value = \"Hello, world!\"\n\
}\n";

const TERRAFORM_LIB_SAMPLE: &str = "# Library sample Terraform module.\n\
# {name}: outputs Hello, world!.\n\
variable \"greeting\" {\n\
  type    = string\n\
  default = \"Hello, world!\"\n\
}\n";

// --------------------------------------------------------------- OpenSCAD ---

const OPENSCAD_MANIFEST: &str = "// {name} - OpenSCAD project. Render with openscad.\n";

const OPENSCAD_CMDLINE_SAMPLE: &str = "// Sample OpenSCAD model: Hello, world! text plate.\n\
text(\"Hello, world!\", size = 10);\n";

const OPENSCAD_LIB_SAMPLE: &str = "// Library sample OpenSCAD module.\n\
// {name}: renders the Hello, world! plate.\n\
module hello_plate() {\n\
  text(\"Hello, world!\", size = 10);\n\
}\n";

// ------------------------------------------------------------------- CMake ---

const CMAKE_MANIFEST: &str = "# {name} - CMake build definition.\n\
# Generic skeleton: adjust `project()` and targets for your sources.\n\
cmake_minimum_required(VERSION 3.16)\n\
project({name} C)\n";

const CMAKE_CMDLINE_SAMPLE: &str = "// Sample C source: Hello, world! console program.\n\
#include <stdio.h>\n\
\n\
int main(void) {\n\
    printf(\"Hello, world!\\n\");\n\
    return 0;\n\
}\n";

const CMAKE_LIB_SAMPLE: &str = "// Library sample C source for {name}.\n\
#include <stdio.h>\n\
\n\
void hello(void) {\n\
    printf(\"Hello, world!\\n\");\n\
}\n";

// ------------------------------------------------------------------ Gradle ---

const GRADLE_MANIFEST: &str = "// {name} - Gradle build definition (Groovy DSL).\n\
plugins {\n\
    id 'java'\n\
}\n";

const GRADLE_CMDLINE_SAMPLE: &str = "// Sample Java source: Hello, world! console program.\n\
public class Main {\n\
    public static void main(String[] args) {\n\
        System.out.println(\"Hello, world!\");\n\
    }\n\
}\n";

const GRADLE_LIB_SAMPLE: &str = "// Library sample Java source for {name}.\n\
public class Hello {\n\
    public static String hello() {\n\
        return \"Hello, world!\";\n\
    }\n\
}\n";

// -------------------------------------------------------------- Gradle KTS ---

const GRADLE_KTS_MANIFEST: &str = "// {name} - Gradle build definition (Kotlin DSL).\n\
plugins {\n\
    id(\"java\")\n\
}\n";

const GRADLE_KTS_CMDLINE_SAMPLE: &str = "// Sample Java source: Hello, world! console program.\n\
public class Main {\n\
    public static void main(String[] args) {\n\
        System.out.println(\"Hello, world!\");\n\
    }\n\
}\n";

const GRADLE_KTS_LIB_SAMPLE: &str = "// Library sample Java source for {name}.\n\
public class Hello {\n\
    public static String hello() {\n\
        return \"Hello, world!\";\n\
    }\n\
}\n";

// ------------------------------------------------------------------- Maven ---

const MAVEN_MANIFEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<project xmlns=\"http://maven.apache.org/POM/4.0.0\"\n\
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n\
         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd\">\n\
  <modelVersion>4.0.0</modelVersion>\n\
  <groupId>com.example</groupId>\n\
  <artifactId>{name}</artifactId>\n\
  <version>0.1.0</version>\n\
  <packaging>pom</packaging>\n\
</project>\n";

const MAVEN_CMDLINE_SAMPLE: &str = "// Sample Java source: Hello, world! console program.\n\
public class Main {\n\
    public static void main(String[] args) {\n\
        System.out.println(\"Hello, world!\");\n\
    }\n\
}\n";

const MAVEN_LIB_SAMPLE: &str = "// Library sample Java source for {name}.\n\
public class Hello {\n\
    public static String hello() {\n\
        return \"Hello, world!\";\n\
    }\n\
}\n";

// --------------------------------------------------------------------- Nix ---

const NIX_MANIFEST: &str = "# {name} - Nix expression sample.\n";

const NIX_CMDLINE_SAMPLE: &str = "# Sample Nix expression: Hello, world! derivation output.\n\
{ pkgs ? import <nixpkgs> {} }:\n\
pkgs.writeText \"greeting\" \"Hello, world!\\n\"\n";

const NIX_LIB_SAMPLE: &str = "# Library sample Nix function for {name}.\n\
{\n\
  hello = \"Hello, world!\";\n\
}\n";

// --------------------------------------------------------------------- HCL ---

const HCL_MANIFEST: &str = "# {name} - HCL configuration sample.\n";

const HCL_CMDLINE_SAMPLE: &str = "# Sample HCL block: Hello, world! greeting config.\n\
greeting \"hello\" {\n\
  message = \"Hello, world!\"\n\
}\n";

const HCL_LIB_SAMPLE: &str = "# Library sample HCL module for {name}.\n\
variable \"greeting\" {\n\
  type    = string\n\
  default = \"Hello, world!\"\n\
}\n";

// ---------------------------------------------------------------- Registry --

/// The language registry (FR-017): one recipe per supported language.
///
/// Single source of truth — `/new help` value lists and the renderer both
/// derive from this table, so adding a language extends the whole surface.
pub static REGISTRY: &[LanguageRecipe] = &[
    LanguageRecipe {
        language: Language::Rust,
        manifest_path: "Cargo.toml",
        manifest_template: RUST_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.rs",
                    content: RUST_MAIN_RS,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/lib.rs",
                    content: RUST_LIB_RS,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.rs",
                    content: RUST_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.rs",
                    content: RUST_GUI_MAIN,
                },
            ),
        ],
        run_command: "cargo run",
        test_command: "cargo test",
        gitignore_body: "/target\nCargo.lock.bak\n*.pdb\n",
    },
    LanguageRecipe {
        language: Language::Python,
        manifest_path: "pyproject.toml",
        manifest_template: PYTHON_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.py",
                    content: PYTHON_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/{name}/__init__.py",
                    content: PYTHON_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.py",
                    content: PYTHON_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.py",
                    content: PYTHON_GUI_MAIN,
                },
            ),
        ],
        run_command: "python3 main.py",
        test_command: "python3 -m unittest discover",
        gitignore_body: "__pycache__/\n*.py[cod]\n.venv/\n",
    },
    LanguageRecipe {
        language: Language::Go,
        manifest_path: "go.mod",
        manifest_template: GO_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.go",
                    content: GO_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "{name}.go",
                    content: GO_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.go",
                    content: GO_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.go",
                    content: GO_GUI_MAIN,
                },
            ),
        ],
        run_command: "go run .",
        test_command: "go test ./...",
        gitignore_body: "*.exe\n*.test\nvendor/\n",
    },
    LanguageRecipe {
        language: Language::TypeScript,
        manifest_path: "package.json",
        manifest_template: TS_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.ts",
                    content: TS_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/index.ts",
                    content: TS_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.ts",
                    content: TS_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.ts",
                    content: TS_GUI_MAIN,
                },
            ),
        ],
        run_command: "npx tsx src/main.ts",
        test_command: "npm test",
        gitignore_body: "node_modules/\ndist/\n*.tsbuildinfo\n",
    },
    LanguageRecipe {
        language: Language::JavaScript,
        manifest_path: "package.json",
        manifest_template: JS_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.js",
                    content: JS_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/index.js",
                    content: JS_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.js",
                    content: JS_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.js",
                    content: JS_GUI_MAIN,
                },
            ),
        ],
        run_command: "node src/main.js",
        test_command: "npm test",
        gitignore_body: "node_modules/\ndist/\n",
    },
    LanguageRecipe {
        language: Language::C,
        manifest_path: "build-notes.txt",
        manifest_template: C_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.c",
                    content: C_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.c",
                    content: C_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.c",
                    content: C_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.c",
                    content: C_GUI_MAIN,
                },
            ),
        ],
        run_command: "cc -o app src/main.c && ./app",
        test_command: "cc -o app src/main.c && ./app",
        gitignore_body: "*.o\n*.out\napp\n",
    },
    LanguageRecipe {
        language: Language::Cpp,
        manifest_path: "build-notes.txt",
        manifest_template: CPP_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.cpp",
                    content: CPP_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.cpp",
                    content: CPP_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.cpp",
                    content: CPP_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.cpp",
                    content: CPP_GUI_MAIN,
                },
            ),
        ],
        run_command: "c++ -std=c++17 -o app src/main.cpp && ./app",
        test_command: "c++ -std=c++17 -o app src/main.cpp && ./app",
        gitignore_body: "*.o\n*.out\napp\n",
    },
    LanguageRecipe {
        language: Language::Java,
        manifest_path: "build-notes.txt",
        manifest_template: JAVA_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/Main.java",
                    content: JAVA_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/Hello.java",
                    content: JAVA_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/Main.java",
                    content: JAVA_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/Main.java",
                    content: JAVA_GUI_MAIN,
                },
            ),
        ],
        run_command: "javac src/Main.java -d out && java -cp out Main",
        test_command: "javac src/Main.java -d out && java -cp out Main",
        gitignore_body: "*.class\nout/\n",
    },
    LanguageRecipe {
        language: Language::Kotlin,
        manifest_path: "build-notes.txt",
        manifest_template: KOTLIN_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.kt",
                    content: KOTLIN_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.kt",
                    content: KOTLIN_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.kt",
                    content: KOTLIN_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.kt",
                    content: KOTLIN_GUI_MAIN,
                },
            ),
        ],
        run_command: "kotlinc src/main.kt -include-runtime -d app.jar && java -jar app.jar",
        test_command: "kotlinc src/main.kt -include-runtime -d app.jar && java -jar app.jar",
        gitignore_body: "*.jar\n*.class\n",
    },
    LanguageRecipe {
        language: Language::Ruby,
        manifest_path: "Gemfile",
        manifest_template: RUBY_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.rb",
                    content: RUBY_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.rb",
                    content: RUBY_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.rb",
                    content: RUBY_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.rb",
                    content: RUBY_GUI_MAIN,
                },
            ),
        ],
        run_command: "ruby main.rb",
        test_command: "ruby main.rb",
        gitignore_body: "*.gem\n.bundle/\n",
    },
    LanguageRecipe {
        language: Language::Swift,
        manifest_path: "build-notes.txt",
        manifest_template: SWIFT_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "Sources/main.swift",
                    content: SWIFT_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "Sources/hello.swift",
                    content: SWIFT_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "Sources/main.swift",
                    content: SWIFT_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "Sources/main.swift",
                    content: SWIFT_GUI_MAIN,
                },
            ),
        ],
        run_command: "swift Sources/main.swift",
        test_command: "swift Sources/main.swift",
        gitignore_body: ".build/\n*.o\n",
    },
    LanguageRecipe {
        language: Language::CSharp,
        manifest_path: "build-notes.txt",
        manifest_template: CSHARP_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/Program.cs",
                    content: CSHARP_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/Hello.cs",
                    content: CSHARP_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/Program.cs",
                    content: CSHARP_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/Program.cs",
                    content: CSHARP_GUI_MAIN,
                },
            ),
        ],
        run_command: "dotnet run",
        test_command: "dotnet test",
        gitignore_body: "bin/\nobj/\n",
    },
    LanguageRecipe {
        language: Language::Lua,
        manifest_path: "rockspec-notes.txt",
        manifest_template: LUA_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.lua",
                    content: LUA_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "hello.lua",
                    content: LUA_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.lua",
                    content: LUA_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.lua",
                    content: LUA_GUI_MAIN,
                },
            ),
        ],
        run_command: "lua main.lua",
        test_command: "lua main.lua",
        gitignore_body: "*.luac\n",
    },
    LanguageRecipe {
        language: Language::Zig,
        manifest_path: "build.zig.zon",
        manifest_template: ZIG_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.zig",
                    content: ZIG_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.zig",
                    content: ZIG_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.zig",
                    content: ZIG_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.zig",
                    content: ZIG_GUI_MAIN,
                },
            ),
        ],
        run_command: "zig run src/main.zig",
        test_command: "zig test src/main.zig",
        gitignore_body: "zig-out/\n.zig-cache/\n",
    },
    LanguageRecipe {
        language: Language::Nim,
        manifest_path: "build-notes.txt",
        manifest_template: NIM_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.nim",
                    content: NIM_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.nim",
                    content: NIM_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.nim",
                    content: NIM_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.nim",
                    content: NIM_GUI_MAIN,
                },
            ),
        ],
        run_command: "nim c -r src/main.nim",
        test_command: "nim c -r src/main.nim",
        gitignore_body: "nimcache/\n",
    },
    LanguageRecipe {
        language: Language::Elixir,
        manifest_path: "mix.exs.notes.txt",
        manifest_template: ELIXIR_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "lib/main.ex",
                    content: ELIXIR_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.ex",
                    content: ELIXIR_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "lib/main.ex",
                    content: ELIXIR_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "lib/main.ex",
                    content: ELIXIR_GUI_MAIN,
                },
            ),
        ],
        run_command: "elixir lib/main.ex",
        test_command: "elixir lib/main.ex",
        gitignore_body: "_build/\ndev/\n",
    },
    LanguageRecipe {
        language: Language::Erlang,
        manifest_path: "erlang-notes.txt",
        manifest_template: ERLANG_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.erl",
                    content: ERLANG_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.erl",
                    content: ERLANG_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "src/main.erl",
                    content: ERLANG_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "src/main.erl",
                    content: ERLANG_GUI_MAIN,
                },
            ),
        ],
        run_command: "erlc src/main.erl && escript main.beam",
        test_command: "erlc src/main.erl && escript main.beam",
        gitignore_body: "*.beam\n",
    },
    LanguageRecipe {
        language: Language::Haskell,
        manifest_path: "haskell-notes.txt",
        manifest_template: HASKELL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "app/Main.hs",
                    content: HASKELL_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/Hello.hs",
                    content: HASKELL_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "app/Main.hs",
                    content: HASKELL_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "app/Main.hs",
                    content: HASKELL_GUI_MAIN,
                },
            ),
        ],
        run_command: "runghc app/Main.hs",
        test_command: "runghc app/Main.hs",
        gitignore_body: "dist/\ndist-newstyle/\n*.hi\n*.o\n",
    },
    LanguageRecipe {
        language: Language::Ocaml,
        manifest_path: "ocaml-notes.txt",
        manifest_template: OCAML_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "bin/main.ml",
                    content: OCAML_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.ml",
                    content: OCAML_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "bin/main.ml",
                    content: OCAML_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "bin/main.ml",
                    content: OCAML_GUI_MAIN,
                },
            ),
        ],
        run_command: "ocaml bin/main.ml",
        test_command: "ocaml bin/main.ml",
        gitignore_body: "_build/\n*.cmi\n*.cmo\n",
    },
    LanguageRecipe {
        language: Language::R,
        manifest_path: "DESCRIPTION.notes.txt",
        manifest_template: R_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.R",
                    content: R_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "R/hello.R",
                    content: R_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.R",
                    content: R_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.R",
                    content: R_GUI_MAIN,
                },
            ),
        ],
        run_command: "Rscript main.R",
        test_command: "Rscript main.R",
        gitignore_body: ".Rproj\n.Rhistory\n",
    },
    LanguageRecipe {
        language: Language::Dart,
        manifest_path: "pubspec-notes.txt",
        manifest_template: DART_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "bin/main.dart",
                    content: DART_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.dart",
                    content: DART_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "bin/main.dart",
                    content: DART_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "bin/main.dart",
                    content: DART_GUI_MAIN,
                },
            ),
        ],
        run_command: "dart run bin/main.dart",
        test_command: "dart run bin/main.dart",
        gitignore_body: ".dart_tool/\n.packages\n",
    },
    LanguageRecipe {
        language: Language::Php,
        manifest_path: "composer.json.notes.txt",
        manifest_template: PHP_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.php",
                    content: PHP_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.php",
                    content: PHP_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.php",
                    content: PHP_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.php",
                    content: PHP_GUI_MAIN,
                },
            ),
        ],
        run_command: "php main.php",
        test_command: "php main.php",
        gitignore_body: "vendor/\n",
    },
    LanguageRecipe {
        language: Language::Perl,
        manifest_path: "cpanfile.notes.txt",
        manifest_template: PERL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.pl",
                    content: PERL_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.pl",
                    content: PERL_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.pl",
                    content: PERL_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.pl",
                    content: PERL_GUI_MAIN,
                },
            ),
        ],
        run_command: "perl main.pl",
        test_command: "perl main.pl",
        gitignore_body: "local/\n*.bak\n",
    },
    LanguageRecipe {
        language: Language::Shell,
        manifest_path: "build-notes.txt",
        manifest_template: SHELL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.sh",
                    content: SHELL_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.sh",
                    content: SHELL_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.sh",
                    content: SHELL_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.sh",
                    content: SHELL_GUI_MAIN,
                },
            ),
        ],
        run_command: "sh main.sh",
        test_command: "sh main.sh",
        gitignore_body: "*.log\n",
    },
    LanguageRecipe {
        language: Language::Zsh,
        manifest_path: "build-notes.txt",
        manifest_template: ZSH_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.zsh",
                    content: ZSH_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.zsh",
                    content: ZSH_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.zsh",
                    content: ZSH_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.zsh",
                    content: ZSH_GUI_MAIN,
                },
            ),
        ],
        run_command: "zsh main.zsh",
        test_command: "zsh main.zsh",
        gitignore_body: "*.log\n",
    },
    LanguageRecipe {
        language: Language::Fish,
        manifest_path: "build-notes.txt",
        manifest_template: FISH_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "main.fish",
                    content: FISH_MAIN,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib/hello.fish",
                    content: FISH_LIB,
                },
            ),
            (
                AppType::Tui,
                SourceFile {
                    path: "main.fish",
                    content: FISH_TUI_MAIN,
                },
            ),
            (
                AppType::Gui,
                SourceFile {
                    path: "main.fish",
                    content: FISH_GUI_MAIN,
                },
            ),
        ],
        run_command: "fish main.fish",
        test_command: "fish main.fish",
        gitignore_body: "*.log\n",
    },
    LanguageRecipe {
        language: Language::Toml,
        manifest_path: "toml-notes.txt",
        manifest_template: TOML_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.toml",
                    content: TOML_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.toml",
                    content: TOML_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Yaml,
        manifest_path: "yaml-notes.txt",
        manifest_template: YAML_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.yaml",
                    content: YAML_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.yaml",
                    content: YAML_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Json,
        manifest_path: "json-notes.txt",
        manifest_template: JSON_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.json",
                    content: JSON_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.json",
                    content: JSON_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Xml,
        manifest_path: "xml-notes.txt",
        manifest_template: XML_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.xml",
                    content: XML_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.xml",
                    content: XML_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Html,
        manifest_path: "index.html",
        manifest_template: HTML_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.html",
                    content: HTML_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.html",
                    content: HTML_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - open index.html in a browser)",
        test_command: "(none - open index.html in a browser)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Css,
        manifest_path: "css-notes.txt",
        manifest_template: CSS_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "styles/main.css",
                    content: CSS_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "styles/library.css",
                    content: CSS_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Scss,
        manifest_path: "scss-notes.txt",
        manifest_template: SCSS_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "styles/main.scss",
                    content: SCSS_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "styles/library.scss",
                    content: SCSS_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - compile with `sass` first)",
        test_command: "(none - compile with `sass` first)",
        gitignore_body: ".sass-cache/\n",
    },
    LanguageRecipe {
        language: Language::Sql,
        manifest_path: "sql-notes.txt",
        manifest_template: SQL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "scripts/sample.sql",
                    content: SQL_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "scripts/schema.sql",
                    content: SQL_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - run against a SQL engine)",
        test_command: "(none - run against a SQL engine)",
        gitignore_body: "*.db\n*.sqlite\n",
    },
    LanguageRecipe {
        language: Language::Markdown,
        manifest_path: "NOTES.md",
        manifest_template: MARKDOWN_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "docs/sample.md",
                    content: MARKDOWN_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "docs/library.md",
                    content: MARKDOWN_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
    LanguageRecipe {
        language: Language::Protobuf,
        manifest_path: "protobuf-notes.txt",
        manifest_template: PROTOBUF_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "proto/sample.proto",
                    content: PROTOBUF_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "proto/library.proto",
                    content: PROTOBUF_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - compile with `protoc` first)",
        test_command: "(none - compile with `protoc` first)",
        gitignore_body: "*.pb.go\n*_pb2.py\n",
    },
    LanguageRecipe {
        language: Language::Verilog,
        manifest_path: "verilog-notes.txt",
        manifest_template: VERILOG_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/hello.v",
                    content: VERILOG_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello_lib.v",
                    content: VERILOG_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "iverilog -o hello.vvp src/hello.v && vvp hello.vvp",
        test_command: "iverilog -o hello.vvp src/hello.v && vvp hello.vvp",
        gitignore_body: "*.vvp\n*.vcd\n",
    },
    LanguageRecipe {
        language: Language::Vhdl,
        manifest_path: "vhdl-notes.txt",
        manifest_template: VHDL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/hello.vhd",
                    content: VHDL_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello_lib.vhd",
                    content: VHDL_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - analyse with `ghdl -a` first)",
        test_command: "(none - analyse with `ghdl -a` first)",
        gitignore_body: "*.o\n*.cf\nwork-\n",
    },
    LanguageRecipe {
        language: Language::Terraform,
        manifest_path: "main.tf",
        manifest_template: TERRAFORM_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "outputs.tf",
                    content: TERRAFORM_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "variables.tf",
                    content: TERRAFORM_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "terraform init && terraform plan",
        test_command: "terraform validate",
        gitignore_body: ".terraform/\n*.tfstate\n*.tfstate.*\n",
    },
    LanguageRecipe {
        language: Language::OpenScad,
        manifest_path: "openscad-notes.txt",
        manifest_template: OPENSCAD_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/hello.scad",
                    content: OPENSCAD_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello_plate.scad",
                    content: OPENSCAD_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "openscad -o hello.png src/hello.scad",
        test_command: "openscad -o hello.png src/hello.scad",
        gitignore_body: "*.stl\n*.png\n",
    },
    LanguageRecipe {
        language: Language::Cmake,
        manifest_path: "CMakeLists.txt",
        manifest_template: CMAKE_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main.c",
                    content: CMAKE_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/hello.c",
                    content: CMAKE_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "cmake -S . -B build && cmake --build build && ./build/app",
        test_command: "cmake -S . -B build && cmake --build build",
        gitignore_body: "build/\nCMakeCache.txt\n",
    },
    LanguageRecipe {
        language: Language::Gradle,
        manifest_path: "build.gradle",
        manifest_template: GRADLE_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main/java/Main.java",
                    content: GRADLE_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/main/java/Hello.java",
                    content: GRADLE_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "gradle build",
        test_command: "gradle test",
        gitignore_body: "build/\n.gradle/\n",
    },
    LanguageRecipe {
        language: Language::GradleKts,
        manifest_path: "build.gradle.kts",
        manifest_template: GRADLE_KTS_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main/java/Main.java",
                    content: GRADLE_KTS_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/main/java/Hello.java",
                    content: GRADLE_KTS_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "gradle build",
        test_command: "gradle test",
        gitignore_body: "build/\n.gradle/\n",
    },
    LanguageRecipe {
        language: Language::Maven,
        manifest_path: "pom.xml",
        manifest_template: MAVEN_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "src/main/java/Main.java",
                    content: MAVEN_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "src/main/java/Hello.java",
                    content: MAVEN_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "mvn package",
        test_command: "mvn test",
        gitignore_body: "target/\n",
    },
    LanguageRecipe {
        language: Language::Nix,
        manifest_path: "nix-notes.txt",
        manifest_template: NIX_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "default.nix",
                    content: NIX_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "lib.nix",
                    content: NIX_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "nix-build",
        test_command: "nix-instantiate --eval default.nix",
        gitignore_body: "result\nresult-*\n",
    },
    LanguageRecipe {
        language: Language::Hcl,
        manifest_path: "hcl-notes.txt",
        manifest_template: HCL_MANIFEST,
        sources: &[
            (
                AppType::Cmdline,
                SourceFile {
                    path: "sample.hcl",
                    content: HCL_CMDLINE_SAMPLE,
                },
            ),
            (
                AppType::Library,
                SourceFile {
                    path: "library.hcl",
                    content: HCL_LIB_SAMPLE,
                },
            ),
        ],
        run_command: "(none - data format)",
        test_command: "(none - data format)",
        gitignore_body: "",
    },
];
