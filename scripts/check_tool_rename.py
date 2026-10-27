#!/usr/bin/env python3
"""Check every NFR-008 predicate against the checked-in tree (spec `uniformtools`).

One implementation, two entry points:

* ``python3 scripts/check_tool_rename.py`` -- the CI guard. Exits 0 when no
  NFR-008 predicate holds anywhere in the tree, 1 otherwise, naming every
  offending file and line for every predicate in one report.
* ``cargo test -p ragent-agent -E 'test(test_tool_rename_guard::)'`` -- the crate
  test imports the same predicates by invoking this file as a subprocess, so the
  guard cannot drift between the script and the test (NFR-009).

Hermetic and cheap: reads only files inside the repository, makes no network
request, does not open ``tools.xlsx``, and finishes in a second or two.

See ``scripts/check-tool-rename.sh`` for the human-facing CLI and self-test.
The shell script is the entry point ``.github/workflows/ci.yml`` and
``pre-flight.sh`` invoke; it delegates here for the hot path.

## The seven predicates

NFR-008 lists the ways the no-alias rule (FR-026) can erode, and requires that a
guard fail the build when any of them holds. This file implements all seven. Each
hit is labelled with its predicate letter so a reader knows which rule broke and
which edit fixes it.

(a) **A `before` spelling is a key or a value in an alias table.** Scanned by text
    in the table declarations (``<X>Alias { spelling: "...", target: "..." }``) and
    in the pair tables (``("spelling", "target"),`` rows): every ``Alias { .. }``
    block in the tree, the files in ``RESOLVER_FILES``, and any line carrying an
    alias field. The *value* half matters as much as the key half: an alias keyed
    on something else but pointing at a retired name resolves nothing once the
    rename lands.
(a2) **An alias entry maps a spelling onto the identical string.** Not one of the
    seven, but FR-031 refuses it and it is dead weight the resolver should not
    carry, so the same scan reports it. ``SELF_MAPPING_ALLOW`` holds the one
    reviewed table that legitimately shows the shape - the FR-031 refusal's own
    doctest, which has to write the entry it refuses.
(b) **A `before` spelling appears in a resolver match arm.** Scanned as a
    ``"name" => `` arm in the resolver files (``RESOLVER_FILES``, each with its
    written reason), plus any ``Rename::after_of`` / ``Rename::before_of`` /
    ``Rename::is_retired`` / ``Rename::is_rename_spelling`` call carrying a
    literal - that call *is* a hand-written arm, whatever file it is in. An arm
    whose spelling is a retired one is reported whether its body returns a tool
    name, a permission family or a display label: all three are the "second
    spelling" FR-026 forbids.
(c) **A `before` spelling is a default configuration value.** The defaults are
    constructed in Rust, so the authoritative half is the real ``Config::default()``
    observed by the crate test (``test_fr026_no_alias_survives``). This guard adds
    the shape half inside ``ragent-config``: a retired spelling in a
    ``default = "..."`` serde attribute, or any literal inside a ``fn default_*()``
    body (which is where a defaulted value is written).
(d) **A `before` spelling is a registry key.** A registry key is a tool's
    ``name()``, not a literal at the ``register`` call, so the live half is
    evaluated against the registry by the crate test. Reported here is the shape
    half: a retired spelling returned from a ``fn name(`` body, which is the value
    ``register`` would key on.
(e) **A `before` spelling is a quoted or backticked literal in non-test source.**
    ``crates/*/src/**/*.rs`` and ``src/*.rs``, skipping ``tests/`` (a test may
    name a retired spelling to assert it is retired), outside the two structural
    exemptions and the per-line ``EXEMPTIONS`` table.
(f) **One tool name is registered more than once.** A registry pass by the crate
    test (``test_fr026_no_alias_survives``) and a text pass here that counts
    ``fn name(`` bodies across the tree, so two tool types claiming one name are
    caught without running anything. A name declared in two different files is the
    hit: one file may legitimately spell a name twice (an alias and its canonical
    tool share the shape, not the string).
(g) **A registered tool's `execute` delegates wholly to a tool that was already
    registered.** A registry pass by the crate test (it can see the delegation
    target and the registration order together) and a text pass here that reads an
    ``execute`` body which forwards and decides nothing, to a tool type declared in
    a crate the forwarder's own crate depends on - see ``_delegate_violations``.

Predicate (e)'s "outside the FR-023 table and the CHANGELOG.md entry" is the two
structural exemptions below; every other exemption is a per-line ``EXEMPTIONS``
entry carrying the written reason it is not a tool name.

## What a "literal" is

A hit is a double-quoted or backticked literal on a **code** line whose decoded
value is *exactly* one of the thirteen retired spellings. Exact equality matters:
``"write "`` (the history heuristic's English verb probe) and ``"write_file"`` (a
prefix probe) are not the retired name and are not hits. A trailing ``//`` comment
is stripped before the literals are read, so a code line that mentions the old
spelling in a note is judged on its code alone.

A **comment line** - one whose first non-space characters are ``//`` - is prose
and yields no code part at all, so it is outside every predicate. That is a scope
decision rather than an exemption: a comment cannot key on a name, and the rename's
own documentation and a doc-comment example must be free to state the retired
spelling (a doctest that feeds ``"bash": "Bash"`` in to assert the FR-031 refusal
would be impossible otherwise). A ``//`` that appears *after* code is the trailing
note and is stripped, leaving the code judged on its own.

## The two structural exemptions

1. ``crates/ragent-types/src/tool_rename.rs`` -- the FR-023 table itself. Its
   ``before:`` fields necessarily carry every retired spelling, and it is the
   single source of truth this guard *reads* (see below), so scanning it would be
   circular. The exemption covers the literal scan only: the table is read, not
   skipped, and a parse failure is a guard failure.
2. ``CHANGELOG.md`` -- outside the scanned tree; the ``Renamed`` table and its
   narrative must name all thirteen spellings to document the migration. It is
   also the only prose file whose job is to name the retired spellings.

## The per-line exemption table

Everything else is an entry in ``EXEMPTIONS``. Each entry carries the reason it is
not a tool name, and is keyed on the file, the literals it carries and the exact
(stripped) source text rather than on a line number, so a line moving is not a
false alarm and a line *changing* is. Each entry also declares how many lines it
must match and which predicates it silences (predicate (e) unless it says
otherwise; a resolver arm also needs the file in ``RESOLVER_FILES``).

The list is checked in both directions, which is what makes it an audit rather
than a filter:

* a retired literal on a line matching no entry is a violation (the site the task
  exists to catch);
* an entry that matches no line, or fewer lines than it declares, is also a
  failure -- a dead exemption is an unreviewable one, and it tells the next reader
  that a file is safe for a reason that no longer applies;
* an entry whose declared literals differ from the ones actually found, or which
  names a spelling that is not retired, is a failure.

Each entry also names the predicates it silences. Predicate (e) is the default; an
entry whose line is also a resolver arm (``Permission::from``'s arms, a language-id
match, a stage-id match) names ``"b"`` as well, and an entry that silences nothing
(the guard's own self-test fixture, which exists to be reported) declares ``()`` -
the empty set - so a line that must still fail is not silenced by having been
written down.

## Hermetic and cheap (NFR-009)

One implementation, three callers, no separate logic to drift:

* ``scripts/check-tool-rename.sh`` - the CI and ``pre-flight.sh`` entry point;
* ``python3 scripts/check_tool_rename.py`` - the same program, invoked directly;
* ``crates/ragent-agent/tests/test_tool_rename_guard.rs`` - the ``cargo test``
  case, which runs ``--json`` and asserts the *same* verdict, the *same* hits and
  the *same* ASCII wording the script prints. The two forms cannot disagree
  because there is only one of them.

``--json`` exists for that third caller: a machine-readable report of the verdict,
every hit (predicate, path, line, literals, the exact edit) and every exemption
problem, so the test can compare the two entry points field by field rather than
by re-parsing prose. ``--root DIR`` exists for the same reason: it lets the test
point the one implementation at a scratch copy of the repository with a fixture
seeded into it, which is how the hermetic and one-run-reports-every-predicate
cases are proven without writing into the checked-in tree.

The program reads only files under the root, makes no network request, never
opens ``tools.xlsx``, and finishes in about two seconds on the full tree.

No exemption is scoped to a whole file. An entry for a tool-name site is a defect:
it would silently permit the next stale literal added to that file.

## One run, every predicate

All predicates run in one pass over one parse of every scanned file, and every hit
is collected before anything is printed. The report is sorted by path then line,
each line carries its predicate letter, the offending text, and the exact edit
that clears it, so a single run fixes the whole set rather than the first failure.
Every message is ASCII and the exit code is non-zero on any hit (NFR-009).
"""

from __future__ import annotations

import ast
import json
import pathlib
import re
import sys
from typing import Callable, NamedTuple

ROOT = pathlib.Path(__file__).resolve().parent.parent

# --- The rename table, read from its one home -------------------------------
#
# The table is NOT duplicated here. Reading it means this guard cannot disagree
# with the code it guards, and it makes the FR-023 twelve-month invariant ("the
# pairs live in exactly one place") true of the guard too. A parse failure is a
# guard failure, not a skip: guessing the spellings would defeat the point.

TABLE_FILE = "crates/ragent-types/src/tool_rename.rs"

# Structural exemption 1 (FR-023 table). It is where BEFORE/AFTER come from.
STRUCTURAL_SKIP = frozenset({TABLE_FILE})

# Structural exemption 2 (the migration note). Outside the scanned tree.
STRUCTURAL_CHANGELOG = "CHANGELOG.md"

SCAN_GLOBS = ("crates/*/src/**/*.rs", "crates/*/src/*.rs", "src/*.rs")

_TABLE_ROW_RE = re.compile(
    r'Rename\s*\{\s*before:\s*"([^"]+)"\s*,\s*after:\s*"([^"]+)"\s*,?\s*\}',
    re.S,
)

EXPECTED_PAIRS = 13


class TableError(Exception):
    """The FR-023 table could not be read. Fails the guard closed."""


def load_pairs(root: pathlib.Path = ROOT) -> list[tuple[str, str]]:
    """Return the FR-023 ``(before, after)`` pairs from the Rust table.

    Args:
        root: Repository root to read ``TABLE_FILE`` beneath.

    Returns:
        The pairs in table order (ascending by ``after``).

    Raises:
        TableError: The file is missing, unreadable, unparsable, or does not hold
            exactly ``EXPECTED_PAIRS`` pairs.
    """
    path = root / TABLE_FILE
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        # An absent table is the dangerous case: the cached pair set below was
        # read from THIS module's checkout, so a run pointed at another root
        # (``--root``) would judge that tree against spellings it never supplied.
        # Fail closed rather than scan with a table the root does not have.
        raise TableError(
            f"cannot read {TABLE_FILE} under {root}: {exc} (the guard reads the "
            "table from the root it scans, so a tree without it cannot be judged)"
        ) from exc
    pairs = _TABLE_ROW_RE.findall(text)
    if len(pairs) != EXPECTED_PAIRS:
        raise TableError(
            f"{TABLE_FILE} yields {len(pairs)} Rename rows, expected "
            f"{EXPECTED_PAIRS}; the guard reads this table rather than "
            "repeating it, so it must stay in the `Rename { before: .., "
            "after: .. }` shape"
        )
    return pairs


_PAIRS = load_pairs()
BEFORE: tuple[str, ...] = tuple(before for before, _ in _PAIRS)
AFTER_OF: dict[str, str] = {before: after for before, after in _PAIRS}
BEFORE_SET = frozenset(BEFORE)
AFTER_SET = frozenset(after for _, after in _PAIRS)


# --- Predicate metadata -----------------------------------------------------


class Predicate(NamedTuple):
    """One NFR-008 predicate, its rule, and the edit that clears a hit.

    Attributes:
        letter: The label a hit carries (`a`-`g`, plus the extra `a2`).
        title: The rule in one line, used when a hit has no retired spelling.
        fix: The exact edit that clears a hit of this predicate.
        nfr008: Whether the letter is one of NFR-008's seven, or an extra rule
            the guard also enforces. The success message names the seven, so a
            clean run lists the requirement's own letters and nothing else.
    """

    letter: str
    title: str
    fix: str
    nfr008: bool = True


PREDICATES: dict[str, Predicate] = {
    p.letter: p
    for p in (
        Predicate(
            "a",
            "alias-table entry names or targets a retired spelling",
            "delete the entry (FR-026: a renamed tool has one spelling, and it "
            "resolves on the registry alone)",
        ),
        Predicate(
            "a2",
            "alias-table entry maps a spelling onto the identical string",
            "delete the no-op entry; the resolver treats it as unresolved and "
            "carries it for nothing (FR-031)",
            nfr008=False,
        ),
        Predicate(
            "b",
            "resolver match arm dispatches on a retired spelling",
            "delete the arm; after the rename the adopted spelling resolves on "
            "the registry, so no arm is needed (FR-026)",
        ),
        Predicate(
            "c",
            "default configuration value holds a retired spelling",
            "change the default to the adopted spelling, or drop it (FR-031)",
        ),
        Predicate(
            "d",
            "a tool reports a retired spelling as its registry key",
            "rename the tool to its adopted spelling (FR-023, FR-026)",
        ),
        Predicate(
            "e",
            "retired tool spelling as a quoted/backticked literal in non-test source",
            "migrate the literal to the adopted spelling, or add an EXEMPTIONS "
            "entry stating why it is not a tool name (FR-027)",
        ),
        Predicate(
            "f",
            "one tool name is registered more than once",
            "give each tool a distinct name; a second registration under one name "
            "is exactly the second spelling FR-026 forbids",
        ),
        Predicate(
            "g",
            "a tool's execute delegates wholly to an earlier-registered tool",
            "make the tool do its own work, or register it under the adopted "
            "spelling; a wholesale forwarder is an alias wearing a tool's shape "
            "(FR-026 clause 2)",
        ),
    )
}


# --- The per-line exemption table -------------------------------------------


class Exemption(NamedTuple):
    """One reviewed, individually justified non-tool-name line.

    Attributes:
        path: Repository-relative path of the source file.
        literals: Every retired spelling the line carries, sorted. Declaring them
            makes a changed literal set a failure rather than a silent widening
            of the exemption.
        text: The exact stripped source text of the line, keyed instead of a line
            number so a shift is not a false alarm and an edit is.
        count: How many lines this entry must match (some expressions repeat).
        reason: Why the line is not a tool-name site. Required; an entry without
            one is rejected before the scan runs.
        predicates: Which predicate letters this line is exempt from. Defaults to
            ``("e",)``; a line that is also a resolver arm names ``"b"`` too.
    """

    path: str
    literals: tuple[str, ...]
    text: str
    count: int
    reason: str
    predicates: tuple[str, ...] = ("e",)


# Every entry below is a reviewed decision from the T-033 migration audit
# (`specs/uniformtools/T-033-MIGRATION-SITES.md`, "Deliberately not migrated").
# The classes are labelled per entry so the list reads as an audit rather than a
# filter. Read the reason before adding one: an entry for a site that *keys* on a
# tool name is the defect this guard exists to catch.
EXEMPTIONS: tuple[Exemption, ...] = (
    # --- Permission vocabulary (FR-028): a permission type, not a tool name ---
    # `Permission::from` accepts these spellings from a config rule; the tool
    # called `Edit`/`Bash`/`EnterPlanMode`/`ExitPlanMode` is governed BY them.
    # The `from` arms are also resolver arms of the permission vocabulary, so
    # they are exempt from predicate (b) as well as predicate (e).
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("edit", "write"),
        '"edit" | "write" => Self::Edit,',
        1,
        "Permission::from arm -- permission type strings, not tool names (FR-028)",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("bash",),
        '"bash" | "execute" => Self::Bash,',
        1,
        "Permission::from arm -- permission type string, not a tool name (FR-028)",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("plan_enter",),
        '"plan_enter" | "plan" => Self::PlanEnter,',
        1,
        "Permission::from arm -- permission type string, not a tool name (FR-028)",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("plan_exit",),
        '"plan_exit" => Self::PlanExit,',
        1,
        "Permission::from arm -- permission type string, not a tool name (FR-028)",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("edit",),
        'Self::Edit => write!(f, "edit"),',
        1,
        "Permission::Display arm -- the round trip of the type string (FR-028)",
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("bash",),
        'Self::Bash => write!(f, "bash"),',
        1,
        "Permission::Display arm -- the round trip of the type string (FR-028)",
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("plan_enter",),
        'Self::PlanEnter => write!(f, "plan_enter"),',
        1,
        "Permission::Display arm -- the round trip of the type string (FR-028)",
    ),
    Exemption(
        "crates/ragent-config/src/permission.rs",
        ("plan_exit",),
        'Self::PlanExit => write!(f, "plan_exit"),',
        1,
        "Permission::Display arm -- the round trip of the type string (FR-028)",
    ),
    Exemption(
        "crates/ragent-agent/src/session/permissions.rs",
        ("edit",),
        '|| permission == "edit"',
        1,
        "permission-type comparison -- file:write family, not a dispatch (FR-028)",
    ),
    Exemption(
        "crates/ragent-agent/src/session/permissions.rs",
        ("write",),
        '|| permission == "write")',
        1,
        "permission-type comparison -- file:write family, not a dispatch (FR-028)",
    ),
    Exemption(
        "crates/ragent-agent/src/tool/aliases.rs",
        ("ask_user",),
        '"ask_user"',
        1,
        "AskUserQuestion::permission_category -- the rule key is permission "
        "vocabulary (FR-028), not a registry lookup",
    ),
    # --- Shell program names: the shell binary, not the Bash tool ---
    Exemption(
        "crates/ragent-agent/src/session/verification.rs",
        ("bash",),
        'let result = Command::new("bash")',
        1,
        "spawns the shell program to run a verification script",
    ),
    Exemption(
        "crates/ragent-tools-core/src/bash.rs",
        ("bash",),
        'if let Ok(path) = which::which("bash") {',
        1,
        "discovers the shell binary on PATH; no tool registry involved",
    ),
    Exemption(
        "crates/ragent-tools-core/src/bash.rs",
        ("bash",),
        'OsStr::new("bash"),',
        2,
        "the shell program for `bash -n` syntax validation (`ShellType::Bash` "
        "and the PowerShell fallback that reaches the same arm)",
    ),
    Exemption(
        "crates/ragent-tools-core/src/bash.rs",
        ("bash",),
        'ShellType::Bash => "bash".as_ref(),',
        1,
        "the shell program for `bash -c`; the arm matches a ShellType variant",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-tools-core/src/bash.rs",
        ("bash",),
        'ShellType::PowerShell(_) => "bash".as_ref(),',
        1,
        "documented fallback to the shell program if the outer match changes",
        ("e", "b"),
    ),
    # --- Shell commands in a validation list: program names, not tools ---
    Exemption(
        "crates/ragent-tools-core/src/bash.rs",
        ("grep",),
        '"cat", "head", "tail", "grep", "egrep", "fgrep", "rg", // ripgrep',
        1,
        "SAFE_COMMANDS entry -- the UNIX program the shell may run",
    ),
    Exemption(
        "crates/ragent-agent/src/skill/context.rs",
        ("grep",),
        '"grep", "rg", "cut", "sort", "uniq", "tr", "diff", "comm", "paste", '
        '"column", "fmt", "fold",',
        1,
        "ALLOWED_EXECUTABLES entry -- the read-only executable a skill body may "
        "run, checked against the base name of a spawned process",
    ),
    Exemption(
        "crates/ragent-connectors/src/descriptor.rs",
        ("bash",),
        '"bash",',
        1,
        "DENIED_CONNECTOR_COMMANDS entry (SEC-009) -- a shell program refused as "
        "a connector stdio command",
    ),
    # --- Config section names, not tool keys ---
    Exemption(
        "crates/ragent-config/src/bash_lists.rs",
        ("bash",),
        'crate::list_config::patch_config(scope, "bash", &["allowlist", '
        '"denylist"], |bash| {',
        4,
        "the `bash` configuration SECTION name the allowlist/denylist live under; "
        "`Config::bash` is unchanged by the tool rename",
    ),
    # --- Language ids: the shell dialect a file or project is written in ---
    Exemption(
        "crates/ragent-codeindex/src/scanner.rs",
        ("bash",),
        '"sh" | "bash" => "shell",',
        1,
        "code-index language alias -- a file extension, not a tool name",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-tools-extended/src/project_scaffold/flags.rs",
        ("bash",),
        '"shell" | "sh" | "bash" => Ok(Language::Shell),',
        1,
        "scaffold language id -- an application language a project is created "
        "in, not a tool",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-tools-extended/src/project_scaffold/help.rs",
        ("bash",),
        "\\x20     `js`/`jsx`, `c++`, `c_header`/`cpp_header`, `sh`/`bash`,\\n\\",
        1,
        "scaffold help text listing the language ids `--language` accepts",
    ),
    Exemption(
        "crates/ragent-tui/src/app/toolchain.rs",
        ("bash",),
        'Some(LanguageRuntime::many(&["bash"])),',
        1,
        "toolchain runtime name -- a language runtime probed on the host, not a "
        "tool registration",
    ),
    # --- Pipeline stage ids (GovCreate): a stage label, not the Write tool ---
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        '"write" => "writing spec files",',
        1,
        "GovCreate stage id -> human label; the stage writes spec files, it is "
        "not a tool call",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        'return cancelled(completed, "write", progress);',
        1,
        "GovCreate stage id passed to the cancellation reporter",
    ),
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        'progress.emit(GovCreateProgress::Started { id: "write" });',
        1,
        "GovCreate stage id for the streaming display",
    ),
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        'return stage_failed(completed, "write", cause, progress);',
        1,
        "GovCreate stage id passed to the failure reporter",
    ),
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        'return stage_failed(completed, "write", other.to_string(), progress);',
        1,
        "GovCreate stage id on the catch-all error path",
    ),
    Exemption(
        "crates/ragent-tools-extended/src/archdoc/govcreate_run.rs",
        ("write",),
        'id: "write",',
        1,
        "GovCreate completed-stage record",
    ),
    # --- English nouns and verbs in heuristic tables (not tool names) ---
    Exemption(
        "crates/ragent-agent/src/team/classify.rs",
        ("write",),
        '"write",',
        1,
        "KEYWORD_MAP stem -- the English verb in a swarm topic classifier",
    ),
    Exemption(
        "crates/ragent-specs/src/validate.rs",
        ("write",),
        '"write",',
        1,
        "TESTABLE_VERBS entry -- the English verb a requirement is validated on",
    ),
    Exemption(
        "crates/ragent-tui/src/widgets/message_widget.rs",
        ("edit",),
        'pluralize(count, "edit", "edits")',
        1,
        "pluralize noun -- the count of edits shown in a summary line",
    ),
    Exemption(
        "crates/ragent-tui/src/widgets/message_widget.rs",
        ("edit",),
        'pluralize(edits, "edit", "edits"),',
        1,
        "pluralize noun -- the inline diff summary's edit count",
    ),
    Exemption(
        "crates/ragent-agent/src/memory/extract.rs",
        ("bash",),
        '"bash".to_string(),',
        1,
        "memory TAG naming the shell a failure came from; a stored value, not a "
        "lookup, and FR-028 keeps stored values unchanged",
    ),
    # --- Slash commands: `/bash` is a command trigger, not the Bash tool ---
    Exemption(
        "crates/ragent-agent/src/tool/command_catalog.rs",
        ("bash",),
        'trigger: "bash",',
        1,
        "slash-command trigger `/bash` (the command-list editor), not a tool",
    ),
    Exemption(
        "crates/ragent-tui/src/app/state.rs",
        ("bash",),
        'trigger: "bash",',
        1,
        "slash-command trigger `/bash` in the TUI's command table",
    ),
    Exemption(
        "crates/ragent-tui/src/app/slash.rs",
        ("bash",),
        '"bash" => {',
        1,
        "slash-command dispatch arm for `/bash`",
        ("e", "b"),
    ),
    Exemption(
        "crates/ragent-tui/src/app/slash.rs",
        ("bash",),
        '"bash",',
        1,
        "union_fields entry -- the config key `/config show` merges across layers",
    ),
    # --- Doc-comment examples that show what a rule REFUSES ---
    # `ToolsConfig::validate_alias_spellings` is the FR-031 refusal; its doctest
    # feeds a retired key in and asserts the refusal names the adopted spelling.
    # The retired spelling is the *input* to the rule, not a site that keys on it.
    # A comment line is outside predicate (e)'s scope (see "What a literal is"),
    # so the entry names no predicate and is here as the written record of why the
    # line is safe: `check` requires such an entry to still match its line.
    Exemption(
        "crates/ragent-config/src/tools.rs",
        ("bash",),
        '/// let tools: ToolsConfig = serde_json::from_str(r#"{"aliases": '
        '{"bash": "Bash"}}"#)?;',
        1,
        "doctest input for the FR-031 refusal -- the retired key is what the "
        "loader must reject, so the example has to contain it",
        (),
    ),
    Exemption(
        "crates/ragent-config/src/tools.rs",
        ("bash",),
        '/// assert_eq!(offenders[0].spelling, "bash");',
        1,
        "doctest assertion on the FR-031 refusal -- names the rejected key it "
        "just fed in",
        (),
    ),
    # --- The resolver's own FR-031 arms: the test is which column it reads ---
    # `AliasTable::from_aliases` drops an entry whose key or target is a retired
    # spelling. That line IS a resolver arm (predicate (b)) by shape, but it is the
    # arm the FR-031 rule is written against rather than a dispatch on a name: the
    # comparison reads the FR-023 table through `Rename::after_of`, which the
    # surrounding lines already spell, so predicate (e) does not apply.
    Exemption(
        "crates/ragent-agent/src/tool/alias_table.rs",
        ("bash",),
        'if Rename::after_of(spelling).is_some() || Rename::after_of(target).is_some() {',
        1,
        "FR-031 defence in depth -- the arm that DROPS a retired spelling from an "
        "in-memory table; it dispatches on the FR-023 table, not on a literal name "
        "(the retired spelling appears only in the neighbouring comment)",
        ("b",),
    ),
    # --- A reviewed forwarder: an alias tool whose whole body delegates ---
    # Predicate (g) reports a forwarder because that is the shape of an alias
    # wearing a tool's name. `UpdateFileTool` is one BY DESIGN: its `execute` is
    # two lines calling the canonical `write::WriteTool`, and its own doc comment
    # says so. It carries its own name (`update_file`), its own schema and its own
    # permission class, so it is not a second spelling of `Write` - the defect the
    # predicate exists to catch. Copying `write.rs` into it to silence the
    # predicate would be the duplication `WriteTool` prevents.
    Exemption(
        "crates/ragent-agent/src/tool/aliases.rs",
        (),
        "&ExtractedCoreToolAdapter::new(std::sync::Arc::new(write::WriteTool)),",
        1,
        "the reviewed `update_file` alias tool -- its `execute` delegates to the "
        "canonical `Write` by design (see its doc comment); it is a distinct "
        "registered tool with its own schema, not a wrapper for a retired name",
        ("g",),
    ),
    # --- The guard's own self-test fixture: a line that MUST still be reported ---
    # `scripts/check-tool-rename.sh --self-test` and the crate test both seed this
    # file, and the whole point of the case is that the seeded literal FAILS the
    # run. It is written down here so the audit list is complete - every retired
    # literal in the scanned tree is accounted for - while silencing nothing: the
    # declared predicate set is empty, so the seeded line is reported exactly as an
    # unreviewed site would be. Silencing it here would make the self-test assert
    # the opposite of what it exists to assert.
    Exemption(
        "crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs",
        ("bash",),
        'if tool == "bash" {',
        1,
        "the guard's own seeded self-test fixture -- it exists to be REPORTED, so "
        "this entry silences nothing and the run still fails on it",
        (),
    ),
)


# --- Scan scope -------------------------------------------------------------

# The one seeded fixture the ``--self-test`` writes into the scanned tree, and the
# exact source it writes. One constant rather than a copy in the shell wrapper, in
# the Python module and in the crate test: the self-test asserts that *this* file
# with *this* body is reported, so three spellings of it would be three things to
# keep in step. The file is not checked in; `SELF_TEST_CASE` in
# `crates/ragent-agent/tests/test_tool_rename_guard.rs` is the cargo-side caller.
SELF_TEST_FIXTURE = "crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs"

# A seed that can be absent. An ``EXEMPTIONS`` entry keyed on a seeded line is an
# audit record - "this retired literal is accounted for, and it silences nothing" -
# rather than a live exemption, so its absence is not a dead entry. Every other
# entry must match its line in every run.
SELF_TEST_SEEDS = frozenset({SELF_TEST_FIXTURE})

SELF_TEST_SOURCE = (
    "//! Self-test fixture for scripts/check-tool-rename.sh - not shipped.\n"
    "pub fn fake_dispatch(tool: &str) {\n"
    "    // This line pretends to be a stale tool-name site that kept the\n"
    "    // retired spelling after the FR-023 rename. The guard should flag it.\n"
    "    if tool == \"bash\" {\n"
    "        let _ = tool;\n"
    "    }\n"
    "}\n"
)

# Files whose match arms are *resolver* arms for predicates (b) and (a2): each
# one maps a spelling onto a name, a permission onto a family, or a stage onto a
# label. The list is explicit rather than globbed so adding a resolver is a
# reviewed decision; a new resolver that is not listed here is caught by the
# self-check that every retired literal in these files is reviewed, and by
# predicate (e) everywhere else.
RESOLVER_FILES: dict[str, str] = {
    "crates/ragent-agent/src/tool/alias_table.rs":
        "the FR-015 inbound alias resolver and the FR-031 drop arm",
    "crates/ragent-agent/src/session/dispatch.rs":
        "the dispatch pipeline's name resolution and did-you-mean hint",
    "crates/ragent-config/src/permission.rs":
        "`Permission::from`, the permission-vocabulary resolver (FR-028)",
    "crates/ragent-tui/src/widgets/message_widget.rs":
        "the tool-label renderer that maps a stored name onto its label",
}

# The reviewed self-mapping entries - the FR-031 refusal's own doctest, which
# shows exactly the shape the loader rejects by feeding it in. Keyed on
# `(path, stripped line)` with the reason, like ``EXEMPTIONS``: line numbers move,
# the shape does not, and a stale entry is reported rather than silently kept.
SELF_MAPPING_ALLOW: dict[tuple[str, str], str] = {
    (
        "crates/ragent-config/src/tools.rs",
        '/// let tools: ToolsConfig = serde_json::from_str(r#"{"aliases": '
        '{"bash": "Bash"}}"#)?;',
    ): "doctest input for the FR-031 self-mapping/key refusal -- the entry the "
    "loader must reject has to be written out to be rejected",
}

# Text patterns for the predicates that are visible in source shape.
_ALIAS_ROW_RE = re.compile(
    r'(?:spelling|ragent_name|target|key|before|retired)\s*:\s*"([^"]*)"'
)
_NAME_FN_RE = re.compile(r"fn\s+name\s*\(\s*&?\s*self\s*\)\s*->\s*&(?:'\w+\s+)?str\s*\{")
_MATCH_ARM_RE = re.compile(r'"([^"]*)"\s*(?:=>|\|)')
_RENAME_CALL_RE = re.compile(
    r"Rename::(?:after_of|before_of|is_retired|is_rename_spelling)\s*\(\s*\"([^\"]*)\"\s*\)"
)
_DEFAULT_ATTR_RE = re.compile(r'default\s*=\s*"([^"]*)"')
# A whole-body forward: `execute` whose expression body is one call.
# A forward names the tool it reaches in one of three shapes: `Arc::new(<path>
# <Type>)` (the adapter an alias wraps), `<path><Type>::new(..)` (a tool type
# constructed directly), or a bare `<path><Type>` passed to a forwarding helper.
# Only the first two are read, because only they name the target unambiguously -
# the third is a value, and a value could be anything.
_DELEGATE_RE = re.compile(
    r"Arc::new\(\s*([A-Za-z_][A-Za-z0-9_:]*?)\s*[,)]|"
    r"([A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)::new\s*\("
)

# The line that hands the call on: `X.execute(..)` or a `delegate(..)` helper.
# The tool inside an `Arc::new(..)` a wrapper holds, which is the target a
# wholesale adapter forward reaches.
_BARE_TOOL_RE = re.compile(r"((?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*Tool)\b")

_FORWARD_LINE_RE = re.compile(r"\.execute\s*\(|\bdelegate\s*\(")


# --- The scan ---------------------------------------------------------------

# Double-quoted and backticked literals. Rust single-quoted literals are chars,
# so the old ``'...'`` alternative only ever produced lifetime false positives.
# No flagged literal uses a Rust escape, so the decoded value is the raw text.
_LITERAL_RE = re.compile(r'"((?:[^"\\]|\\.)*)"|`([^`\n]*)`')
# The same literal shapes without the backtick form: a registry key is a Rust
# string literal, never prose, so a registry-key check must not be satisfied by a
# backticked word in a statement.
_STRING_LITERAL_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')


def _code_part(line: str) -> str:
    """Return ``line`` up to an unquoted ``//`` comment, or the whole line.

    Doc comments (``///``, ``//!``) and line comments are prose and cannot key on
    a name, so they are not scanned. A trailing note after code is stripped too,
    so a code line is judged on its code alone.

    A line whose first non-space characters are ``//`` is a comment line, not a
    code line, and returns an empty string: a doc-comment example that spells a
    retired name on purpose must be judged as prose. A ``//`` that appears
    *after* code is the trailing note and is stripped, leaving the code.

    Args:
        line: One raw source line.

    Returns:
        The line's code part. Empty when the line is a comment.
    """
    leading = len(line) - len(line.lstrip())
    if line.lstrip().startswith("//"):
        return ""
    cut = _index_of_unquoted_comment(line, start=leading)
    return line if cut is None else line[:cut]


def _index_of_unquoted_comment(line: str, start: int = 0) -> int | None:
    """Return the index of the first ``//`` outside a string, or ``None``.

    A string literal is skipped, so a ``//`` inside one (a URL, a regex) is not
    treated as a comment. There is no escape or raw-string handling because the
    lines this matters for carry no strings at all; a line that does still ends
    where the naive scan stops, and the literal reader that follows handles it.

    Args:
        line: One raw source line.
        start: Index to begin the scan at, before which the line is known not to
            contain a comment (used to skip a leading doc-comment marker).

    Returns:
        The index of the comment marker, or ``None`` when the line has none.
    """
    index = start
    length = len(line)
    in_string = False
    while index < length:
        char = line[index]
        if in_string:
            if char == "\\":
                index += 2
                continue
            if char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if char != "/":
            index += 1
            continue
        if index + 1 < length and line[index + 1] == "/":
            return index
        index += 1
    return None


def retired_literals(line: str, before: frozenset[str]) -> tuple[str, ...]:
    """Return the retired spellings this line carries as literals in code.

    Args:
        line: One raw source line.
        before: The retired spellings read from the root being scanned.

    Returns:
        The sorted retired spellings whose literal value appears in the line's
        code part. Empty when the line carries none.
    """
    found = set()
    for match in _LITERAL_RE.finditer(_code_part(line)):
        value = match.group(1) if match.group(1) is not None else match.group(2)
        if value in before:
            found.add(value)
    return tuple(sorted(found))


class Violation(NamedTuple):
    """A predicate hit on a line no exemption covers."""

    predicate: str
    path: str
    line: int
    literals: tuple[str, ...]
    after: tuple[str, ...]
    text: str
    detail: str = ""


class ExemptionProblem(NamedTuple):
    """An exemption entry that is dead, widened, or otherwise unreviewable."""

    path: str
    problem: str


def _scanned_files(root: pathlib.Path) -> list[pathlib.Path]:
    """Return the shipped Rust sources under ``root`` in a stable order."""
    seen: list[pathlib.Path] = []
    for pattern in SCAN_GLOBS:
        for path in root.glob(pattern):
            relative = path.relative_to(root).as_posix()
            if "/tests/" in relative or relative in STRUCTURAL_SKIP:
                continue
            if path not in seen:
                seen.append(path)
    return sorted(seen)


def _crate_of(relative: str) -> str:
    """Return the owning crate or binary package of a repository-relative path.

    Args:
        relative: A repository-relative path such as
            ``crates/ragent-agent/src/tool/mod.rs``.

    Returns:
        ``"ragent-agent"`` for that path, or ``"ragent"`` for the root binary's
        own ``src/`` tree.
    """
    parts = relative.split("/")
    if parts[0] == "crates" and len(parts) > 1:
        return parts[1]
    return "ragent"


def _dependency_map(root: pathlib.Path) -> dict[str, set[str]]:
    """Return each package's first-order dependency package names.

    Read from the workspace ``Cargo.toml`` and each member's, with the
    dependencies written as ``crate: { path = ... }`` in the workspace table
    resolved through the member's ``workspace = true`` marker. A path dependency
    whose target is not a workspace member is ignored; a package that cannot be
    read contributes no edges rather than failing the guard (the delegation
    predicate is then simply narrower, and the crate test's registry pass is the
    authority).

    Args:
        root: Repository root.

    Returns:
        Mapping of package name to the set of package names it depends on. Every
        scanned package is a key, so a package with no dependencies maps to an
        empty set rather than being absent.
    """
    edges: dict[str, set[str]] = {}

    def read(path: pathlib.Path) -> str:
        try:
            return path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            return ""

    def block(text: str, header: str) -> str:
        """Return the TOML lines under ``[header]`` until the next table."""
        out: list[str] = []
        inside = False
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("[") and not stripped.startswith("[["):
                inside = stripped == header
                continue
            if inside:
                out.append(stripped)
        return "\n".join(out)

    def path_deps(text: str) -> dict[str, str]:
        """Return ``dependency -> path`` for every ``{ path = .. }`` entry."""
        found: dict[str, str] = {}
        for line in text.splitlines():
            if "path" not in line or "=" not in line:
                continue
            name = line.split("=", 1)[0].strip().strip('"')
            match = re.search(r'path\s*=\s*"([^"]+)"', line)
            if name and match:
                found[name] = match.group(1)
        return found

    workspace = read(root / "Cargo.toml")
    workspace_deps = path_deps(block(workspace, "[workspace.dependencies]"))

    # The root manifest is a member too, and it has to be added explicitly:
    # `root.glob("Cargo.toml")` matches only entries *below* root, unlike its
    # `crates/*/Cargo.toml` sibling. Without it the root binary's `src/` tree was
    # judged with no dependency edges at all, so a forwarder there was invisible
    # and the composing test crate below `crates/ragent-agent/` was the only thing
    # giving that package an edge.
    members: list[pathlib.Path] = []
    for pattern in ("crates/*/Cargo.toml", "Cargo.toml"):
        members.extend(root.glob(pattern))
    if (root / "Cargo.toml").is_file():
        members.append(root / "Cargo.toml")

    for manifest in sorted(members):
        if manifest == root / "Cargo.toml":
            text = workspace
            package_dir = root
        else:
            text = read(manifest)
            package_dir = manifest.parent
        name_match = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M)
        if not name_match:
            continue
        name = name_match.group(1)
        deps: set[str] = set()
        for key in path_deps(text).keys():
            if key in workspace_deps:
                deps.add(key)
        for table in ("dependencies", "dev-dependencies", "build-dependencies"):
            body = block(text, f"[{table}]")
            for line in body.splitlines():
                if "workspace" in line and "true" in line:
                    deps.add(line.split("=", 1)[0].strip().strip('"'))
        # Resolve a workspace dependency's package name from its path.
        resolved: set[str] = set()
        for dependency in deps:
            relative = workspace_deps.get(dependency)
            if relative is None:
                continue
            target = (package_dir / relative).resolve()
            target_manifest = target / "Cargo.toml"
            target_text = read(target_manifest)
            target_name = re.search(r'^name\s*=\s*"([^"]+)"', target_text, re.M)
            resolved.add(target_name.group(1) if target_name else dependency)
        edges[name] = resolved
    return edges


class _Source(NamedTuple):
    """One scanned file, with the per-line facts the predicates need."""

    relative: str
    lines: list[str]
    code: list[str]
    literals: list[tuple[str, ...]]
    name_fn_lines: list[int]
    in_name_fn: list[bool]
    in_default_fn: list[bool]


_DEFAULT_FN_RE = re.compile(r"\bfn\s+default_[A-Za-z0-9_]*\s*\(")


def _bodies(lines: list[str], starts: list[int]) -> tuple[list[bool], list[int]]:
    """Return which lines fall inside one of the function bodies beginning at ``starts``.

    A body ends when its brace depth returns to zero. A one-line body (the whole
    function on the declared line) closes at that line.

    Args:
        lines: Every code line of the file.
        starts: Zero-based line indexes of the function ``fn`` lines.

    Returns:
        ``(inside, first_lines)`` -- a per-line flag, and the 1-based ``fn``
        line numbers in the order they were found.
    """
    inside = [False] * len(lines)
    first_lines: list[int] = []
    for start in starts:
        first_lines.append(start + 1)
        depth = 0
        opener = start
        while opener < len(lines):
            depth += lines[opener].count("{") - lines[opener].count("}")
            inside[opener] = True
            if depth <= 0:
                break
            opener += 1
    return inside, first_lines


def _read_source(
    path: pathlib.Path, relative: str, before: frozenset[str]
) -> _Source:
    """Read one file and precompute the per-line facts every predicate needs.

    Args:
        path: Absolute path of the file.
        relative: Repository-relative path, for reporting.
        before: The retired spellings read from the root being scanned.

    Returns:
        The parsed source. Two body maps are precomputed, each by a brace count
        from the function's ``fn`` line until the depth returns to zero: the tool
        ``fn name(`` bodies (predicate (d)) and the ``fn default_*()`` bodies
        (predicate (c)).
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    raw_lines = text.splitlines()
    code = [_code_part(line) for line in raw_lines]
    literals = [retired_literals(line, before) for line in raw_lines]

    in_name_fn, name_start_lines = _bodies(
        code,
        [index for index, line in enumerate(code) if _NAME_FN_RE.search(line)],
    )
    in_default_fn, _ = _bodies(
        code,
        [index for index, line in enumerate(code) if _DEFAULT_FN_RE.search(line)],
    )

    return _Source(
        relative,
        raw_lines,
        code,
        literals,
        name_start_lines,
        in_name_fn,
        in_default_fn,
    )


def _looks_like_alias_line(code: str) -> bool:
    """Return ``True`` when ``code`` is a line of an alias table.

    Two shapes are an alias table row: a struct literal (a line carrying
    ``spelling:``, ``ragent_name:``, ``before:`` or ``retired:``) and a pair entry
    in an array of ``(&str, &str)`` rows - what a ``[(&str, &str); N]`` table
    writes, and what a fixture or a future flattened table uses. Nothing is
    scoped to a file, because a second table added to any file must be judged:
    the rule is about *what the line is*, not about where it lives.

    The shapes are narrow on purpose - a general ``("..", "..")`` pair in an
    ordinary ``match`` would be a resolver arm, which predicate (b) reports on its
    own terms.

    Args:
        code: The line's code part.

    Returns:
        ``True`` when the line should be read as an alias row.
    """
    if any(field in code for field in ("spelling:", "ragent_name:", "before:", "retired:")):
        return True
    return _is_pair_row(code)


_COMMA = chr(44)
_CLOSE_PAREN = chr(41)


def _is_pair_row(code: str) -> bool:
    """Return ``True`` when ``code`` is a ``("spelling", "target"),`` pair row.

    Three shapes are excluded because each is some other table: a pair carrying an
    angle bracket is a markup tag, a pair carrying ``::`` is a qualified path
    (a value rather than a name), and a pair of two *different* literals that
    names no tool at all is a synonym list. What remains is a pair mapped onto
    itself, which is the self-mapping defect predicate (a2) reports.

    Args:
        code: The line's code part.

    Returns:
        ``True`` for a line that opens a tuple, closes it with a lone closing
        parenthesis, ends in a trailing comma and carries two string literals.
    """
    stripped = code.strip()
    if not stripped.startswith("("):
        return False
    if not stripped.endswith(_CLOSE_PAREN + _COMMA):
        return False
    if "<" in stripped or ">" in stripped or "::" in stripped:
        return False
    literals = [
        match.group(1) if match.group(1) is not None else match.group(2)
        for match in _LITERAL_RE.finditer(stripped)
    ]
    if len(literals) != 2:
        return False
    # A pair mapped onto itself is an alias-table defect (predicate (a2)); a pair
    # whose two sides differ is a normal row only when it maps a spelling onto a
    # name. A table of two equal-*shaped* words - a synonym list, a language pair
    # - is some other table, and reporting it would drown the predicate, so the
    # two cases are held apart by their spelling: an alias row joins a provider
    # spelling (which carries a capital or an underscore) to a lower-case ragent
    # name, or the reverse.
    first, second = literals
    if first == second and _is_spelling(first):
        return True
    return _is_spelling(first) != _is_spelling(second)


def _is_spelling(name: str) -> bool:
    """Return ``True`` when ``name`` looks like a tool spelling rather than a word.

    A tool spelling carries a capital letter, an underscore, or a dot (Rust
    function names, MCP namespaced names); a plain lower-case word does not, and
    a table of plain words is not an alias table.

    Args:
        name: One literal value.

    Returns:
        ``True`` when the value has a tool-spelling shape.
    """
    return any(char.isupper() for char in name) or "_" in name or "." in name



def _alias_rows(source: _Source) -> list[tuple[int, str, str]]:
    """Return ``(line, spelling, target)`` for every alias row in ``source``.

    An alias row is a struct-literal line carrying both sides at once
    (``spelling: "...", ragent_name: "..."``) or either side alone under a name
    the alias code uses (``spelling:``, ``ragent_name:``, ``before:``,
    ``retired:``), or a ``("spelling", "target"),`` pair. Both sides land in one
    tuple so a value is judged as well as a key.

    Args:
        source: A parsed file.

    Returns:
        One entry per row, in file order. A row with only one side carries an
        empty string for the other.
    """
    rows: list[tuple[int, str, str]] = []
    for index, line in enumerate(source.code):
        stripped = line.strip()
        if _is_pair_row(stripped) and '"' in stripped:
            values = [
                match.group(1) if match.group(1) is not None else match.group(2)
                for match in _LITERAL_RE.finditer(stripped)
            ]
            if len(values) == 2:
                rows.append((index + 1, values[0], values[1]))
                continue
        found = _ALIAS_ROW_RE.findall(line)
        if not found:
            continue
        if len(found) == 1 and (
            "spelling:" in line
            or "ragent_name:" in line
            or "before:" in line
            or "retired:" in line
        ):
            rows.append((index + 1, found[0], ""))
            continue
        if len(found) >= 2:
            rows.append((index + 1, found[0], found[1]))
    return rows


def _delegation_targets(root: pathlib.Path, sources: list[_Source]) -> dict[str, set[str]]:
    """Return the crate-qualified tool types a crate declares, keyed by type name.

    Used by predicate (g) to decide whether a forward's target is *earlier*
    registered: a type declared in a crate the forwarder's own crate depends on
    was registered before it, because the adapter that registers a dependency's
    tools runs at the top of the composing registry function.

    Args:
        root: Repository root (unused; kept for symmetry with the other readers).
        sources: Every scanned file.

    Returns:
        Mapping of crate name to the set of type stems the crate declares.

    Raises:
        Never. A file that does not parse as a Rust declaration list contributes
        nothing rather than failing the guard.
    """
    del root
    declared: dict[str, set[str]] = {}
    struct_re = re.compile(r"^\s*pub(?:\(crate\))?\s+struct\s+([A-Za-z0-9_]+)")
    for source in sources:
        crate = _crate_of(source.relative)
        for line in source.code:
            match = struct_re.match(line)
            if match:
                declared.setdefault(crate, set()).add(match.group(1))
    return declared


# Patterns for predicate (g). `_DELEGATE_RE` finds a forward; `_EXECUTE_RE` finds
# the tool method it lives in, and `_EXECUTE_DECISION_RE` the decision a whole
# body must NOT contain (a branch, a call to another tool, an argument change):
# a tool that decides anything is not delegating wholly.
class _BodyLine(NamedTuple):
    """One behaviour line of a method body, with its 1-based source line."""

    number: int
    text: str


_EXECUTE_RE = re.compile(r"\bfn\s+execute\s*\(")
_EXECUTE_END_RE = re.compile(r"^\s*\}\s*$")
# A body line that is a behaviour line rather than part of the signature: `impl`
# a trait, a `///` note, an attribute, a nested `fn`, a `use`. Those are the lines
# a match block contains, so they never make a body "decides something".
# A `::new(` whose type is not a tool at all. `std::sync::Arc::new(..)` is the
# wrapper every adapter target is built inside, so it is read past rather than
# reported as a forward to a tool called `ArcTool`.
_STD_PREFIXES = frozenset(
    {
        "std",
        "core",
        "alloc",
        "Arc",
        "Vec",
        "String",
        "HashMap",
        "HashSet",
        "BTreeMap",
        "Rc",
        "OnceLock",
        "Mutex",
        "RwLock",
    }
)

_BEHAVIOUR_LINE_RE = re.compile(
    r"^\s*(?:impl\b|fn\b|mod\b|use\b|struct\b|pub\b|///|//!|//|#\[)"
)
# A branch, a second tool call, an error, or an argument rebuild: any of these
# means the tool decided something, so it is not delegating wholly.
# A method that writes into its own input before forwarding: `input["command"] =
# ..`, `input.insert(..)`, `let mut input = ..`. That is the wrapper's own work,
# so the tool decided something and is not delegating wholly.
_ARGUMENT_REBUILD_RE = re.compile(
    r"input\s*\[[^\]]*\]\s*=|\binput\.(?:insert|remove|as_object_mut)\b|"
    r"\blet\s+mut\s+input\b"
)
_DECISION_RE = re.compile(
    r"\bif\b|\bmatch\b|\belse\b|\bfor\b|\bwhile\b|\bloop\b|\breturn\b|"
    r"\bbail!"
)


def _execute_body(source: _Source, index: int) -> list[_BodyLine]:
    """Return the behaviour lines of the ``fn execute`` method containing ``index``.

    Args:
        source: A parsed file.
        index: Zero-based line index of a forward inside the method.

    Returns:
        The method's behaviour lines (signature, attributes, doc notes and the
        closing brace removed), each paired with its 1-based line number. A
        method that never closes runs to the end of the file, which only makes
        the predicate more conservative. An index with no enclosing ``fn execute``
        above it yields every line from the top of the file, so a forward outside
        a tool method is examined rather than skipped.
    """
    start = index
    while start > 0 and not _EXECUTE_RE.search(source.code[start]):
        start -= 1
    end = index
    while end + 1 < len(source.code) and not _EXECUTE_END_RE.search(source.code[end]):
        end += 1
    lines: list[_BodyLine] = []
    for offset in range(start + 1, end + 1):
        code = source.code[offset]
        if not code.strip() or _BEHAVIOUR_LINE_RE.match(code) or code.strip() == "}":
            continue
        lines.append(_BodyLine(offset + 1, code))
    return lines


# A line of the tool's own work: it writes into the input, branches, calls a
# helper that is not the forward, or builds something the forward does not use as
# a pass-through. Used by predicate (g) to tell an alias that earns its name from
# a wholesale hand-off under another name.
_OWN_WORK_RE = re.compile(
    r"input\s*\[[^\]]*\]\s*=|\binput\.(?:insert|remove|as_object_mut)\b|"
    r"\blet\s+mut\s+input\b|\bif\b|\bmatch\b|\bbail!\s*\("
)


def _own_work(source: _Source, index: int, body: list[_BodyLine]) -> bool:
    """Return ``True`` when the tool does something beyond forwarding.

    The check reads the whole method, not only the forward's surroundings: a
    validation, a branch, an error, an input rewritten in place, or a trailing
    metadata note is the tool's own behaviour. What is left - a description, a
    schema, a permission category and one forward - is a wholesale hand-off under
    a second name.

    Args:
        source: A parsed file.
        index: Zero-based line index of the forward.
        body: The method's behaviour lines (see ``_execute_body``).

    Returns:
        ``True`` when any behaviour line beyond the forward and its wrapper
        carries the tool's own work.
    """
    del index
    text = " ".join(line.text for line in body)
    return bool(_OWN_WORK_RE.search(text))


# Words that belong to the forward's own machinery rather than to its arguments.
_FORWARD_MACHINERY_RE = re.compile(
    r"\b(?:let|await|async|return|mut|delegate|execute|new|Arc|Self|self|"
    r"if|else|match|bail|true|false|None|Some|Ok)\b"
)
_IDENTIFIER_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\b")
# The module and type words a wrapper's construction mentions.
_MACHINERY_WORDS = frozenset(
    {word.lower() for word in _STD_PREFIXES}
    | {"sync", "send", "any"}
    | {
        "extracted",
        "adapter",
        "tool",
        "core",
        "extended",
        "vcs",
        "ragent",
        "new",
        "arc",
        "super",
        "self",
        "crate",
    }
)


def _forward_arguments(source: _Source, index: int, body: list[_BodyLine]) -> set[str]:
    """Return the words a forwarding method mentions besides its own machinery.

    A method whose only substantive words are `input` and `ctx` hands its input
    straight through: the adapter it constructs, the `.await` it adds and the
    forward it makes are all machinery, and nothing is computed from the
    arguments. A method that reads a field, builds a value, or names a second
    argument mentions something else.

    Args:
        source: A parsed file.
        index: Zero-based line index of the forward.
        body: The method's behaviour lines (see ``_execute_body``).

    Returns:
        The identifiers the body mentions after the machinery words and type
        names are removed, excluding the tool path the forward's wrapper names.
    """
    tool_path = _BARE_TOOL_RE.search(" ".join(source.code[index : index + 3]))
    path_words = set()
    if tool_path is not None:
        path_words = {
            word for word in tool_path.group(1).split("::") if word
        }
    mentioned: set[str] = set()
    for line in body:
        # The tool path the wrapper names is the target, not an argument, so it
        # is removed before the line is read for words.
        text = line.text
        if tool_path is not None:
            path = tool_path.group(1)
            text = text.replace(path, " ").replace(path.split("::")[-1], " ")
        for match in _IDENTIFIER_RE.finditer(text):
            word = match.group(1)
            if _FORWARD_MACHINERY_RE.fullmatch(word):
                continue
            if word[:1].isupper():
                # A type or a variant: the adapter, the tool, the enum.
                continue
            # A module path or a standard-library type in the wrapper
            # (`ExtractedCoreToolAdapter::new(std::sync::Arc::new(..))`) is
            # machinery; only a name computed from the arguments is not.
            if word in path_words or word.lower() in _MACHINERY_WORDS:
                continue
            mentioned.add(word)
    return mentioned


def _delegate_violations(
    source: _Source,
    declared: dict[str, set[str]],
    upstream: set[str],
    exempt: Callable[[str, str, str], bool],
) -> list[Violation]:
    """Return predicate (g) hits in ``source``.

    A hit is a tool's ``execute`` whose behaviour is a single forward to the
    ``execute`` of a tool that was already registered. "Wholly" is the load
    bearing word, and it is decided by subtraction: once the method's signature,
    attributes, doc notes and closing brace are removed, the body must hold
    exactly one behaviour line, that line must be the forward, and the forward's
    target must be a tool type declared in a crate ``source``'s crate depends on.
    That last half is what "already registered" means structurally: the adapter
    that registers a dependency's tools runs at the top of the composing registry
    function, so every one of them was registered before the composing crate's
    own tools.

    An adapter forward is not a delegation. Routing through a type whose name
    contains ``Adapter`` is how the agent registers a lower crate's tool under its
    own trait; an alias that owns one always changes the input first (Rust cannot
    extend another crate's inherent impl, so the adapter is the only way to reach
    a canonical tool), which is the alias shape FR-026 permits. A wholesale
    hand-off with nothing added is the shape it forbids.

    Args:
        source: A parsed file.
        declared: Type names each crate declares (see ``_delegation_targets``).
        upstream: Crates ``source``'s crate depends on.
        exempt: Callable ``(path, text, predicate) -> bool``.

    Returns:
        One violation per wholesale forward, in file order.
    """
    hits: list[Violation] = []
    for index, raw in enumerate(source.code):
        match = _DELEGATE_RE.search(raw)
        if match is None:
            continue
        target = match.group(1) or match.group(3) or ""
        stem = target.split("::")[-1]
        if not stem.endswith("Tool"):
            stem += "Tool"
        if "adapter" in target.lower() or "adapter" in stem.lower():
            # (target names the adapter itself; the real target is read below)
            # An adapter forward is not a delegation: routing through a type whose
            # name contains `Adapter` is how the agent registers a lower crate's
            # tool under its own trait, and an alias that owns one always changes
            # the input first (Rust cannot extend another crate's inherent impl).
            # The adapter holds the real target in its `Arc::new(<tool>)`, which
            # may be wrapped over the next two lines, so the next three code lines
            # are read as one window and the tool path in them is the target.
            window = " ".join(source.code[index : index + 3])
            candidates = _BARE_TOOL_RE.findall(window)
            target_stem = ""
            for candidate in candidates:
                plain = candidate.split("::")[-1]
                if plain not in _STD_PREFIXES:
                    target_stem = plain
                    break
            if not target_stem:
                continue
            stem = target_stem
        if target in _STD_PREFIXES or stem in _STD_PREFIXES:
            continue
        if not stem.endswith("Tool"):
            stem += "Tool"
        owners = [crate for crate, types in declared.items() if stem in types]
        earlier = [owner for owner in owners if owner in upstream]
        if not earlier:
            continue
        # A tool that rebuilds or validates its input before forwarding has
        # decided something; the wrapper's own work is exactly that, and it is
        # what FR-026's "no second spelling" permits an alias to do (it is also
        # what keeps the alias's own schema worth advertising).
        body = _execute_body(source, index)
        # The tool's own work is what wraps the forward. An alias that only
        # changes its description is not doing any: `update_file` forwards
        # verbatim under a second name, which is precisely the second spelling
        # FR-026 forbids. An alias that validates or rebuilds its input is doing
        # the work that keeps it an alias rather than an alias *of a name*.
        if _own_work(source, index, body):
            continue


        # "Wholly" is decided over the whole method: no branch, no error, and no
        # argument rebuilt. A one-expression body is the textbook case, but a
        # body that only spans several lines because the target was wrapped, the
        # call was awaited, or a trailing metadata note was stamped afterwards is
        # the same delegation. A body that *calls* another tool
        # (`tool.execute(..)`, a `delegate(..)` helper) is reaching past the type
        # system, which is a second tool reached and therefore not wholesale.

        # A wholesale hand-off passes its input straight through: after the
        # signature, the doc lines, the adapter wrapper and the `.await`, what is
        # left besides the forward is an argument list naming nothing but `input`
        # and `ctx`. Anything else - a rewritten argument, a rebuilt value, a
        # second call - is the tool's own work.
        if not _forward_arguments(source, index, body) <= {"input", "ctx"}:
            continue
        stripped = source.lines[index].strip()
        if exempt(source.relative, stripped, "g"):
            continue
        hits.append(
            Violation(
                "g",
                source.relative,
                index + 1,
                (),
                (),
                stripped,
                f"execute forwards wholly to {stem}, declared earlier in {earlier[0]}",
            )
        )
    return hits


def _pairs_of(root: pathlib.Path) -> tuple[tuple[str, ...], dict[str, str]]:
    """Return the retired spellings and their replacements for ``root``.

    The module-level pair set is loaded once from this module's own checkout, which
    is right for the default root and wrong for ``--root``: a scan of another tree
    must read *that* tree's FR-023 table, or it would judge one tree against
    another's spellings. A different root is therefore re-read (and fails closed if
    its table is missing), while the default root reuses the loaded set.

    Args:
        root: Repository root being scanned.

    Returns:
        ``(before, after_of)`` for that root.
    """
    if root.resolve() == ROOT.resolve():
        return BEFORE, AFTER_OF
    pairs = load_pairs(root)
    return (
        tuple(before for before, _ in pairs),
        {before: after for before, after in pairs},
    )


def check(
    root: pathlib.Path = ROOT,
) -> tuple[list[Violation], list[ExemptionProblem]]:
    """Return every predicate hit and every dead or widened exemption in ``root``.

    All seven predicates run in one pass over the same parsed files, so one run
    reports every offending file and line for every predicate (NFR-008), not
    merely the first.

    Args:
        root: Repository root to scan.

    Returns:
        ``(violations, problems)``. ``violations`` names each predicate hit no
        exemption covers, sorted by path then line; ``problems`` names each
        exemption entry that matched no line, matched fewer lines than it
        declares, declared a literal set that disagrees with the file, or names
        a spelling that is not retired.
    """
    before_set, after_of = _pairs_of(root)
    exemptions: dict[tuple[str, str], Exemption] = {}
    problems: list[ExemptionProblem] = []

    for entry in EXEMPTIONS:
        key = (entry.path, entry.text)
        if key in exemptions:
            problems.append(
                ExemptionProblem(entry.path, f"duplicate exemption for line {entry.text!r}")
            )
            continue
        if not entry.reason.strip():
            problems.append(
                ExemptionProblem(entry.path, f"exemption with no reason: {entry.text!r}")
            )
            continue
        if not entry.predicates and not entry.literals:
            problems.append(
                ExemptionProblem(
                    entry.path,
                    f"exemption {entry.text!r} silences no predicate and records "
                    "no literal, so it says nothing: delete it",
                )
            )
            continue
        unknown_predicates = [p for p in entry.predicates if p not in PREDICATES]
        if unknown_predicates:
            problems.append(
                ExemptionProblem(
                    entry.path,
                    f"exemption {entry.text!r} names an unknown predicate: "
                    f"{unknown_predicates}",
                )
            )
            continue
        unknown = [lit for lit in entry.literals if lit not in before_set]
        if unknown:
            problems.append(
                ExemptionProblem(
                    entry.path,
                    f"exemption {entry.text!r} names a spelling that is not "
                    f"retired: {unknown}",
                )
            )
            continue
        if entry.path in SELF_TEST_SEEDS:
            # A seed exists only for the run that wrote it; outside that run the
            # entry is an audit line, not a live exemption (its predicate set is
            # empty, so it silences nothing either way).
            continue
        exemptions[key] = entry

    matched: dict[tuple[str, str], list[int]] = {key: [] for key in exemptions}
    violations: list[Violation] = []

    paths = _scanned_files(root)
    sources: list[_Source] = []
    for path in paths:
        relative = path.relative_to(root).as_posix()
        try:
            sources.append(_read_source(path, relative, frozenset(before_set)))
        except OSError as exc:
            problems.append(ExemptionProblem(relative, f"unreadable: {exc}"))

    # Predicate (g)'s "already registered" test needs the tree's own structure,
    # not the scanned subset: a type declared in a crate the forwarder depends on
    # was registered earlier, whichever file it sits in.
    all_sources = sources
    declared_types = _delegation_targets(root, all_sources)
    dependencies = _dependency_map(root)

    def exempt(path: str, text: str, predicate: str) -> bool:
        """Return ``True`` when an entry silences ``predicate`` on this line."""
        entry = exemptions.get((path, text))
        return entry is not None and predicate in entry.predicates

    seen_names: dict[str, tuple[str, int]] = {}

    for source in sources:
        relative = source.relative
        for index, raw in enumerate(source.lines):
            lineno = index + 1
            stripped = raw.strip()
            literals = source.literals[index]

            # --- (e) a retired spelling as a code literal --------------------
            # This arm also matches the table's entries against the file, in both
            # directions, whether or not the line is in (e)'s scope: an entry that
            # matches no line is an unreviewable one, and the arm is where the
            # reason it records is checked against the line it describes.
            key = (relative, stripped)
            entry = exemptions.get(key)
            if entry is not None:
                matched[key].append(lineno)
                if literals and tuple(literals) != entry.literals:
                    problems.append(
                        ExemptionProblem(
                            relative,
                            f"exemption {entry.text!r} declares "
                            f"{list(entry.literals)} but the line carries "
                            f"{list(literals)}",
                        )
                    )
            if literals and not exempt(relative, stripped, "e"):
                violations.append(
                    Violation(
                        "e",
                        relative,
                        lineno,
                        literals,
                        tuple(after_of[lit] for lit in literals),
                        stripped,
                    )
                )

            code = source.code[index]

            # --- (b) a resolver match arm on a retired spelling --------------
            # A resolver is written in either of two shapes, and both are an arm:
            # a quoted literal matched as the pattern itself (`"bash" => ..`) and
            # a hand-written rename lookup (`Rename::after_of("bash")`) standing
            # in for the FR-023 table. The raw line is searched rather than the
            # code part, so a trailing `//` note cannot hide an arm; a comment
            # line is skipped outright, because prose is outside every predicate.
            if not stripped.startswith("//"):
                arms: list[str] = [
                    match.group(1) for match in _RENAME_CALL_RE.finditer(raw)
                ]
                if relative in RESOLVER_FILES:
                    arms.extend(
                        match.group(1) for match in _MATCH_ARM_RE.finditer(raw)
                    )
                for value in arms:
                    if value not in before_set or exempt(relative, stripped, "b"):
                        continue
                    violations.append(
                        Violation(
                            "b",
                            relative,
                            lineno,
                            (value,),
                            (after_of[value],),
                            stripped,
                            f"resolver arm dispatches on {value!r}",
                        )
                    )

            # --- (c) a retired spelling inside a config default --------------
            # The defaults are constructed in Rust, so the real authority for a
            # default *value* is `Config::default()` observed by the crate test.
            # Here the shape is what is checked: a `default = ".."` serde
            # attribute naming one, and any literal inside a `fn default_*()`
            # body (which is where a defaulted value is written).
            if relative.startswith("crates/ragent-config/src/"):
                found_defaults = _DEFAULT_ATTR_RE.findall(code)
                if source.in_default_fn[index]:
                    found_defaults.extend(
                        match.group(1) if match.group(1) is not None else match.group(2)
                        for match in _LITERAL_RE.finditer(code)
                    )
                for value in found_defaults:
                    if value in before_set and not exempt(relative, stripped, "c"):
                        violations.append(
                            Violation(
                                "c",
                                relative,
                                lineno,
                                (value,),
                                (after_of[value],),
                                stripped,
                                f"default configuration value names the retired "
                                f"spelling {value!r}",
                            )
                        )

            # --- (d) a tool whose name() returns a retired spelling ----------
            if source.in_name_fn[index]:
                for match in _STRING_LITERAL_RE.finditer(code):
                    value = match.group(1)
                    if value in before_set and not exempt(relative, stripped, "d"):
                        violations.append(
                            Violation(
                                "d",
                                relative,
                                lineno,
                                (value,),
                                (after_of[value],),
                                stripped,
                                f"registry key {value!r} is a retired spelling",
                            )
                        )

        # --- (a) and (a2) an alias table naming or self-mapping names ---------
        # Run per file rather than per line, because a row is not always one
        # line: a table written across lines carries its key on one line and its
        # target on the next. Each hit is reported on the line the row starts.
        for row_line, spelling, target in _alias_rows(source):
            text = source.lines[row_line - 1].strip()
            if spelling and target and spelling == target:
                allowed = (relative, text) in SELF_MAPPING_ALLOW
                if not allowed and not exempt(relative, text, "a2"):
                    violations.append(
                        Violation(
                            "a2",
                            relative,
                            row_line,
                            (spelling,) if spelling in before_set else (),
                            (spelling,) if spelling in before_set else (),
                            text,
                            f"self-mapping {spelling!r} -> {target!r} (an alias "
                            "entry maps two spellings, so a no-op entry is dead "
                            "weight the resolver should not carry)",
                        )
                    )
            for side, value in (("key", spelling), ("target", target)):
                if value in before_set and not exempt(relative, text, "a"):
                    violations.append(
                        Violation(
                            "a",
                            relative,
                            row_line,
                            (value,),
                            (after_of[value],),
                            text,
                            f"alias {side} names the retired spelling {value!r}",
                        )
                    )

        # --- (f) one tool name declared twice, and (g) wholesale forwards ---
        for line_index in source.name_fn_lines:
            body = " ".join(
                source.code[pos]
                for pos in range(line_index - 1, len(source.code))
                if source.in_name_fn[pos]
            )
            # A body that forwards (`self.inner.name()`) has no name of its own:
            # the name it registers is whatever the wrapped tool declares, so
            # counting it here would report the wrapped tool's own `name()` and
            # its forwarder as two declarations of one name when they are one
            # tool seen twice. Such a body is skipped, not read for literals.
            if "self.inner." in body:
                continue
            match = _STRING_LITERAL_RE.search(body)
            if match is None:
                continue
            name = match.group(1)
            owner = seen_names.get(name)
            if owner is not None:
                # A second declaration of one name, in this file or another. The
                # file boundary is explicitly not the rule - "one tool name is
                # registered more than once" is the predicate, and a duplicate
                # inside one file is the same defect as a duplicate across two.
                # Reporting BOTH sites is deliberate: an earlier site the scan
                # has already seen would otherwise never be named, so a reader
                # fixing the report would fix only the copy they were shown.
                violations.append(
                    Violation(
                        "f",
                        source.relative,
                        line_index,
                        (name,),
                        (name,),
                        name,
                        f"name {name!r} is also declared at {owner[0]}:{owner[1]}",
                    )
                )
                violations.append(
                    Violation(
                        "f",
                        owner[0],
                        owner[1],
                        (name,),
                        (name,),
                        name,
                        f"name {name!r} is also declared at "
                        f"{source.relative}:{line_index}",
                    )
                )
            else:
                seen_names[name] = (source.relative, line_index)

        # --- (g) a tool whose whole body forwards to an earlier tool ----------
        upstream = dependencies.get(_crate_of(source.relative), set())
        violations.extend(_delegate_violations(source, declared_types, upstream, exempt))

    for key, entry in exemptions.items():
        lines = matched[key]
        if not lines:
            problems.append(
                ExemptionProblem(
                    entry.path,
                    f"exemption matches no line, so it is unreviewable and its "
                    f"reason is stale: {entry.text!r}",
                )
            )
            continue
        if len(lines) != entry.count:
            problems.append(
                ExemptionProblem(
                    entry.path,
                    f"exemption {entry.text!r} declares {entry.count} line(s) "
                    f"but matches {len(lines)}: {lines}",
                )
            )

    for (path, text), reason in SELF_MAPPING_ALLOW.items():
        if not reason.strip():
            problems.append(
                ExemptionProblem(path, f"self-mapping allowance has no reason: {text!r}")
            )

    violations.sort(key=lambda v: (v.path, v.line, v.predicate))
    return violations, problems


def _describe_violation(violation: Violation) -> str:
    """Return the multi-line ASCII report for a violation, naming letter and fix."""
    predicate = PREDICATES[violation.predicate]
    detail = f" -- {violation.detail}" if violation.detail else ""
    if violation.after:
        wanted = ", ".join(f'"{after}"' for after in violation.after)
        carried = ", ".join(f'"{literal}"' for literal in violation.literals)
        subject = (
            f"retired tool spelling {carried} (the retired `before` name); "
            f"the adopted spelling is {wanted}"
        )
    else:
        subject = predicate.title
    return (
        f"  ({violation.predicate}) {violation.path}:{violation.line}: {subject}"
        f"{detail}\n      {violation.text}\n"
        f"      fix: {predicate.fix}"
    )


def _nfr008_letters() -> list[str]:
    """Return the seven NFR-008 letters the guard enforces, in order."""
    return [letter for letter, predicate in PREDICATES.items() if predicate.nfr008]


def report_lines(
    violations: list[Violation], problems: list[ExemptionProblem]
) -> list[str]:
    """Return the human report for one run, as lines.

    Args:
        violations: Predicate hits no exemption covers.
        problems: Dead or widened exemption entries.

    Returns:
        The report's lines. Empty when the run is clean: the success line is
        built separately by :func:`success_line`, so "no report" and "clean" stay
        the same thing.
    """
    lines: list[str] = []

    if violations:
        letters = sorted({v.predicate for v in violations})
        lines.append(
            f"ERROR: Found {len(violations)} NFR-008 predicate hit(s) across "
            f"predicate(s) {', '.join(f'({letter})' for letter in letters)} "
            "(spec `uniformtools`, scripts/check_tool_rename.py)."
        )
        lines.append(
            "Each hit is a way the FR-026 no-alias rule can erode: a retired "
            "spelling reaching an alias table, a resolver arm, a default, a "
            "registry key or a source literal, or a second registration or a "
            "wholesale forwarder standing in for one."
        )
        lines.append(
            "The only expected per-line exemptions are permission-type "
            "vocabulary (FR-028) and shell/language/stage-id/command names -- an "
            "entry for a tool-name site is a defect."
        )
        lines.append("")
        for violation in violations:
            lines.extend(_describe_violation(violation).split("\n"))
        lines.append("")

    if problems:
        lines.append(
            f"ERROR: {len(problems)} exemption problem(s) in "
            "scripts/check_tool_rename.py (FR-027)."
        )
        lines.append(
            "An exemption that matches no line, matches fewer lines than it "
            "declares, or declares the wrong literals is unreviewable: it tells "
            "the next reader a file is safe for a reason that no longer holds. "
            "Delete it or correct it."
        )
        lines.append("")
        for problem in problems:
            lines.append(f"  (e) {problem.path}: {problem.problem}")
        lines.append("")

    return lines


def success_line() -> str:
    """Return the one-line ASCII summary a clean run prints."""
    letters = ", ".join(f"({letter})" for letter in _nfr008_letters())
    return (
        "OK: no NFR-008 predicate holds in non-test source: no retired spelling "
        "in an alias table, a resolver arm, a config default, a registry key or "
        "a code literal, no second registration of one name, and no wholesale "
        f"forwarder (predicates {letters}); "
        f"{len(EXEMPTIONS)} reviewed exemption(s) all match."
    )


def report(root: pathlib.Path = ROOT) -> dict[str, object]:
    """Run the check on ``root`` and return the machine-readable report.

    The cargo test case is the caller: it compares this against the script's
    human output, so the two entry points are asserted to be one implementation
    rather than two that agree by review (NFR-009).

    Args:
        root: Repository root to scan.

    Returns:
        A mapping with ``ok`` (the exit verdict), ``error`` (a table-read failure
        message, else ``None``), ``root``, ``hits`` (one entry per violation),
        ``problems`` (one entry per exemption problem) and ``success`` (the line a
        clean run prints, so the test can compare the wording too).
    """
    result: dict[str, object] = {
        "ok": False,
        "error": None,
        "root": str(root),
        "hits": [],
        "problems": [],
        "success": success_line(),
    }
    try:
        violations, problems = check(root)
    except TableError as exc:
        result["error"] = str(exc)
        return result

    result["hits"] = [
        {
            "predicate": v.predicate,
            "path": v.path,
            "line": v.line,
            "literals": list(v.literals),
            "after": list(v.after),
            "text": v.text,
            "detail": v.detail,
            "fix": PREDICATES[v.predicate].fix,
            "nfr008": PREDICATES[v.predicate].nfr008,
            "report": _describe_violation(v),
        }
        for v in violations
    ]
    result["problems"] = [
        {"path": p.path, "problem": p.problem} for p in problems
    ]
    result["ok"] = not violations and not problems
    return result


def _run_self_test(root: pathlib.Path) -> int:
    """Seed the stale-literal fixture under ``root`` and assert it is reported.

    The fixture is written, scanned and removed in one call rather than from the
    shell wrapper, so the file cannot be left behind by an interrupted run and the
    one implementation owns all of it (NFR-009).

    Args:
        root: Repository root to seed beneath.

    Returns:
        Process exit code: 0 when the seeded violation was detected and the
        fixture was removed, 1 when the guard passed it or the seed failed.
    """
    seed = root / SELF_TEST_FIXTURE
    try:
        seed.write_text(SELF_TEST_SOURCE, encoding="utf-8")
    except OSError as exc:
        print(f"ERROR: self-test cannot seed {SELF_TEST_FIXTURE}: {exc}")
        return 1
    try:
        violations, problems = check(root)
    except TableError as exc:
        print(f"ERROR: self-test cannot enforce NFR-008: {exc}")
        return 1
    finally:
        try:
            seed.unlink()
        except OSError as exc:
            print(f"ERROR: self-test cannot remove {SELF_TEST_FIXTURE}: {exc}")

    seeded = [
        v for v in violations if v.path == SELF_TEST_FIXTURE and v.predicate == "e"
    ]
    if not seeded:
        print(
            "ERROR: self-test failed - the guard passed a seeded stale literal "
            f'({SELF_TEST_FIXTURE}: if tool == "bash").'
        )
        return 1
    if problems:
        print(
            f"ERROR: self-test found {len(problems)} exemption problem(s) while "
            "the fixture was seeded:"
        )
        for problem in problems:
            print(f"  {problem.path}: {problem.problem}")
        return 1
    print(
        "OK: self-test passed - seeded violation detected as (e) "
        f"{SELF_TEST_FIXTURE}:{seeded[0].line}."
    )
    return 0


def _parse_args(argv: list[str]) -> tuple[str, pathlib.Path]:
    """Return the selected mode and the root to scan.

    Args:
        argv: The arguments after the program name.

    Returns:
        ``(mode, root)`` where ``mode`` is ``check``, ``json``, ``exemptions`` or
        ``self-test``.

    Raises:
        SystemExit: An unknown argument was given.
    """
    mode = "check"
    root = ROOT
    remaining = list(argv)
    while remaining:
        argument = remaining.pop(0)
        if argument == "--exemptions":
            mode = "exemptions"
        elif argument == "--self-test":
            mode = "self-test"
        elif argument == "--json":
            mode = "json"
        elif argument == "--check":
            mode = "check"
        elif argument == "--root":
            if not remaining:
                print("ERROR: --root needs a directory argument")
                sys.exit(2)
            root = pathlib.Path(remaining.pop(0)).resolve()
        else:
            print(f"ERROR: unknown argument {argument!r}")
            print("usage: check_tool_rename.py [--check|--json|--self-test|"
                  "--exemptions] [--root DIR]")
            sys.exit(2)
    return mode, root


def main() -> None:
    """Run the guard, printing a report and exiting 0 (clean) or 1 (failed)."""
    mode, root = _parse_args(sys.argv[1:])

    if mode == "self-test":
        sys.exit(_run_self_test(root))

    if mode == "json":
        payload = report(root)
        print(json.dumps(payload, indent=2))
        sys.exit(0 if payload["ok"] else 1)

    if mode == "exemptions":
        print("# Structural exemptions (not scanned for literals):")
        print(f"  {TABLE_FILE}")
        print("    the FR-023 table itself -- its `before:` fields necessarily")
        print("    carry every retired spelling, and this guard reads it as the")
        print("    set it enforces.")
        print(f"  {STRUCTURAL_CHANGELOG} (outside the scanned tree)")
        print("    the Renamed table and its narrative document all thirteen")
        print("    spellings; the migration note has to name them.")
        print("")
        print("# Scope, not an exemption:")
        print("  comment prose (///, //!, //) is not scanned -- a comment cannot")
        print("  key on a name. A trailing // comment on a code line is stripped")
        print("  first, so the code part is still judged.")
        print("")
        print(
            f"# Reviewed self-mapping entries (predicate (a2)): "
            f"{len(SELF_MAPPING_ALLOW)}"
        )
        for (path, text), reason in SELF_MAPPING_ALLOW.items():
            print(f"  {path}")
            print(f"    {text!r}")
            print(f"      allowance-reason: {reason}")
        print("")
        print(f"# Resolver files (predicates (a), (a2) and (b)): {len(RESOLVER_FILES)}")
        for path, why in RESOLVER_FILES.items():
            print(f"  {path}")
            print(f"    {why}")
        print("")
        print(
            f"# Per-line exemptions ({len(EXEMPTIONS)} entries, each with the "
            "reason it is not a tool name):"
        )
        current = None
        for entry in EXEMPTIONS:
            if entry.path != current:
                current = entry.path
                print(f"  {current}")
            times = "" if entry.count == 1 else f" x{entry.count}"
            print(f"    {entry.text!r}{times}")
            print(f"      literals:   {list(entry.literals)}")
            print(f"      predicates: {list(entry.predicates)}")
            print(f"      reason:     {entry.reason}")
        print("")
        print(
            f"# NFR-008 predicates implemented ({len(_nfr008_letters())} of the "
            f"seven, plus the extra (a2) rule):"
        )
        for letter, predicate in PREDICATES.items():
            print(f"  ({letter}) {predicate.title}")
        return

    try:
        violations, problems = check(root)
    except TableError as exc:
        print(f"ERROR: cannot enforce NFR-008: {exc}")
        sys.exit(1)

    for line in report_lines(violations, problems):
        print(line)
    if violations or problems:
        sys.exit(1)

    print(success_line())
    sys.exit(0)


if __name__ == "__main__":
    main()
