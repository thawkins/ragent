# T-029 - Rename set derived from `tools.xlsx` (FR-024, FR-025)

**Spec ID:** uniformtools
**Task:** T-029
**Requirements:** FR-024 (two-sheet exact-match rule), FR-025 (exact-spelling tie-break)
**Status:** completed
**Dependencies:** none (root task)
**Verified against:** `tools.xlsx`, `sha256 ad2a4574...0ba6bdae` (section 1)

This document is the recorded derivation the specification requires. It is the
input to T-030 (which commits the pairs as `crates/ragent-agent/src/tool/rename.rs`)
and the comparison baseline for TC-019 in `TESTPLAN.md`.

Nothing here is a judgement call: every accept and every refusal below is the
mechanical result of applying FR-024 and FR-025 to the workbook as delivered.

## Result

**13 pairs accepted, 2 targets refused, 15 candidates total.** Ascending by
`after`:

```
 1  ask_user     -> AskUserQuestion
 2  bash         -> Bash
 3  edit         -> Edit
 4  plan_enter   -> EnterPlanMode
 5  plan_exit    -> ExitPlanMode
 6  glob         -> Glob
 7  grep         -> Grep
 8  skill_manage -> Skill
 9  task_create  -> TaskCreate
10  task_get     -> TaskGet
11  task_list    -> TaskList
12  task_update  -> TaskUpdate
13  write        -> Write
```

The numbers are the position in `TOOL_RENAMES`, i.e. the ascending-`after` order
FR-023 requires. Nine of the thirteen differ only by case; `AskUserQuestion`,
`EnterPlanMode`, `ExitPlanMode` and `Skill` change the words.

Refused by FR-025 (two claimants each, neither spelling matches):

```
WebFetch  <- mf_fetch, webfetch
WebSearch <- mf_search, websearch
```

Excluded by FR-024: 20 pairs where the Anthropic row is not `Exact` (including
`new_agent` -> `Agent` and `read` -> `Read`), 40 pairs where the ragent row is
not `Exact` (including the five `cron_*` tools), and 121 entries that name no
Anthropic tool at all. `TodoWrite` is `Exact` on the Anthropic sheet and named by
no ragent row. Details and per-clause accounting follow.

Read section 3 first if you are checking the arithmetic: it closes the books
twice, once over the 215 `(row, token)` pairs and once over the 148 ragent rows,
and explains why the two views differ by two rows.

---

## 1. Workbook facts

Source: `tools.xlsx`, repository root, sheets `Anthropic Tools` and
`ragent Tools` only. No other sheet is consulted.

| Sheet | Rows | Columns used | Notes |
|---|---|---|---|
| `Anthropic Tools` | 46 | `Tool` (A), `Mapping` (E) | 19 rows carry `Mapping = Exact`; the rest are `Close`, `Partial` or `No equivalent` |
| `ragent Tools` | 148 | `Tool` (A), `Anthropic equivalent` (E), `Mapping` (F) | `Exact` 24, `Close` 55, `Partial` 2, `Ragent-only` 67 |

Both sheets have unique tool names (0 duplicates in either). `Anthropic
equivalent` is a free-text cell whose `/`-separated entries are the candidate
targets; it is not restricted to Anthropic tool names (see section 7.3).

The workbook this derivation reads is
`sha256 ad2a4574bb8656d053970cbb77e19ccfe70d16efcb8557c6e24ad49d0ba6bdae`,
56,376 bytes, 7 sheets. It is a working-tree artefact, not a tracked file, so a
re-run may find a newer one; confirm the digest before comparing. A different
`tools.xlsx` can legitimately produce a different table, which is why TC-019
checks the fingerprint before it checks the pairs.

## 2. The rule, restated as an algorithm

A pair `(before, after)` enters the table only if all three clauses hold
(FR-024):

- **Clause A** - the `Anthropic Tools` row whose `Tool` is `after` carries
  `Mapping = Exact`;
- **Clause B** - the `ragent Tools` row whose `Tool` is `before` carries
  `Mapping = Exact`;
- **Clause C** - that ragent row's `Anthropic equivalent` cell names `after` as
  one of its `/`-separated entries, character for character.

Then, for each surviving `after` (FR-025):

- exactly one claimant -> rename it;
- two or more claimants -> rename the claimant whose current name equals
  `after` once each `_`-separated segment is capitalised (`bash` -> `Bash`);
  the other claimants keep their current names;
- two or more claimants and no claimant passes that test -> rename none of
  them, and record the refusal with the competing names.

Note the scope of the tie-break: FR-025 governs *collisions*. A target claimed
by a single ragent tool is renamed even though its name does not match the
target's spelling - that is how `ask_user` -> `AskUserQuestion`,
`plan_enter` -> `EnterPlanMode`, `plan_exit` -> `ExitPlanMode` and
`skill_manage` -> `Skill` survive. Applying the capitalisation test to
singletons as well would wrongly reduce the table to 9 pairs.

### 2.1 Tokenisation - entries are trimmed, and that is load-bearing

Clause C compares names "character for character", but the cell is written with
padding around the separators: 53 of the 148 ragent rows carry a cell of the
form `Edit / Write` or `CronCreate / CronDelete / CronList`, so the raw split
yields `" Write"` with a leading space. Each entry is therefore `trim()`ed
before it is compared, and nothing else is normalised - no case folding, no
prefix matching, no punctuation stripping.

The trimming is not cosmetic. Re-deriving without it drops **four** pairs,
because the four `task_*` rows are the only `Exact` ragent rows whose cell has
three separators and therefore padded middle entries:

```
# the section 8 script, with `p.strip()` replaced by `p` in the candidate loop
$ python3 derive.py
34 22 15
[... 13 pairs ...] 13
[... 2 refusals ...] 2

$ python3 derive-untrimmed.py     # no .strip() on the split entries
[... 9 pairs ...] 9
```

The four lost pairs are `task_create` -> `TaskCreate`, `task_get` -> `TaskGet`,
`task_list` -> `TaskList` and `task_update` -> `TaskUpdate`.

The numbers behind that: 53 of the 148 rows pad their entries, and of the 215
tokens only **37** name an `Anthropic Tools` row before trimming against **94**
after. Trimming rescues 57 tokens, and the four it rescues into an `Exact` pair
are `TaskCreate`, `TaskGet`, `TaskList` and `TaskUpdate` on the four `task_*`
rows. A re-derivation that reads the workbook with a different library must trim
the same way, or it will report 9 pairs and call the table wrong.

## 3. Accounting - every `(row, token)` pair from `ragent Tools`

Each ragent row contributes one pair per `/`-separated entry in its
`Anthropic equivalent` cell: **215 pairs in total**. Each falls into exactly
one bucket, and the buckets sum to the total.

| Bucket | Pairs | Distinct tools | Meaning |
|---|---|---|---|
| Clause C fails | 121 | 90 | the entry is not the name of any `Anthropic Tools` row |
| Clause A fails | 20 | 20 | the entry names an Anthropic row whose `Mapping` is not `Exact` |
| Clause B fails | 40 | 18 | the entry names an `Exact` Anthropic row, but the ragent row itself is not `Exact` |
| Survives all three | 34 | 22 | 34 pairs over **15 distinct targets** |
| **Total** | **215** | - | |

The 34 surviving pairs reduce to 15 targets, then to 13 renames and 2 refusals.

The buckets are disjoint by construction, which is what makes the pair column
sum to 215. A pair is counted against exactly one clause, decided in this order:
the entry names no `Anthropic Tools` row -> clause C; else the Anthropic target
is not `Exact` -> clause A; else the ragent row is not `Exact` -> clause B. So a
`Close` ragent row that names a `Close` target is counted under clause A, not
under both. The distinct-tool columns inherit that: they count the tools that
were refused *at* each clause, not every tool that could have failed it.

### 3.1 The same accounting seen from the row

The pair table answers "why was this candidate dropped". The complementary view -
every one of the 148 rows and the one reason it settles on - reconciles to the
same 148 and is what a reviewer should check first, because a row missing from
both tables is a row nobody accounted for:

| Row outcome | Rows | Section |
|---|---|---|
| clause C - names no Anthropic tool | 90 | 7.3 |
| clause A - Anthropic target is not `Exact` | 18 | 7.1 |
| clause B - its own ragent row is not `Exact` | 18 | 7.2 |
| renamed (one of the 13) | 13 | 4 |
| claimant of a refused target | 4 | 5 |
| loses a collision to a winning claimant | 5 | 6 |
| **Total** | **148** | |

The two views differ by exactly two rows: `team_read_messages` and `wait_agents`
appear under clause A at pair level (each contributes one pair naming a `Close`
Anthropic tool) and under clause C at row level (each also contributes one prose
entry, and the row is counted once, at its first failing clause). That is why
section 7.1's pair/tool column reads 20/20 while this table reads 18 - and why
both are right.

Per-tool clause-A refusals, from this row view:

| Anthropic target | Its mapping | Rows that fail clause A on it |
|---|---|---|
| `Agent` | `Close` | `new_agent` |
| `EndConversation` | `Close` | `agent_complete` |
| `LSP` | `Close` | the ten `codeindex_*` tools |
| `ListAgents` | `Close` | `list_agents` |
| `Monitor` | `Partial` | `bg` |
| `Read` | `Close` | `pdf_read`, `read` |
| `SendMessage` | `Close` | `team_message` |
| `TaskStop` | `Close` | `cancel_agent` |
| **Total** | | **18** |

`wait_agents` and `team_read_messages` complete section 7.1's 20 pairs; they are
absent here because their prose entry fails clause C first.

## 4. The accepted pairs (13)

Ascending by `after`, which is the order FR-023 requires and the order T-030
commits. Every row's `Anthropic Tools` mapping is `Exact`, its `ragent Tools`
mapping is `Exact`, and the ragent cell names the target exactly.

| # | Before | After | Claimants for `after` | How selected |
|---|---|---|---|---|
| 1 | `ask_user` | `AskUserQuestion` | 1 | sole claimant |
| 2 | `bash` | `Bash` | 2 | exact spelling; `bash_reset` keeps its name |
| 3 | `edit` | `Edit` | 3 | exact spelling; `apply_patch`, `patch` keep their names |
| 4 | `plan_enter` | `EnterPlanMode` | 1 | sole claimant |
| 5 | `plan_exit` | `ExitPlanMode` | 1 | sole claimant |
| 6 | `glob` | `Glob` | 1 | sole claimant |
| 7 | `grep` | `Grep` | 1 | sole claimant |
| 8 | `skill_manage` | `Skill` | 1 | sole claimant |
| 9 | `task_create` | `TaskCreate` | 4 | exact spelling |
| 10 | `task_get` | `TaskGet` | 4 | exact spelling |
| 11 | `task_list` | `TaskList` | 4 | exact spelling |
| 12 | `task_update` | `TaskUpdate` | 4 | exact spelling |
| 13 | `write` | `Write` | 3 | exact spelling; `create`, `update_file` keep their names |

This reproduces the FR-023 table in `SPEC.md` row for row, in the same order.

## 5. The refused targets (2)

Both survive FR-024 and then fail FR-025: two claimants each, and neither
claimant's capitalised form equals the target.

| Target | Claimants | Test applied | Outcome |
|---|---|---|---|
| `WebFetch` | `mf_fetch`, `webfetch` | `mf_fetch` -> `MfFetch`, `webfetch` -> `Webfetch`; neither is `WebFetch` | rename neither; both keep their names |
| `WebSearch` | `mf_search`, `websearch` | `mf_search` -> `MfSearch`, `websearch` -> `Websearch`; neither is `WebSearch` | rename neither; both keep their names |

The refusal is deliberate and is not an oversight to be fixed later: FR-026
forbids an alias, so `WebFetch` and `WebSearch` cannot be reserved for a
provider spelling by renaming a non-matching tool to them. They remain provider
spellings that the narrowed D7 alias table (T-019, pruned by T-033) resolves,
and that is the only route by which a Claude skill naming `WebFetch` reaches
`webfetch`.

## 6. Claimants left under their current names by the tie-break (5)

Nine ragent tools lose at least one target to a *winning* claimant. Four of them -
`task_create`, `task_get`, `task_list`, `task_update` - lose three sibling
`Task*` targets each but win their own spelling, so they are renamed anyway
(rows 9-12 above). The other five are the losing claimants in the `Bash`, `Edit`
and `Write` collisions, and each loses its only claim. (Four further tools -
`mf_fetch`, `webfetch`, `mf_search`, `websearch` - also end up under their current
names, but they lose to a *refusal* rather than to a sibling: section 5.)

The five that lose every target they claim, and therefore keep the name they ship
today, are:

| Tool | Claimed | Lost to | Its own ragent row |
|---|---|---|---|
| `apply_patch` | `Edit` | `edit` | `Exact` (so clause B is not why it stays) |
| `bash_reset` | `Bash` | `bash` | `Exact` |
| `create` | `Write` | `write` | `Exact` |
| `patch` | `Edit` | `edit` | `Exact` |
| `update_file` | `Write` | `write` | `Exact` |

All five are `Exact` on their own row, so they survive FR-024 and are removed
only by FR-025. That is why they matter: they are the five tools a naive
"capitalise the name and rename it" implementation would wrongly rename
(`bash_reset` -> `BashReset`, `apply_patch` -> `ApplyPatch`, ...), and the
tie-break exists to stop exactly that.

## 7. Exclusions - FR-024 clause failures, with the failing clause

### 7.1 Clause A - the `Anthropic Tools` row is not `Exact` (20 pairs)

These are ragent rows whose cell names a real Anthropic tool, but that tool's
own row is not rated `Exact`. No amount of exactness on the ragent side can
rescue them.

| Anthropic target | Its mapping | ragent rows naming it |
|---|---|---|
| `Agent` | `Close` | `new_agent`, `wait_agents` |
| `EndConversation` | `Close` | `agent_complete` |
| `LSP` | `Close` | the ten `codeindex_*` tools |
| `ListAgents` | `Close` | `list_agents` |
| `Monitor` | `Partial` | `bg` |
| `Read` | `Close` | `read`, `pdf_read` |
| `SendMessage` | `Close` | `team_message`, `team_read_messages` |
| `TaskStop` | `Close` | `cancel_agent` |

That is 8 distinct targets over 20 distinct ragent tools and 20 pairs (one pair
each, because every excluded ragent row happens to name exactly one non-`Exact`
Anthropic tool). Two of those tools - `wait_agents` and `team_read_messages` -
also carry a prose entry, so they are counted under clause C in the per-row view
of section 3.1; that table is the same fact seen from the rows and totals 18
there. Both are correct, and section 3.1 explains the difference.

`new_agent` -> `Agent` and `read` -> `Read` are the two exclusions the
specification calls out by name: both ragent rows are themselves `Exact`, so
only clause A refuses them. `read` keeps its name, and therefore "read" stays
both the tool and the verb after this change - a fact every non-test literal
must be read against (PLAN risk note, section "Risk Notes").

### 7.2 Clause B - the `ragent Tools` row is not `Exact` (18 tools, 40 pairs)

These name a genuinely `Exact` Anthropic target, but their own row is `Close`,
so FR-024 refuses the pair.

| Anthropic target | ragent rows claiming it |
|---|---|
| `CronCreate`, `CronDelete`, `CronList` | `cron_add`, `cron_disable`, `cron_enable`, `cron_list`, `cron_remove` |
| `Bash` | `copy_file`, `file_info`, `get_env`, `list`, `make_directory`, `move_file`, `rm` |
| `Glob` | `copy_file`, `file_info`, `get_env`, `list`, `make_directory`, `move_file`, `rm` |
| `Edit` | `append_to_file`, `diff_files` |
| `Write` | `append_to_file` |
| `TaskCreate`, `TaskUpdate` | `team_task_claim`, `team_task_complete`, `team_task_create`, `team_task_list` |

The five `cron_*` tools are the exclusion the specification calls out: all five
carry `Mapping = Close` on their own row even though `CronCreate`, `CronDelete`
and `CronList` are `Exact` on the Anthropic sheet, so each of the five fails
clause B three times - hence 15 of the 40 pairs here. The arithmetic reconciles:
`cron_*` 15, `Bash` 7, `Glob` 7, `TaskCreate` 4, `TaskUpdate` 4, `Edit` 2,
`Write` 1.

### 7.3 Clause C - the entry names no Anthropic tool (121 pairs)

`Anthropic equivalent` is prose, not an enumeration of the Anthropic sheet.
Of the 215 `(row, token)` pairs, 121 name something that is not an
`Anthropic Tools` row, so clause C - "that ragent row's cell names `after` as
one of its `/`-separated entries" - has no `after` to name. The pair is
discarded before any mapping is looked up. The 18 distinct forms, with the
number of pairs each accounts for:

| Entry | Pairs | What it actually names |
|---|---|---|
| `MCP` | 30 | a transport, not a tool |
| `Bash (glab)` | 19 | the shell running `glab` |
| `Bash (git)` | 18 | the shell running `git` |
| `none` | 16 | no equivalent |
| `Agent teams` | 12 | a feature, not a tool |
| `Bash (gh)` | 11 | the shell running `gh` |
| `CLAUDE.md` | 3 | a file loaded into context |
| `Edit (repeated)` | 2 | a repeated-edit usage note |
| `Bash (bc` | 1 | half of the `Bash (bc/python)` cell on `calculator` |
| `python)` | 1 | the other half of the same cell |
| `Bash (curl)` | 1 | the shell running `curl` |
| `Agent (name)` | 1 | `Agent` with a named-teammate note |
| `inbox` | 1 | a data structure |
| `SendMessage (fan-out)` | 1 | a broadcast note |
| `SendUserFile (partial)` | 1 | a partial-match note |
| `TaskOutput (removed)` | 1 | a target the vendor has since removed |
| `WebFetch (repeated)` | 1 | a repeated-fetch usage note |
| `thinking` | 1 | a model capability |

Two properties of this table matter for review:

- **A prefix match would change nothing.** Re-reading the cells leniently -
  taking the leading identifier of each entry so that `Bash (git)` reads as
  `Bash` and `Edit (repeated)` reads as `Edit` - yields the same 15 targets, the
  same 13 renames and the same 2 refusals. The derivation is not sensitive to
  how the free-text annotations are trimmed. The strict reading is recorded
  here because it is the only one FR-024's "names `after` as one of its
  `/`-separated entries" wording supports.
- **The `calculator` cell is a single phrase split by the `/` separators**:
  `Bash (bc/python)` reads as the two entries `Bash (bc` and `python)`, and
  neither is a tool. `calculator` is therefore excluded by clause C, not by a
  malformed-workbook accident that a future reader should "repair" into a
  `Bash` claim. The repair would not even help: `calculator`'s own `Mapping` is
  `Partial`, so clause B refuses the pair independently (section 7.2 does not
  list `calculator` because its cell never reaches clause B - it fails clause C
  first).

### 7.4 `Exact` targets with no `Exact` ragent claimant (4 targets)

The remaining `Anthropic Tools` rows rated `Exact` that produced no rename:

| Anthropic target | Status | Why no pair |
|---|---|---|
| `CronCreate` | `Exact` | its only ragent claimants are the five `cron_*` rows, all `Close` (clause B) |
| `CronDelete` | `Exact` | same |
| `CronList` | `Exact` | same |
| `TodoWrite` | `Exact` | no `ragent Tools` row names it anywhere in the workbook (clause C) |

`WebFetch` and `WebSearch` are `Exact` too and also produce no rename, but for a
different reason - they *do* have `Exact` ragent claimants (two each) and are
refused by FR-025, so they are recorded in section 5, not here.

`TodoWrite` is the exclusion the specification calls out by name. There is
nothing to rename, so nothing is renamed. For the three `cron_*` targets the
refusal is not that a rename is impossible at the spelling level - `cron_list`
would capitalise to `CronList` - but that their ragent rows are screened `Close`,
so FR-024 never puts them forward. That is the clause B list in section 7.2,
seen from the target's side instead of the tool's.

Arithmetic check: 19 `Exact` Anthropic rows = 13 accepted (section 4) + 2 refused
(section 5) + these 4.

### 7.5 Rows excluded outright

Breaking the 148 ragent rows by what their `Anthropic equivalent` cell contains:

| Cell content | Rows |
|---|---|
| every entry is an `Anthropic Tools` name | 58 |
| a mix of Anthropic names and prose | 2 (`team_read_messages` = `SendMessage / inbox`, `wait_agents` = `TaskOutput (removed) / Agent`) |
| no Anthropic name at all | 88 |
| **Total** | **148** |

The 88 rows with no Anthropic name contribute nothing but clause-C failures.
Add the 2 mixed rows, whose prose entries also fail clause C, and the 90 distinct
tools in section 3's clause-C column reconcile: 88 + 2 = 90.

## 8. How to reproduce this derivation

`crates/ragent-tools-core/src/xlsx.rs` is a writer only; adding a reader purely
to re-derive the table in a test would breach NFR-002 and FR-023 explicitly
forbids the shipped test from opening the workbook. Reproduction is therefore a
manual step (TC-019), run outside the repository's build:

```python
import openpyxl
wb = openpyxl.load_workbook('tools.xlsx', data_only=True)
anth, rag = wb['Anthropic Tools'], wb['ragent Tools']
anth_map = {r[0]: (r[4] or '').strip() for r in anth.iter_rows(min_row=2, values_only=True) if r[0]}
rows = [{'tool': r[0], 'eq': str(r[4] or ''), 'm': (r[5] or '').strip()}
        for r in rag.iter_rows(min_row=2, values_only=True) if r[0]]

cands = {}
for r in rows:
    if r['m'] != 'Exact':
        continue
    for p in {p.strip() for p in r['eq'].split('/') if p.strip()}:
        if anth_map.get(p) == 'Exact':
            cands.setdefault(p, set()).add(r['tool'])

cap = lambda n: ''.join(s[:1].upper() + s[1:] for s in n.split('_'))
accepted, refused = [], []
for target, claimants in cands.items():
    claimants = sorted(claimants)
    if len(claimants) == 1:
        accepted.append((claimants[0], target))
    else:
        hit = [c for c in claimants if cap(c) == target]
        if len(hit) == 1:
            accepted.append((hit[0], target))
        else:
            refused.append((target, claimants))

pairs = [(b, t) for t, bs in cands.items() for b in bs]
print(len(pairs), len({b for b, _ in pairs}), len(cands))     # 34 22 15
print(sorted(accepted, key=lambda p: p[1]), len(accepted))    # 13 pairs
print(sorted(refused), len(refused))                          # 2 targets
```

Expected output: 13 accepted pairs matching section 4 exactly, and the two
refusals `WebFetch` and `WebSearch` matching section 5. `openpyxl` is a
developer-machine aid for this manual check only; it is not added to the
workspace (`NFR-002`). Before comparing pairs, confirm the workbook fingerprint
in section 1: a different `tools.xlsx` may legitimately derive a different set.

Two numbers to read carefully when this script and TC-019 disagree:

- **The target count and the pair count are different numbers.** The step-3
  filter (an `Exact` ragent row naming an `Exact` target) yields **34 pairs** over
  **22 ragent tools** and **15 distinct targets**. TC-019's expected-results
  bullet used to read "Step 3 yields 15 pairs", which conflated the two; this
  task corrected it to record 34 pairs and 15 targets. Either number reduces to
  the same 13 renames and 2 refusals.
- **`calculator` never appears in the pair accounting at all** - its
  `Anthropic equivalent` cell is `Bash (bc/python)`, which splits into two
  non-tool entries, so it fails clause C before clause B is reached (section 7.3).
  A re-derivation that reports it under clause B is reading the cell leniently.

The script above is the authoritative form of the rule: trim each entry, compare
exactly, then apply the tie-break only to collisions.

### 8.1 Provenance - two independent readers, one answer

This derivation was produced twice, by two readers that share no code, and the
two agreed on every number in this document:

1. a hand-written reader over the workbook's OOXML parts - `xl/workbook.xml` for
   the sheet list, `xl/_rels/workbook.xml.rels` for the sheet paths, and
   `xl/sharedStrings.xml` for the cell text - using only the Python standard
   library (`zipfile`, `xml.etree.ElementTree`); and
2. `openpyxl` 3.1.5, the reader the section 8 script uses.

Both reported 46 Anthropic rows with 19 `Exact`, 148 ragent rows with 24
`Exact` / 55 `Close` / 2 `Partial` / 67 `Ragent-only`, 215 `(row, token)` pairs,
the same 121 / 20 / 40 / 34 split, the same 13 pairs and the same 2 refusals.
The shared-string reader has to be careful about one thing the library hides:
`xl/worksheets/sheetN.xml` references strings by index into
`sharedStrings.xml`, and an index read from the wrong sheet file silently
returns plausible-looking tool names from a different sheet. The sheet order in
`workbook.xml` is Summary, Capability Map, OpenAI Tools, Anthropic Tools,
ragent Tools, Coverage, Sources - so `Anthropic Tools` is `sheet4.xml` and
`ragent Tools` is `sheet5.xml` under this workbook, and the reader resolves them
through the relationship ids rather than by position.

**Re-verified when the record was closed.** Both readers were run again against
the workbook in the tree, and every figure in this document that they can
reproduce was checked mechanically rather than re-read from the prose: the two
sheet shapes in section 1, the 53 padded rows and the 37/94 token counts in
section 2.1, the 215 / 121 / 20 / 40 / 34 pair accounting and its 90 / 20 / 18 /
22 tool columns in section 3, the 90 / 18 / 18 row view in section 3.1, the 15
candidates, 13 pairs and 2 refusals in sections 4 and 5, the 9 / 5 / 4 split in
section 6, every clause breakdown in section 7 (8 targets over 20 pairs at
clause A, 40 pairs over 18 tools at clause B, 18 entry forms over 121 pairs at
clause C, 4 `Exact` targets with no pair, 58 / 2 / 88 rows), and the
9-pair/2-refusal result of dropping the `trim()` in section 2.1. The Appendix A
lists were compared element for element against a fresh derivation and match.
Three numbers in an earlier draft did not survive that check and were corrected
in place; section 9 and section 10 each say which and why.

## 9. Invariants the 13 pairs satisfy, for T-030's test

Hand-checked here; enforced by the test T-030 ships, which does not read the
workbook.

- every `before` is a name the default registry registers - verified three ways:
  each appears as a tool `name()` literal in the crate sources; each appears as a
  row in the workbook's `ragent Tools` sheet; and the registry's own count
  matches that sheet - `ragent run --dry-run "<any prompt>"` reports
  `total: 148 visible tools` on the pre-rename tree, exactly the 148 rows
  `ragent Tools` lists. The per-family counts agree too: `task` 4 (the four
  `task_*`), `skill` 1 (`skill_manage`), `plan` 2 (`plan_enter`, `plan_exit`),
  `ask_user` 1 (`ask_user`), `bash` 3 (one of them `bash`), and
  `glob`/`grep`/`edit`/`write` inside `file` (27 tools);
- every `after` is ASCII-identifier shaped (`^[A-Za-z][A-Za-z0-9_]*$`) - all 13
  qualify;
- every `after` is a name the registry does **not** register, so the rename
  cannot collide with an existing tool. Verified by extracting every name a tool
  class can report - the `fn name(&self) -> &'static str { "..." }` literal form
  and the `name: "..."` field form used by the team tools - from every non-test
  source file, which yields **246** candidate names (165 `fn name()` literals
  plus 107 `name:` field literals, de-duplicated). The figure first written here
  was 212; no combination of the two extraction patterns reproduces it on this
  tree, so it is corrected. Intersecting the 246 with the 13 `after` values
  gives the empty set, and all 13 `before` values are present. The scan is
  deliberately over-inclusive (it also catches provider display names and
  non-registry string fields), which is safe for a collision test;
- a raw text search for the adopted spelling `"Bash"` finds **two** literals in
  non-test source and neither registers a tool: the FR-023 table row itself
  (`crates/ragent-agent/src/tool/rename.rs:75` where T-030 committed it; the
  table now lives at `crates/ragent-types/src/tool_rename.rs:75`) and a language
  name in `crates/ragent-agent/src/memory/knowledge_graph.rs:263`. The other
  **13** occurrences are all in the test tree - hook matchers, display
  assertions and the table's own invariant test (section 10) - so 2 + 13 = 15
  is the whole of the literal across the workspace. This bullet first read "The
  15 test-file occurrences" and anchored the table row at `:66`; both are
  corrected, and section 10 explains where the 13 come from;
- the `after` column is unique (13 distinct values);
- the `before` column is unique (13 distinct values);
- the table is ascending by `after`.

## 10. Consequences downstream

- **T-030** commits exactly these 13 pairs, in this order.
  `crates/ragent-agent/src/tool/rename.rs` holds them as `TOOL_RENAMES`, and
  `crates/ragent-agent/tests/test_tool_rename_table.rs` pins the `after` sequence
  to this document's order - so a derivation that changes the set fails a shipped
  test rather than passing silently. At the time of writing the table module and
  its test exist in the working tree (`PLAN.md` still lists T-030 as pending) and
  the rename itself, T-031, has not run, so the test's two registry-facing cases
  assert the pre-rename registry and flip with that change.
- **T-031** renames at source. Nine of the thirteen take the capitalisation-only
  form (`Bash`, `Edit`, `Glob`, `Grep`, `Write`, plus the four `Task*`), so a
  mechanical `name()` edit is not enough: `ask_user`, `plan_enter`, `plan_exit`
  and `skill_manage` change word, not just case.
- **T-038** carries the test churn. Note the scale, with the scan pinned so the
  number is reproducible rather than recollected. Count every occurrence of one
  of the 13 retired spellings as a double-quoted or backticked literal
  (`"(name)"` or `` `(name)` ``) in a `.rs` file. Measured on the **pre-rename
  tree** (`1.0.149`) with the FR-023 table and its invariant test added - i.e.
  the tree this derivation was written against, before T-031 moved a single
  spelling:

  | Bucket | Occurrences | Files |
  |---|---|---|
  | non-test source (`crates/*/src` + `src/`), FR-023 table **excluded** | **377** | **76** |
  | non-test source including the FR-023 table itself | 391 | 77 |
  | non-test source plus `crates/*/benches` (table excluded) | 383 | 77 |
  | test files under `crates/*/tests`, FR-023 invariant test **excluded** | **775** | **160** |
  | the same, invariant test included | 778 | 161 |
  | the same plus the root `tests/` directory (10 files, 4 occurrences) | 779 | 162 |
  | the same plus the root `tests/` directory and the invariant test | 782 | 163 |

  The two figures T-038 should work from are the bold ones: **377 occurrences
  in 76 non-test source files** (the T-033 job) and **775 in 160 test files**
  (the T-038 job).

  **Every figure this section first carried is wrong, and it is worth saying
  exactly how**, because a reader who quoted them (as `PLAN.md` and T-038's row
  did) needs to know which mistake they inherited:

  - **The source column was measured with the single-quote-inclusive rule.**
    Accepting `'name'` as well as `"name"` and `` `name` `` yields 381
    occurrences with the table excluded, 395 with it included and 387 with
    `crates/*/benches` added - exactly the three occurrence figures the table
    used to show. The rule this document states is quoted-or-backticked, so the
    correct figures are 4 lower in each case: 377, 391 and 383. The four
    literals that make the difference are all prose about the retired names and
    are listed under "widening experiments" below.
  - **The source column's file counts were each one high.** The three source
    rows read 77, 78 and 78; the any-quote rule yields 76, 77 and 77 files, and
    the stated rule yields the same 76, 77 and 77 - the four single-quoted
    literals sit in files that already contain a quoted or backticked one, so
    the file count does not move between the two rules. The file column is
    therefore one high on every source row, consistently, while the *test*
    column's file counts are all correct. Both columns of that table should be
    re-measured, not trusted; the commands below do it.
  - **The test column's occurrence counts were two low.** The file counts (160,
    161, 162, 163) reproduce exactly; the occurrences do not. The stated rule
    yields 775 / 778 / 779 / 782 against the recorded 773 / 776 / 777 / 780, so
    every row in that column is short by two. Because the file column was right,
    the error was invisible: only the occurrence column shifted.
  - **`PLAN.md`'s older 388/79** matches nothing on any tree under any rule.

  Re-run the commands below rather than trusting the table; the counts move as
  prose is written. Source comments count; `docs/`, `CHANGELOG.md` and this
  document do not. The `grep -v` covers both possible homes for the table (it
  was `crates/ragent-agent/src/tool/rename.rs` when this derivation was written
  and is `crates/ragent-types/src/tool_rename.rs` now), so the same pair of
  commands is correct on either tree.

  ```bash
  PAT='["`](ask_user|bash|edit|plan_enter|plan_exit|glob|grep|skill_manage|task_create|task_get|task_list|task_update|write)["`]'

  # source bucket, FR-023 table excluded: occurrences, then files
  find crates/*/src src -name '*.rs' -type f \
    | grep -vE 'tool_rename\.rs$|tool/rename\.rs$' \
    | xargs grep -ohE "$PAT" | wc -l
  find crates/*/src src -name '*.rs' -type f \
    | grep -vE 'tool_rename\.rs$|tool/rename\.rs$' \
    | xargs grep -lE "$PAT" | wc -l

  # test bucket, invariant test excluded: occurrences, then files
  find crates/*/tests -name '*.rs' -type f | grep -v 'test_tool_rename_table.rs' \
    | xargs grep -ohE "$PAT" | wc -l
  find crates/*/tests -name '*.rs' -type f | grep -v 'test_tool_rename_table.rs' \
    | xargs grep -lE "$PAT" | wc -l
  ```

  On the derivation-time tree those print `377`, `76`, `775`, `160`.

  Per-spelling split of the 377, so T-033 and T-038 know which name carries
  the weight (`bash` alone is a fifth of the source bucket, which is why it is
  also the name with the most non-tool meanings):

  | Spelling | Source occurrences (table excluded) |
  |---|---|
  | `bash` | 81 |
  | `edit` | 65 |
  | `grep` | 60 |
  | `write` | 55 |
  | `glob` | 39 |
  | `ask_user` | 19 |
  | `task_get` | 11 |
  | `task_update` | 10 |
  | `task_create` | 9 |
  | `task_list` | 9 |
  | `plan_enter` | 7 |
  | `plan_exit` | 7 |
  | `skill_manage` | 5 |
  | **Total** | **377** |

  **For orientation after the landing.** The table above is the size of the job
  as this derivation saw it. On the tree as committed (`1.0.149` + the rename,
  `d6abcd4a`) the same rule gives a different shape, because that commit
  migrated most of the source and none of the tests: **370** occurrences in
  **78** non-test source files, and **836** in **167** test files (839/168
  with the invariant test, 840/169 and 843/170 with the root `tests/`
  directory). T-033 and T-038 must measure the tree in front of them; neither
  column transfers.

  Two widening experiments, recorded so a reader does not have to redo them:
  adding the single-quoted form adds exactly the four literals in **two**
  files - `crates/ragent-agent/src/agent/mod.rs:658` names `'grep'` and
  `'glob'`, `crates/ragent-agent/src/tool/aliases.rs:94` names `'write'` and
  `:140` names `'bash'` - and all four are prose about the retired names, so
  they stay outside the rule. Including `crates/*/benches` adds exactly the
  six literals in `crates/ragent-bench/benches/agent_loop.rs` (five `bash`,
  one `write`), which build `MessagePart::ToolCall` records and a
  `MockLlmScript` with the retired spelling.
- **T-033/T-034** migration must not touch the five tie-break losers
  (`apply_patch`, `bash_reset`, `create`, `patch`, `update_file`), the two
  refusal claimants (`mf_fetch`, `webfetch`, `mf_search`, `websearch`) or
  `read`, none of which is renamed.
- **T-019/T-033** the D7 alias table keeps `WebFetch` and `WebSearch` (refused
  targets) and the OpenAI spellings, and drops every entry whose target became a
  ragent name.
- **T-031/T-038, two pre-existing spellings the derivation found.** Both matter
  because the rename makes them *correct* where today they are speculative:
  - `crates/ragent-tui/src/widgets/message_widget.rs:159` already has
    `capitalize_tool_name`, which uppercases only the first character
    (`bash` -> `Bash`, but `task_create` -> `Task_create`). That is the wrong
    rule for FR-025's per-segment capitalisation and it must not be reused as
    the rename's implementation - the 13 pairs are a fixed table, not a
    transformation. Its one existing assertion
    (`crates/ragent-tui/tests/test_tool_display.rs:459`) keeps passing unchanged
    because `bash` really does become `Bash`. (Both line numbers are the
    pre-rename ones; on the committed tree they are `:168` and `:474`, and the
    function's rule is unchanged.)
  - Eight test call sites already pass `"Bash"` as a hook matcher or as the tool
    name a hook guard is asked about: `crates/ragent-agent/tests/test_plugin_hooks.rs:134`,
    `:143`, `:144`, `crates/ragent-agent/tests/test_agent_skill_hooks_extra.rs:778`,
    `:793`, `:794`, `:817`, and `crates/ragent-plugins/tests/test_manifest.rs:834`.
    Those matchers start matching a real tool once the rename lands, which is the
    outcome FR-028 wants; they need no migration, so T-038 should leave them
    untouched rather than "update" a matcher that is already correct. This
    bullet originally read "ten call sites" and listed two further line numbers,
    `test_plugin_hooks.rs:265` and `:269`; that file has 247 lines, so neither
    line exists and both are removed. Eight is the count that resolves.
  - Five more test literals across three files spell `Bash` and are also already
    correct: `crates/ragent-tui/tests/test_tool_display.rs:459`
    (`capitalize_tool_name("bash") == "Bash"`),
    `crates/ragent-tui/tests/test_text_selection.rs:503` (an either-spelling
    contains check; this bullet first said `:497`, which is the middle of the
    buffer-dump loop above the assertion), and three in
    `crates/ragent-agent/tests/test_tool_rename_table.rs`
    (the FR-023 table's own pinned list and two lookup assertions). With the
    eight above that accounts for all **13** `"Bash"` literals in the test tree.
    This bullet and the one above originally totalled 15; the corrected pairs
    are 8 matcher sites + 5 display/table literals = 13, which is what the
    invariant bullet in section 9 now records alongside the 2 in non-test
    source (the FR-023 table row and the `knowledge_graph.rs` language name).
- **T-029 touches no Rust file.** The task is a derivation and a record, not an
  implementation: no source edit, no test edit, no new dependency, no tool
  renamed. The files it writes are this document, one `CHANGELOG.md` entry, one
  corrected expected value in `TESTPLAN.md` (TC-019 step 3) and one corrected
  count in `PLAN.md`. T-030 is the first task that writes code.
- **One observation for whoever picks up T-030 (recorded, not acted on).** The
  FR-023 table's home moved after this derivation: it was committed at
  `crates/ragent-agent/src/tool/rename.rs` and now sits at
  `crates/ragent-types/src/tool_rename.rs` behind a one-line re-export, so
  `ragent-config` can read it (FR-031). Section 9's table-row anchor is given
  for both locations. This document does not prescribe the location - FR-023
  only requires one module - but a reader comparing paths should know the
  move happened and that no derivation number depends on it.

---

## Appendix A - every one of the 148 ragent rows and its outcome

Section 3.1 gives the counts; this table names the rows, so a reviewer can
confirm that nothing fell between the buckets. Buckets are disjoint and decided
in the order C, A, B, then the FR-025 outcome, matching section 3.

### A.1 Clause C - the cell names no `Anthropic Tools` row (90 rows)

`calculator`, `commands_info`, `conversation_search`, `git_add`, `git_branch`,
`git_checkout`, `git_cherry_pick`, `git_clone`, `git_commit`, `git_diff`,
`git_fetch`, `git_log`, `git_merge`, `git_pull`, `git_push`, `git_remote`,
`git_reset`, `git_show`, `git_stash`, `git_status`, `git_tag`,
`github_close_issue`, `github_comment_issue`, `github_create_issue`,
`github_create_pr`, `github_get_actions`, `github_get_issue`, `github_get_pr`,
`github_list_issues`, `github_list_prs`, `github_merge_pr`, `github_review_pr`,
`gitlab_approve_mr`, `gitlab_cancel_job`, `gitlab_cancel_pipeline`,
`gitlab_close_issue`, `gitlab_comment_issue`, `gitlab_create_issue`,
`gitlab_create_mr`, `gitlab_get_issue`, `gitlab_get_job`, `gitlab_get_job_log`,
`gitlab_get_mr`, `gitlab_get_pipeline`, `gitlab_list_issues`, `gitlab_list_jobs`,
`gitlab_list_mrs`, `gitlab_list_pipelines`, `gitlab_merge_mr`, `gitlab_retry_job`,
`gitlab_retry_pipeline`, `http_request`, `initiative`, `memory_forget`,
`memory_recall`, `memory_store`, `mf_cache_clear`, `mf_crawl`, `mf_version`,
`model_info`, `multi_edit`, `multiedit`, `open`, `os_info`, `pdf_write`,
`ragent_info`, `session_search`, `spec_coverage`, `spec_list`, `spec_read`,
`spec_search`, `spec_task_update`, `team_approve_plan`, `team_assign_task`,
`team_broadcast`, `team_cleanup`, `team_create`, `team_idle`, `team_memory_read`,
`team_memory_write`, `team_read_messages`, `team_shutdown_ack`,
`team_shutdown_teammate`, `team_spawn`, `team_status`, `team_submit_plan`,
`team_wait`, `think`, `tool_info`, `wait_agents`.

`team_read_messages` and `wait_agents` are here because each has one prose entry
that names no tool; both also have a tool entry, which is why they appear in
section 7.1's 20 pairs too (section 3.1).

### A.2 Clause A - the Anthropic target is not `Exact` (18 rows)

`agent_complete`, `bg`, `cancel_agent`, `codeindex_communities`,
`codeindex_dependencies`, `codeindex_explain`, `codeindex_godnodes`,
`codeindex_path`, `codeindex_references`, `codeindex_reindex`, `codeindex_search`,
`codeindex_status`, `codeindex_symbols`, `list_agents`, `new_agent`, `pdf_read`,
`read`, `team_message`.

Every one names exactly one non-`Exact` Anthropic target (the mapping per target
is in section 3.1's table). Only two of the 18 - `new_agent` and `read` - are
themselves `Exact` on the ragent sheet, so for those two clause A is the *only*
thing refusing the pair; that is exactly why the specification names them. The
other 16 are `Close` on their own row as well, so they would fail clause B if
clause A had passed. They are counted here rather than in section A.3 because
section 3's buckets are decided C then A then B, and their first failure is A.

### A.3 Clause B - the ragent row is not `Exact` (18 rows)

`append_to_file`, `copy_file`, `cron_add`, `cron_disable`, `cron_enable`,
`cron_list`, `cron_remove`, `diff_files`, `file_info`, `get_env`, `list`,
`make_directory`, `move_file`, `rm`, `team_task_claim`, `team_task_complete`,
`team_task_create`, `team_task_list`.

These 18 rows own the 40 clause-B pairs: `cron_*` contribute 3 each (15),
`copy_file`/`file_info`/`get_env`/`list`/`make_directory`/`move_file`/`rm` 2 each
(14), `append_to_file` 2, `diff_files` 1, the four `team_task_*` 2 each (8).
15 + 14 + 2 + 1 + 8 = 40.

### A.4 Renamed (13 rows)

`ask_user`, `bash`, `edit`, `glob`, `grep`, `plan_enter`, `plan_exit`,
`skill_manage`, `task_create`, `task_get`, `task_list`, `task_update`, `write` -
the section 4 table.

### A.5 Claimants of a refused target (4 rows)

`mf_fetch`, `webfetch` (both claim `WebFetch`), `mf_search`, `websearch` (both
claim `WebSearch`).

### A.6 Tie-break losers (5 rows)

`apply_patch`, `bash_reset`, `create`, `patch`, `update_file` - the section 6
table.

90 + 18 + 18 + 13 + 4 + 5 = **148**.
