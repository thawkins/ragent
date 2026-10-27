# T-032 - No alias survives the rename, on any inbound path (FR-026)

Status: **completed**. Requirement: FR-026 (spec `uniformtools`). Dependencies:
T-031 (the rename) and T-014 (the one dispatch pipeline).

## 1. What the requirement asks

FR-026 has three clauses, and each fails differently:

1. the tool is exposed under its `after` name **only** - `Tool::name()`, the
   registry map key, `definitions()`, the `/tools` listing, `tool_info`,
   `commands_info`, the `--dry-run` report and the TUI;
2. `before` is **not** registered as a second tool, and no forwarding wrapper or
   adapter stands under it;
3. a call or lookup using `before` **fails as an unknown tool** carrying the
   did-you-mean hint, and `before` is not accepted at any inbound tool-name
   resolution stage - on every inbound path: the TUI prompt, `ragent run`, the
   OpenAI-compatible `/v1` surface, and ACP.

Clause 1 is a surface property, clause 2 is a registry property, and clause 3 is
a behaviour that has to hold at four different entry points. The task is a
*proof*, not a behaviour change: `git diff` touches no production source file.

## 2. The state found, which settled the design

Three facts in the tree decided what could be asserted where, and two of them
contradicted the first draft of this file.

### 2.1 The case fold is permitted, and nine of the thirteen `before` names need it

FR-026's own case-folding note settles it:

> Matching an inbound name without regard to ASCII letter case (`bash` reaching
> `Bash`, `WRITE` reaching `Write`) is a spelling match between two forms of the
> **same** word ... Case folding therefore applies to the dispatch lookup only,
> and never to a surface FR-026 or FR-031 names.

Nine of the thirteen pairs differ only by letter case (`Bash`, `Edit`, `Glob`,
`Grep`, `Write`, and the four `Task*` names), so nine `before` spellings are
literally the lower-case form of their adopted name. The first draft asserted
that `canonical_dispatch_name` must return `None` for every retired spelling;
that contradicts the requirement it claims to enforce, and it fails on a correct
tree. The case now asserts the **permitted** arm for the nine case-only pairs and
the refusal for the four word-changing ones (`ask_user`, `plan_enter`,
`plan_exit`, `skill_manage`) - and clause 1's surface refusals stay exact.

The same reading explains the second surprise: `ToolRegistry::get` is an exact,
case-sensitive hash lookup, so it refuses `bash` even though the dispatch fold
accepts it. "The registry refuses it" and "the dispatch lookup folds it" are both
true and are different surfaces.

### 2.2 The wrapper checks are the whole of clause 2, because a grep cannot do it

Clause 2's "no forwarding wrapper or adapter" reads like a source scan, and the
first attempt tried one. It fails for a structural reason: the workspace's one
forwarding alias, `LegacyMultiEditAlias`
(`crates/ragent-agent/src/tool/mod.rs`), reports its name from a `&'static str`
**field** (`self.0`), so its file contains no call-shaped literal at all - the
retired spelling it returns is a value passed in at construction, and a scan
keyed on a quoted `before` literal finds nothing in a file that plainly contains
one.

Worse, and recorded because it cost the most time: a **seeded** wrapper
registered under a retired spelling survived five different scan shapes during
that attempt (literal needles, `fn name()` return shapes, `register(` call
shapes, wrapper-type names, and a whole-file match). The registry is where the
two spellings finally become indistinguishable, so the registry is where the
proof looks:

- case 3 compares the key set against the names the registered tools report, and
  compares each pair's two spellings for simultaneous resolution;
- case 8 seeds the defect (a `ForwardingStub` registered under `bash` beside a
  `NamedStub` registered under `Bash`) and proves the comparison sees it, so
  case 3 is not passing by agreeing with itself.

### 2.3 Every inbound surface already shares one path

- the TUI prompt: `crates/ragent-tui/src/app/session_ops.rs` calls
  `SessionProcessor::process_message`;
- `ragent run`: `src/main.rs`'s `Commands::Run` arm calls it - **not**
  `src/cli.rs`, which only parses the arguments;
- the `/v1` surface: `crates/ragent-server/src/routes/openai.rs` calls it for
  both `stream:false` and `stream:true`;
- ACP: `crates/ragent-agent/src/acp/server.rs` calls it from `session/prompt`.

So clause 3's "at every inbound path" holds by construction, and the case's job
is to pin the construction rather than re-test four transports. The
`multiedit` alias is the workspace's one forwarding wrapper and is deliberately
**not** an FR-023 row (it is `editrenewal`'s deprecation of the older
`multi_edit`/`multiedit` pair), so clause 2's wrapper checks are scoped to the
FR-023 pairs - and case 3 asserts that scoping out loud, so `multiedit` becoming
a rename row fails the file rather than silently widening it.

## 3. What this task added

One file, `crates/ragent-agent/tests/test_fr026_no_alias_survives.rs` (9 cases),
registered in `crates/ragent-agent/tests/all.rs`:

| # | Case | What it reads |
|---|---|---|
| 1 | `test_no_second_registration_stands_under_a_before_spelling` | duplicate keys, `get`, the key list, and the two halves of the case fold |
| 2 | `test_no_retired_spelling_is_a_registry_key_a_tool_name_or_a_definition` | `list`, `all_tools().name()`, `definitions()` |
| 3 | `test_no_tool_delegates_to_one_whose_name_pair_it_shares` | both spellings of every pair, and key-vs-reported-name |
| 4 | `test_the_alias_table_refuses_both_columns_of_every_rename_pair` | `AliasTable::from_aliases` with a `before` source, a `before` target, and a `before` source pointed elsewhere |
| 5 | `test_every_before_spelling_resolves_to_nothing_with_a_did_you_mean_hint` | the alias stage, the fold, `resolve_call`, `suggest_tool_name` |
| 6 | `test_the_agent_loop_refuses_a_before_call_and_appends_its_refusal` | a real `process_message` turn, auto-approval on |
| 7 | `test_the_inbound_paths_share_one_resolution_chain` | the four surfaces, `dispatch.rs` |
| 8 | `test_a_seeded_second_registration_is_detected` | the negative control for cases 1-3 |
| 9 | `test_the_builtin_alias_list_names_no_rename_spelling` | `tools.aliases: true` and the empty-object form |

Case 6 is the one that produces an observable rather than reading a source: a
scripted provider sends one retired-spelling tool call, and every later reply is
`done`. Two pairs are driven, one from each half of the table - `write` (differs
by case, so the loop canonicalises it and the record carries `Write`) and
`skill_manage` (differs in words, so the record carries `skill_manage` and the
refusal says "Unknown tool"). Both are refused, both carry
`ToolCallStatus::Error`, and nothing is written into the session directory.

Case 7's scan skips comment lines: several of these files explain in prose that
they delegate to the shared path, and a scan that read those sentences would
demand edits to the explanation.

## 4. Deliberately unchanged

- **No production source.** The proof found the requirements already met; the
  only source file it touched was during red-proof work and was restored (see
  section 5).
- **`LegacyMultiEditAlias`** (`crates/ragent-agent/src/tool/mod.rs`). It is a
  forwarding wrapper, and clause 2 forbids forwarding wrappers - for the FR-023
  rows. `multiedit` is not one, and retiring it is `editrenewal`'s decision, not
  this spec's.
- **The `tools.aliases` built-in list.** Case 9 proves it names neither column of
  any pair; it was already correct (`BUILTIN_PROVIDER_ALIASES` holds only the
  four provider spellings ragent did not adopt).
- **`scripts/check-tool-rename.sh`.** The literal half of the no-alias rule
  (FR-027, NFR-008 predicates (d) and (e)) belongs to T-034 and T-047; this file
  adds the behavioural half those scans cannot see.

## 5. Red proofs, and the ones that did not come out

Three proofs were attempted against production source. All three were restored
byte-identically, and two of them **failed to prove anything**, which is
recorded here rather than quietly dropped.

1. **A forwarding wrapper under a retired spelling** - `create_default_registry`
   registered a wrapper reporting `write`. **The file stayed green.** Root cause:
   `ToolRegistry` is a map keyed by a `BTreeMap`-ordered name list built at
   registration, and the wrapper's own `name()` is not the key; the wrapper's key
   is `multiedit`, so two wrappers collapse onto one key. Five successive
   scan shapes (literal needles, `fn name()` return shapes, `register(` shapes,
   wrapper type names, whole-file matching) never made this red. This is the
   finding that moved case 3 from a source scan to a registry comparison.
2. **A second registration seeded at the top of `create_default_registry`** -
   three different seeds (`register(UpdateFileTool)`, `register(AskUserTool)`,
   `register(PlanExitTool)`) each left the name list at 148, because the name was
   already present. The one seed that *should* have been red - a bespoke
   `NamedStub("bash")` - measured as 148 -> 149 in a standalone example but was
   **not reflected in the test run** (the test binary stayed green across several
   minutes of rebuilds). The seed never became a usable proof, so it is not
   claimed; case 8 exists because of it and proves the same property in a
   registry the test owns.
3. **The negative control in the other direction** - case 8's seeded registry
   (two `NamedStub`s, and a `ForwardingStub` over an adopted tool) *is* the
   positive result: it demonstrates that the comparison cases 1-3 make detects a
   second registration and a wrapper, so those cases are not vacuous.

What was proven instead, and is worth as much: the two failures above are
measurements, not guesses, and they are why this file's clause-2 half is a
registry assertion with a seeded control rather than a grep.

## 6. Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo check --workspace --all-targets` | clean (1m 01s) |
| `cargo clippy -p ragent-agent --all-targets` | names no touched file (one `unnecessary_literal_bound` was found and fixed by reading the stub's own field) |
| `cargo nextest run -p ragent-agent -E 'test(test_fr026_no_alias_survives::)'` | 9 passed |
| `cargo nextest run -p ragent-agent --no-fail-fast` | 2144 passed, 1 skipped, 0 failed |
| guard scripts | 12 of 13 exit 0; `check-bench-regression` exits 2 with no `target/criterion` (pre-existing) |
| production diff | none - `git diff` on `crates/ragent-agent/src/` shows only T-031's uncommitted rename |

## 7. Hand-off

- **T-047** (NFR-008) owns the literal half: predicates (d) registry key and (e)
  non-test literal are the guard's, and predicate (b) "a `before` spelling
  appears in a resolver match arm" is now also pinned behaviourally here, so a
  resolver arm that translated a retired name would fail both files.
- **NFR-008 predicate (g)** ("a registered tool's `execute` delegates wholly to
  another registered tool's `execute`") is the general form of clause 2. Case 3
  proves it for the FR-023 pairs only, because the general form is false on this
  tree by design (`multiedit`). Anyone implementing (g) has to decide what
  `multiedit` means first, and this document is the record of why.
- **`specs/uniformtools/TESTPLAN.md`** TC-028 is the manual counterpart; its
  "Documentation half" style table now has an automated half for this task
  (section 3).
