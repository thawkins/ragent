//! Guard: no escape hatch re-enables a retired tool spelling (spec
//! `uniformtools` FR-031).
//!
//! FR-031 is an *unwanted-behaviour* requirement: if any supported mechanism - a
//! configuration key, an environment variable, a CLI flag, or a plugin,
//! connector or agent setting - would make a retired `before` spelling
//! resolvable, the system **shall** not honour it, and no such key, variable or
//! flag **shall** be added.
//!
//! The escape-hatch audit (T-043) found exactly one mechanism that can name a
//! tool and therefore restore a spelling: the `tools.aliases` table. Its
//! per-entry rejection of a retired spelling is FR-031's own clause and is
//! implemented by the alias-entry validation task; this file does not duplicate
//! it. Everything else is checked here, so the audit stays true as the tree
//! grows rather than being a sentence in a changelog:
//!
//! | # | Predicate | Why it is the right check |
//! |---|---|---|
//! | 1 | no `ragent.json` field name equals a rename spelling | a key called `bash` would be the obvious compatibility switch |
//! | 2 | no environment variable is *named* a rename spelling | a `RAGENT_BASH=1`-style switch has to be named something |
//! | 3 | the resolution path reads no environment and loads no config | resolution is a pure function of the registry and the table |
//! | 4 | no CLI flag, alias or subcommand equals a rename spelling | `--bash` would be a flag, and flags fail loudly |
//! | 5 | the `tools` section has exactly its three documented keys | a fourth key could only be one that names a tool |
//! | 6 | no plugin, connector, agent, hook or skill setting is named one | those settings are the remaining third-party surfaces |
//!
//! ## What "named" means, and why the match is whole-name
//!
//! Every predicate compares an *identifier* - a field name, a variable name, a
//! flag name - against the rename spellings as a whole. A substring rule would
//! be wrong in both directions: `GIT_BASH` (the Windows Git Bash install path,
//! `ragent-tools-core/src/bash.rs`) contains `BASH` without being a switch that
//! restores `bash`, and `bash_allowlist` would contain `bash` without being one
//! either. A mechanism that restored a spelling would have to be *called* the
//! spelling, or it would not be the compatibility switch FR-031 forbids.
//!
//! ## What this file deliberately does not assert
//!
//! Settings whose *value* may be a tool name are not escape hatches, and the
//! audit records why rather than testing them:
//!
//! - a skill's `allowed-tools` and an agent's `allowed_tools` **restrict** the
//!   set a model may call; a name they list that no tool holds simply matches
//!   nothing, so they cannot make a retired spelling resolvable;
//! - a hook `matcher` is compared against the name of a call that has already
//!   been resolved, so it can only fail to match - which FR-028 reports at load
//!   instead of honouring;
//! - a plugin id, connector id, MCP server id, backend id, ACP agent id or
//!   `agent.<name>` key is a *source* identifier, and every tool those sources
//!   contribute is namespaced (`plugin_<id>_<tool>`, `mcp_<server>_<tool>`), so
//!   no id can occupy a bare ragent name - that refusal is FR-032;
//! - `agent.options` and a provider `options` map are free-form, but the keys
//!   anything reads from them are a closed, short list - `acp` on an agent, and
//!   `model_id`, `auto_start`, `device`, `models_path`, `profile`, `region`,
//!   `reasoning_level` and the legacy `thinking` on a provider - so a tool name
//!   placed there is never read.

use std::fs;
use std::path::{Path, PathBuf};

use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_config::ToolsConfig;
use regex::Regex;

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

/// Every `.rs` file under `dir`, recursively (skips `tests/`, `benches/`,
/// `examples/` and `build.rs`, which cannot define a user-facing key, variable
/// or flag).
fn collect_production_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(name, "tests" | "benches" | "examples" | "target") {
                continue;
            }
            collect_production_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "build.rs")
        {
            out.push(path);
        }
    }
}

/// Production sources of the whole workspace.
fn production_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_production_sources(&workspace_root().join("crates"), &mut out);
    collect_production_sources(&workspace_root().join("src"), &mut out);
    out.sort();
    out
}

/// Production sources of one crate-relative directory.
fn production_sources_under(relative: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_production_sources(&workspace_root().join(relative), &mut out);
    out.sort();
    out
}

/// Read a file relative to the workspace root.
fn read(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `true` when `name` is a retired `before` spelling.
fn is_before(name: &str) -> bool {
    TOOL_RENAMES.iter().any(|row| row.before == name)
}

/// `true` when `name` equals either spelling of any FR-023 pair.
///
/// Both columns are checked, not just `before`: a mechanism *called* `Bash`
/// would be an equally effective alias for `bash`, so it is refused for the same
/// reason.
fn is_rename_spelling(name: &str) -> bool {
    TOOL_RENAMES
        .iter()
        .any(|row| row.before == name || row.after == name)
}

/// `snake_case` to `kebab-case`, the way clap derives a long flag from a field.
fn kebab(ident: &str) -> String {
    ident.replace('_', "-")
}

/// A key/flag/variable name that legitimately equals a rename spelling, with the
/// written reason it is not a switch that restores one.
///
/// Every entry is reviewed individually. An entry for a site that *does* let a
/// retired name resolve is a defect, not an exemption.
const EXEMPTIONS: &[(&str, &str, &str)] = &[(
    "crates/ragent-config/src/config.rs",
    "bash",
    "`bash` is the shell-command allowlist/denylist *section* (`bash.allowlist`, \
     `bash.denylist`), named for the `bash:execute` permission family that FR-028 \
     keeps unchanged. It configures which shell commands may run; it names no \
     tool and registers none.",
)];

/// The written reason `key` at `file` is exempt, if it is.
fn exemption_for(file: &str, key: &str) -> Option<&'static str> {
    EXEMPTIONS
        .iter()
        .find(|(path, name, _)| *path == file && *name == key)
        .map(|(_, _, reason)| *reason)
}

/// Every field identifier declared inside a `struct` or `enum` body in
/// `source`, with its line number.
///
/// Private fields are included, not just `pub` ones: a deserialised field that is
/// only read inside its own crate is still a `ragent.json` key, so restricting the
/// scan to `pub` would have let a private `bash: bool` through. Scanning the item
/// *bodies* rather than every `name:` line matters too - a struct literal
/// (`bash: BashConfig::default()`) and a match arm look identical to a field
/// declaration, and both would be false positives.
fn declared_field_names(source: &str) -> Vec<(usize, String)> {
    let item = Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+[A-Za-z_]")
        .expect("static item regex");
    let field = Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:")
        .expect("static field regex");

    let mut declared = Vec::new();
    let mut inside = false;
    let mut depth: i32 = 0;
    for (index, line) in source.lines().enumerate() {
        if !inside && item.is_match(line) {
            inside = true;
            depth = 0;
        }
        if !inside {
            continue;
        }
        if let Some(captures) = field.captures(line) {
            declared.push((index + 1, captures[1].to_string()));
        }
        depth += i32::try_from(line.matches('{').count()).unwrap_or(0);
        depth -= i32::try_from(line.matches('}').count()).unwrap_or(0);
        // A unit struct (`pub struct Foo;`) closes on its own line; a braced one
        // closes when the matching brace brings the depth back to zero.
        if depth <= 0 && (line.contains('}') || line.trim_end().ends_with(';')) {
            inside = false;
        }
    }
    declared
}

/// Every `#[serde(rename = "…")]` / `#[serde(alias = "…")]` value in `source`,
/// with its line number - a second accepted JSON spelling for a field.
fn serde_name_attributes(source: &str) -> Vec<(usize, String)> {
    let attribute =
        Regex::new(r#"(?:rename|alias)\s*=\s*"([^"]+)""#).expect("static serde-rename regex");
    source
        .lines()
        .enumerate()
        .flat_map(|(index, line)| {
            attribute
                .captures_iter(line)
                .map(move |captures| (index + 1, captures[1].to_string()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. No `ragent.json` field name is a rename spelling
// ---------------------------------------------------------------------------

#[test]
fn test_no_config_field_name_is_a_rename_spelling() {
    let mut offenders = Vec::new();
    for path in production_sources_under("crates/ragent-config/src") {
        let relative = path
            .strip_prefix(workspace_root())
            .expect("path under root")
            .to_string_lossy()
            .into_owned();
        let source = fs::read_to_string(&path).expect("read config source");
        let candidates = declared_field_names(&source)
            .into_iter()
            .chain(serde_name_attributes(&source));
        for (line, name) in candidates {
            if !is_rename_spelling(&name) || exemption_for(&relative, &name).is_some() {
                continue;
            }
            offenders.push(format!("{relative}:{line} declares `{name}`"));
        }
    }
    assert!(
        offenders.is_empty(),
        "a configuration key may not be named a rename spelling (FR-031):\n  {}\n\
         If the key names no tool and registers none, add it to EXEMPTIONS with \
         the reason it is not a compatibility switch.",
        offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 2. No environment variable is *named* a rename spelling
// ---------------------------------------------------------------------------

#[test]
fn test_no_environment_variable_is_named_a_rename_spelling() {
    // Every helper the workspace reads an environment variable through. A new
    // helper must be added here, so the predicate cannot be bypassed by routing
    // a read through a function this list does not know.
    let reader = Regex::new(
        r#"(?:env::var|env::var_os|read_env|read_credential_env|read_config_env)\s*\(\s*"([^"]+)""#,
    )
    .expect("static env-reader regex");

    let mut offenders = Vec::new();
    let mut inspected = 0usize;
    for path in production_sources() {
        let relative = path
            .strip_prefix(workspace_root())
            .expect("path under root")
            .to_string_lossy()
            .into_owned();
        let source = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("cannot read {}: {e}", path.display());
        });
        for (index, line) in source.lines().enumerate() {
            for captures in reader.captures_iter(line) {
                inspected += 1;
                let name = &captures[1];
                if !is_rename_spelling(name) || exemption_for(&relative, name).is_some() {
                    continue;
                }
                offenders.push(format!("{relative}:{} reads `{name}`", index + 1));
            }
        }
    }
    assert!(
        inspected > 0,
        "the environment-variable scan found no read at all - the predicate has \
         been disarmed, not satisfied"
    );
    assert!(
        offenders.is_empty(),
        "no environment variable may be named a rename spelling (FR-031):\n  {}",
        offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 3. The resolution path reads no environment and loads no config
// ---------------------------------------------------------------------------

#[test]
fn test_tool_name_resolution_reads_no_environment_and_no_config_file() {
    // `resolve_inbound_tool_name` and the FR-023 table are the only two places a
    // spelling becomes a tool name. If either read an environment variable or
    // loaded a config file, that read would be an escape hatch by definition -
    // and it would also reintroduce per-step I/O (NFR-006). Both take their
    // inputs as arguments instead: the registry and an `AliasesConfig` value the
    // caller already holds.
    let forbidden = [
        "std::env",
        "env::var",
        "env::var_os",
        "read_env",
        "read_credential_env",
        "read_config_env",
        "Config::load",
        "load_config",
    ];
    for relative in [
        "crates/ragent-agent/src/tool/alias_table.rs",
        "crates/ragent-agent/src/tool/rename.rs",
        // The FR-023 table itself moved to `ragent-types` in T-042, because
        // `ragent-config` must read it when a config loads and cannot depend on
        // `ragent-agent`. The re-export above resolves to this file, so the
        // purity rule is asserted against the real source.
        "crates/ragent-types/src/tool_rename.rs",
    ] {
        let source = read(relative);
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{relative} contains `{needle}`: the name-resolution path must be a \
                 pure function of the registry and the alias table, so neither an \
                 environment variable nor a config file can change which spellings \
                 resolve (FR-031, NFR-006)"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. No CLI flag, alias or subcommand is a rename spelling
// ---------------------------------------------------------------------------

#[test]
fn test_no_cli_flag_or_subcommand_is_a_rename_spelling() {
    let explicit_long = Regex::new(r#"long\s*=\s*"([^"]+)""#).expect("static long regex");
    let alias = Regex::new(r#"(?:visible_)?alias\s*=\s*"([^"]+)""#).expect("static alias regex");
    let subcommand =
        Regex::new(r#"command\(name\s*=\s*"([^"]+)""#).expect("static subcommand regex");
    // A bare `long` is followed by `,` or `)`; `long = "…"` names its own flag.
    // Rust's regex crate has no look-ahead, so the bare form is spelled out.
    let bare_long = Regex::new(r"\blong\s*[,)]").expect("static bare-long regex");
    let field = Regex::new(r"^\s*([a-z_][a-z0-9_]*)\s*:").expect("static field regex");

    let mut offenders = Vec::new();
    let mut inspected = 0usize;
    for relative in ["src/main.rs", "src/cli.rs"] {
        let source = read(relative);
        let lines: Vec<&str> = source.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            for regex in [&explicit_long, &alias, &subcommand] {
                for captures in regex.captures_iter(line) {
                    inspected += 1;
                    if is_rename_spelling(&captures[1]) {
                        offenders.push(format!(
                            "{relative}:{} names `{}`",
                            index + 1,
                            &captures[1]
                        ));
                    }
                }
            }
            // A bare `#[arg(long)]` derives the flag from the field below it.
            if line.contains("#[arg(") && bare_long.is_match(line) {
                let below = lines[index + 1..]
                    .iter()
                    .find(|candidate| {
                        let trimmed = candidate.trim();
                        !trimmed.is_empty()
                            && !trimmed.starts_with("///")
                            && !trimmed.starts_with("#[")
                    })
                    .copied()
                    .unwrap_or_default();
                if let Some(captures) = field.captures(below) {
                    inspected += 1;
                    let flag = kebab(&captures[1]);
                    if is_rename_spelling(&flag) {
                        offenders.push(format!(
                            "{relative}:{} derives the flag `--{flag}`",
                            index + 2
                        ));
                    }
                }
            }
        }
    }
    assert!(
        inspected > 40,
        "the CLI scan found only {inspected} name(s); the predicate has been \
         disarmed, not satisfied"
    );
    assert!(
        offenders.is_empty(),
        "no CLI flag, alias or subcommand may be named a rename spelling (FR-031); \
         a flag is the loudest possible escape hatch:\n  {}",
        offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 5. The `tools` section has exactly its three documented keys
// ---------------------------------------------------------------------------

#[test]
fn test_tools_section_has_no_fourth_key_that_could_name_a_tool() {
    // A fourth key in `tools` could only be one that names a tool, so the key
    // set is pinned. `tools.aliases` is the single key that can carry a tool
    // name; its per-entry rejection of a retired spelling is FR-031's own clause.
    let populated: ToolsConfig = serde_json::from_str(
        r#"{"aliases": {"WebFetch": "webfetch"}, "expose_alias_names": true, "wire_trace": true}"#,
    )
    .expect("a fully populated tools section parses");
    let value = serde_json::to_value(&populated).expect("the tools section serialises");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("the tools section serialises as an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["aliases", "expose_alias_names", "wire_trace"],
        "the `tools` section is exactly the three documented keys; a fourth could \
         only add a way to name a tool (FR-031)"
    );
}

// ---------------------------------------------------------------------------
// 6. No plugin, connector, agent, hook or skill setting is a rename spelling
// ---------------------------------------------------------------------------

#[test]
fn test_no_dynamic_source_setting_is_a_rename_spelling() {
    // The third-party surfaces: a plugin manifest, a connector catalogue, an
    // agent profile (OASF JSON and Markdown frontmatter), a hook definition and a
    // skill frontmatter. Each is user- or vendor-authored, so each is a place a
    // key called `bash` could otherwise be introduced by content ragent reads
    // rather than by ragent's own source.
    let surfaces = [
        "crates/ragent-plugins/src/manifest.rs",
        "crates/ragent-agent/src/agent/oasf.rs",
        "crates/ragent-agent/src/agent/custom.rs",
        "crates/ragent-agent/src/hooks/mod.rs",
        "crates/ragent-agent/src/skill/loader.rs",
    ];
    let mut checked: Vec<&str> = surfaces.to_vec();
    let connector_sources = production_sources_under("crates/ragent-connectors/src");
    let connector_relatives: Vec<String> = connector_sources
        .iter()
        .map(|path| {
            path.strip_prefix(workspace_root())
                .expect("path under root")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    checked.extend(connector_relatives.iter().map(String::as_str));

    let mut offenders = Vec::new();
    let mut inspected = 0usize;
    for relative in checked {
        let source = read(relative);
        let candidates = declared_field_names(&source)
            .into_iter()
            .chain(serde_name_attributes(&source));
        for (line, name) in candidates {
            inspected += 1;
            if !is_rename_spelling(&name) || exemption_for(relative, &name).is_some() {
                continue;
            }
            offenders.push(format!("{relative}:{line} declares `{name}`"));
        }
    }
    assert!(
        inspected > 100,
        "the dynamic-source scan found only {inspected} settings; the predicate has \
         been disarmed, not satisfied"
    );
    assert!(
        offenders.is_empty(),
        "no plugin, connector, agent, hook or skill setting may be named a rename \
         spelling (FR-031, FR-032):\n  {}",
        offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// The audit's own invariant: the exemptions are reviewed, not accumulated
// ---------------------------------------------------------------------------

#[test]
fn test_escape_hatch_exemptions_are_few_and_each_names_a_rename_spelling() {
    // An exemption for a name that is not a rename spelling is stale, and a
    // growing list means the scan is being defeated rather than satisfied. The
    // audit found exactly one, and FR-031 allows no others today.
    assert_eq!(
        EXEMPTIONS.len(),
        1,
        "the escape-hatch audit's exemption list is expected to hold exactly one \
         entry; review any addition before widening it"
    );
    for (file, name, reason) in EXEMPTIONS {
        assert!(
            is_rename_spelling(name),
            "the exemption for `{name}` in {file} is stale: that spelling is not in \
             the FR-023 table"
        );
        assert!(
            !reason.trim().is_empty(),
            "the exemption for `{name}` in {file} must carry the written reason it is \
             not a compatibility switch"
        );
        assert!(
            workspace_root().join(file).exists(),
            "the exemption names {file}, which does not exist"
        );
    }
}

// ---------------------------------------------------------------------------
// A retired spelling still resolves to nothing through the shipped surfaces
// ---------------------------------------------------------------------------

#[test]
fn test_a_retired_spelling_never_resolves_through_the_builtin_table() {
    // The built-in table is the one table a config can switch on without writing
    // an entry (`tools.aliases: true`), so it is the one place a retired spelling
    // could be reintroduced by ragent's own source rather than by a user's file.
    use ragent_agent::tool::alias_table::{AliasTable, resolve_inbound_tool_name};
    use ragent_agent::tool::create_default_registry;

    let registry = create_default_registry();
    let table: ToolsConfig = serde_json::from_str(r#"{"aliases": true}"#)
        .expect("the boolean form of tools.aliases parses");
    let table = AliasTable::from_tools_config(&table);
    for row in &TOOL_RENAMES {
        assert_eq!(
            resolve_inbound_tool_name(&registry, &table, row.before),
            None,
            "the built-in alias table resolves the retired spelling `{}` onto a tool \
             (it must resolve onto `{}` only through the registry, FR-031)",
            row.before,
            row.after
        );
        assert!(is_before(row.before), "the table row is a retired spelling");
    }
}
