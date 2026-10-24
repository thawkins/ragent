//! The `tools` section of `ragent.json` (spec `uniformtools` FR-015, FR-017).
//!
//! Three opt-in switches, all inert at their defaults so a config file written
//! before this section existed keeps behaving exactly as it did:
//!
//! ```jsonc
//! {
//!   "tools": {
//!     "aliases": { "WebFetch": "webfetch" },
//!     "expose_alias_names": true,
//!     "wire_trace": false
//!   }
//! }
//! ```
//!
//! - [`ToolsConfig::aliases`] (FR-015) declares provider tool spellings that
//!   ragent resolves onto its own tool names before the unknown-tool check. It
//!   accepts either a boolean - `true` requests the built-in table - or an
//!   object mapping a provider spelling to a ragent tool name.
//! - [`ToolsConfig::expose_alias_names`] (FR-015) additionally advertises each
//!   alias spelling in the outbound tool list, so a model that only knows the
//!   provider spelling can still see it. Never advertises a retired ragent
//!   spelling (FR-031): the renamed tools are registered under those spellings,
//!   so they resolve by exact lookup and have no alias entry to advertise.
//! - [`ToolsConfig::wire_trace`] (FR-017) writes each step's request body,
//!   redacted, to `target/temp/wire-trace/<session>-<step>.json`.
//!
//! ## Four values, four meanings
//!
//! `tools.aliases` is deliberately not a plain `Option<bool>`: the key can be
//! absent, `true`, `false`, or an object (possibly empty), and all four say
//! something different. Absent means "this layer has no opinion"; `true` asks
//! for the built-in provider-spelling table; `false` switches resolution off and
//! drops whatever a lower layer contributed; an object contributes its entries.
//! Collapsing `{}` into `false` would make a project file that merely declares
//! an empty table silently wipe the user-global one, and would rewrite the
//! author's `{}` as `false` on a round-trip.
//!
//! ## Nothing here is an escape hatch
//!
//! The section resolves *names*, not capabilities: an alias renames the tool a
//! model already asked for, and the permission layer, the loop restriction and
//! the 7-layer bash validation all still run on the resolved call. That is why
//! the section is merged from a project config like any other non-privileged
//! one rather than being stripped by
//! [`merge_project`](crate::Config::merge_project) - the refusal the rename
//! needs is a per-entry load-time rejection of a retired spelling (FR-031),
//! not a blanket ban on the section.

use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The config path this section is read from, used verbatim in load errors so a
/// reader is sent to the key they wrote.
pub const TOOLS_ALIASES_KEY: &str = "tools.aliases";

/// The `tools` section of `ragent.json` (FR-015, FR-017).
///
/// Every field defaults to "off", and the whole section is omitted from a
/// serialised config while it is at those defaults, so a build that has never
/// seen this section writes a byte-identical `ragent.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Provider tool spellings resolved onto ragent tool names (FR-015).
    ///
    /// Omitted when the user never set the key, so the default config carries
    /// no alias state at all.
    #[serde(default, skip_serializing_if = "AliasesConfig::is_unspecified")]
    pub aliases: AliasesConfig,
    /// Advertise each alias spelling in the outbound tool list (FR-015).
    ///
    /// Default `false`: the model sees only the names the registry holds.
    #[serde(default, skip_serializing_if = "is_false")]
    pub expose_alias_names: bool,
    /// Write the redacted per-step request body under `target/temp/wire-trace/`
    /// (FR-017).
    ///
    /// Default `false`. Inert while absent or `false`, and it never touches a
    /// provider or a tool - it only observes the request the loop was about to
    /// send.
    #[serde(default, skip_serializing_if = "is_false")]
    pub wire_trace: bool,
}

impl ToolsConfig {
    /// `true` when every field is at its default, so the `tools` key can be
    /// omitted from a serialised config.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.aliases.is_unspecified() && !self.expose_alias_names && !self.wire_trace
    }

    /// Merge `other` (the higher-precedence overlay) into `self`.
    ///
    /// The two booleans are opt-in capabilities, so they use OR semantics (the
    /// same rule `sdd` and `piegap` use): a layer that enables one cannot have
    /// it silently turned off by a layer that leaves it at the default. Alias
    /// entries are unioned key-by-key with the overlay winning a collision,
    /// matching the backend registry's per-id replacement, so a later config
    /// file can add one spelling or re-point one without discarding the rest.
    /// The one exception is an overlay that writes `aliases: false`, which turns
    /// alias resolution off and discards the inherited entries - see
    /// [`AliasesConfig`] for why an explicit off has to win.
    pub fn merge(&mut self, other: &Self) {
        self.aliases.merge_with(&other.aliases);
        self.expose_alias_names |= other.expose_alias_names;
        self.wire_trace |= other.wire_trace;
    }

    /// Refuse every `tools.aliases` entry that names a retired ragent tool
    /// spelling (FR-031).
    ///
    /// Called by the config loader once each configuration layer has parsed, so
    /// a refused entry stops the load with a message naming the file it came
    /// from. The check reads the FR-023 table
    /// ([`ragent_types::tool_rename::TOOL_RENAMES`]) rather than a local copy, so
    /// the refusal and the rename cannot drift apart.
    ///
    /// Only the *entries* form can carry a spelling: the boolean forms (`true`
    /// requests the built-in table, `false` switches resolution off) hold no
    /// per-entry data, so they are always accepted. The built-in table itself
    /// holds no renamed spelling, which is what
    /// `crates/ragent-agent/tests/test_tool_alias_table.rs` asserts.
    ///
    /// Returns every offending entry rather than the first, so an author with a
    /// table written from an old `--dry-run` dump is told all of it at once.
    /// Empty means the section is acceptable.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_config::ToolsConfig;
    ///
    /// let tools: ToolsConfig = serde_json::from_str(r#"{"aliases": {"bash": "Bash"}}"#)?;
    /// let offenders = tools.validate_alias_spellings();
    /// assert_eq!(offenders.len(), 1);
    /// assert_eq!(offenders[0].spelling, "bash");
    /// assert_eq!(offenders[0].after, "Bash");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn validate_alias_spellings(&self) -> Vec<RetiredAliasSpelling> {
        let mut offenders = Vec::new();
        for (key, target) in self.aliases.entries() {
            // The self-mapping form asks for two spellings of one tool, which
            // FR-026 forbids for a renamed tool. It is checked before the two
            // single-column rules so a `"Bash": "Bash"` entry is described as a
            // self-mapping rather than as a retired key.
            if key == target {
                if let Some(after) = after_spelling_of(key) {
                    offenders.push(RetiredAliasSpelling {
                        key: key.clone(),
                        target: target.clone(),
                        spelling: key.clone(),
                        after,
                        in_source_key: true,
                        self_mapping: true,
                    });
                }
                continue;
            }
            // A source key equal to a retired `before` spelling.
            if let Some(after) = ragent_types::tool_rename::Rename::after_of(key) {
                offenders.push(RetiredAliasSpelling {
                    key: key.clone(),
                    target: target.clone(),
                    spelling: key.clone(),
                    after,
                    in_source_key: true,
                    self_mapping: false,
                });
                continue;
            }
            // A target value equal to a retired `before` spelling.
            if let Some(after) = ragent_types::tool_rename::Rename::after_of(target) {
                offenders.push(RetiredAliasSpelling {
                    key: key.clone(),
                    target: target.clone(),
                    spelling: target.clone(),
                    after,
                    in_source_key: false,
                    self_mapping: false,
                });
            }
        }
        offenders
    }
}

/// The `after` spelling a renamed tool's spelling maps onto, from **either**
/// column of the FR-023 table (FR-031's self-mapping rule).
///
/// A self-mapping of a retired spelling (`"bash": "bash"`) and one of an adopted
/// spelling (`"Bash": "Bash"`) are both refused, and both are sent to the
/// adopted spelling - which is the name itself in the second case.
fn after_spelling_of(name: &str) -> Option<&'static str> {
    ragent_types::tool_rename::TOOL_RENAMES
        .iter()
        .find(|pair| pair.before == name || pair.after == name)
        .map(|pair| pair.after)
}

/// `skip_serializing_if` helper for the opt-in booleans above.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The `tools.aliases` value (FR-015).
///
/// Two input forms, both exact-match and case-sensitive:
///
/// - a **boolean**: `true` requests the built-in table of provider spellings
///   (the spellings ragent did not adopt, such as `WebFetch`), `false` leaves
///   alias resolution off;
/// - an **object**: provider spelling to ragent tool name, e.g.
///   `{ "WebFetch": "webfetch" }`.
///
/// Both may be in force at once after a merge (a user-global `true` plus a
/// project entry), which is why this is a small struct rather than a two-variant
/// enum. An entry whose value is not a string, or a value that is neither a
/// boolean nor an object, is a load error naming the accepted shapes - a typo
/// must not silently disable alias resolution.
///
/// The entries form is additive across layers, with one deliberate exception: an
/// explicit `false` in a higher-precedence layer turns resolution off and drops
/// every inherited entry. `false` is the only way a config author can say "I do
/// not want a table", and an unconditional union would make a user-global `true`
/// impossible to switch off from the project file - the same trap the
/// `sdd`/`piegap` OR rules avoid by being opt-in in the first place. An empty
/// object is *not* that switch: `{}` contributes no spelling and clears nothing,
/// and it round-trips as `{}` rather than being rewritten as `false`.
///
/// This type validates the *shape* of an entry - a spelling mapping to a
/// string. The *name* check is FR-031's and is a separate step,
/// [`ToolsConfig::validate_alias_spellings`], run by the config loader once the
/// shape has parsed: a source key or a target value that is a retired ragent
/// spelling (or a self-mapping of either spelling of a renamed tool) is a load
/// error naming the file, the spelling and its `after` replacement. Keeping the
/// two apart is deliberate - a deserialiser cannot name the file it is reading,
/// and keeping the shape pure leaves `serde_json::from_str::<ToolsConfig>`
/// usable in a test that must inspect a *rejected* entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasesConfig {
    /// `true` when the key was present in the source JSON, in either form.
    specified: bool,
    /// `true` when the boolean `true` form was used: consult the built-in table.
    builtin: bool,
    /// `true` when the value was the boolean `false`: "resolve no aliases".
    ///
    /// Tracked separately from "no entries" because `false` and `{}` mean
    /// different things: `false` is the author switching the feature off (and
    /// therefore clearing whatever a lower layer contributed), while `{}` is an
    /// empty table that merely contributes no spelling of its own. Collapsing
    /// the two would let a project file that writes `{}` silently discard a
    /// user-global table, and would make `{}` round-trip as `false`.
    explicit_off: bool,
    /// `true` when the value was an object, so an empty one round-trips as `{}`
    /// rather than as `false`.
    was_object: bool,
    /// Explicit provider-spelling entries, ascending by spelling.
    entries: BTreeMap<String, String>,
}

impl AliasesConfig {
    /// `true` when the key was absent from the source JSON, so it round-trips as
    /// absent rather than as an explicit `false`.
    #[must_use]
    pub fn is_unspecified(&self) -> bool {
        !self.specified
    }

    /// `true` when alias resolution is on: the built-in table was requested, or
    /// at least one entry was configured.
    ///
    /// `false` means inbound names resolve against the registry alone, which is
    /// the pre-existing behaviour FR-015 requires when neither switch is set.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.builtin || !self.entries.is_empty()
    }

    /// `true` when the built-in table of provider spellings was requested.
    #[must_use]
    pub fn uses_builtin_table(&self) -> bool {
        self.builtin
    }

    /// The configured provider-spelling entries, ascending by spelling.
    ///
    /// Empty for the boolean form, which carries no per-entry data of its own.
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    /// Merge `other` (the higher-precedence overlay) into `self`.
    ///
    /// An overlay that explicitly writes `false` turns alias resolution off for
    /// the merged config and clears every entry: a layer that says "no aliases"
    /// is stating a preference for this build, and silently keeping a
    /// lower-precedence layer's table would make the key impossible to disable
    /// from the file that sets it. Any other overlay unions: the built-in
    /// request is OR'd (a layer that enables it keeps it on) and entries are
    /// unioned per spelling with the overlay winning a collision.
    ///
    /// An overlay that omits the key changes nothing, and an empty object `{}`
    /// contributes no entry rather than clearing the inherited ones - only
    /// `false` is the switch that turns resolution off.
    fn merge_with(&mut self, other: &Self) {
        if other.explicit_off {
            self.specified = true;
            self.builtin = false;
            self.explicit_off = true;
            self.was_object = false;
            self.entries.clear();
            return;
        }
        if !other.specified {
            return;
        }
        self.specified = true;
        self.explicit_off = false;
        self.builtin |= other.builtin;
        self.was_object |= other.was_object;
        for (spelling, target) in &other.entries {
            self.entries.insert(spelling.clone(), target.clone());
        }
    }
}

impl<'de> Deserialize<'de> for AliasesConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Bool(builtin) => Ok(Self {
                specified: true,
                builtin,
                explicit_off: !builtin,
                was_object: false,
                entries: BTreeMap::new(),
            }),
            Value::Object(map) => {
                let mut entries = BTreeMap::new();
                for (spelling, target) in map {
                    let Some(target) = target.as_str() else {
                        return Err(D::Error::custom(format!(
                            "{TOOLS_ALIASES_KEY}: entry {spelling:?} must map to a ragent tool \
                             name (a string), found {}",
                            json_kind(&target)
                        )));
                    };
                    entries.insert(spelling, target.to_string());
                }
                Ok(Self {
                    specified: true,
                    builtin: false,
                    explicit_off: false,
                    was_object: true,
                    entries,
                })
            }
            other => Err(D::Error::custom(format!(
                "{TOOLS_ALIASES_KEY} must be a boolean (true enables the built-in \
                 provider-spelling table) or an object mapping provider spellings to ragent \
                 tool names, such as {{\"WebFetch\": \"webfetch\"}}; found {}",
                json_kind(&other)
            ))),
        }
    }
}

impl Serialize for AliasesConfig {
    /// Emits the form the section was written in: the entries object when any
    /// entry is configured, otherwise the boolean - except that an empty
    /// object stays an empty object, so `{}` and `false` remain distinguishable
    /// through a round-trip.
    ///
    /// A merged state that carries *both* the built-in request and explicit
    /// entries serialises as the entries object, because one JSON key cannot
    /// express both. That state only arises from a multi-layer merge, and this
    /// rendering is read by `/config show` and the redacting `Debug` impl; it is
    /// never written back over a user's file, which is persisted from its raw
    /// bytes.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !self.entries.is_empty() {
            self.entries.serialize(serializer)
        } else if self.builtin {
            true.serialize(serializer)
        } else if self.was_object {
            self.entries.serialize(serializer)
        } else {
            false.serialize(serializer)
        }
    }
}

/// One `tools.aliases` entry that the FR-023 rename forbids (FR-031).
///
/// FR-031 is an *unwanted-behaviour* requirement: no configuration may make a
/// retired ragent tool spelling resolvable. `tools.aliases` is the one section
/// that can carry a tool name, so its entries are checked when the
/// configuration loads, and an offending entry stops the load rather than being
/// dropped quietly - a silently ignored compatibility entry is exactly the
/// "renamed back by another route" the requirement exists to prevent.
///
/// Three forms are refused, and each names the `after` spelling so the reader is
/// sent to the name that does exist:
///
/// - a **source key** that is a retired `before` spelling (`"bash": "Bash"`);
/// - a **target value** that is a retired `before` spelling (`"WebFetch":
///   "bash"`), because pointing an alias at a dead name resolves nothing and
///   hides the mistake behind an unknown-tool error at call time;
/// - the **self-mapping** of either spelling of a renamed tool (`"Bash":
///   "Bash"`), which asks for two spellings of one tool - the second spelling
///   FR-026 forbids.
///
/// A self-mapping of a spelling the rename never touched (`"think": "think"`)
/// is *not* an error: it asks for no second spelling of a renamed tool and the
/// alias table drops it as a no-op (D7). The distinction is why this is a
/// rename-table lookup rather than a `key == value` test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetiredAliasSpelling {
    /// The entry's source key, exactly as the configuration wrote it.
    pub key: String,
    /// The entry's target value, exactly as the configuration wrote it.
    pub target: String,
    /// The offending spelling - `key` or `target`.
    pub spelling: String,
    /// The `after` spelling that replaced `spelling`.
    pub after: &'static str,
    /// `true` when the offending spelling is the source key.
    pub in_source_key: bool,
    /// `true` for the self-mapping form.
    pub self_mapping: bool,
}

impl RetiredAliasSpelling {
    /// The sentence this entry produces, without the file name (the loader
    /// prepends that, so one rendering serves the file and environment paths).
    #[must_use]
    pub fn reason(&self) -> String {
        if self.self_mapping {
            format!(
                "{TOOLS_ALIASES_KEY} entry {:?} maps {:?} onto itself; that is a renamed \
                 ragent tool, so it resolves by exact registry lookup and may not carry a \
                 second spelling. The adopted spelling is {:?}. Delete the entry.",
                self.key, self.spelling, self.after
            )
        } else if self.in_source_key {
            format!(
                "{TOOLS_ALIASES_KEY} entry {:?} uses the retired ragent tool spelling {:?} as \
                 its source key; that tool was renamed to {:?}. Use {:?} as the key, or a \
                 provider spelling ragent never registered.",
                self.key, self.spelling, self.after, self.after
            )
        } else {
            format!(
                "{TOOLS_ALIASES_KEY} entry {:?} maps onto the retired ragent tool spelling \
                 {:?}; that tool was renamed to {:?}. Point the alias at {:?}, or at another \
                 registered ragent tool.",
                self.key, self.spelling, self.after, self.after
            )
        }
    }
}

impl std::fmt::Display for RetiredAliasSpelling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason())
    }
}

impl std::error::Error for RetiredAliasSpelling {}

/// Name of a JSON value's kind, for the "found ..." clause of a load error.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}
