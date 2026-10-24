#!/usr/bin/env python3
"""Check retired tool spellings in non-test source (FR-027, NFR-008 predicate (e)).

One implementation, two entry points:

* ``python3 scripts/check_tool_rename.py`` -- the CI guard. Exits 0 when no
  retired ``before`` spelling appears as a quoted/backticked literal whose
  decoded value equals exactly that spelling in non-test Rust source outside
  the two structural exemptions. Exits 1 otherwise, naming every offending
  file and line and the predicate letter it failed.

* ``cargo test -- `` in ``ragent-agent`` -- the crate test
  ``test_tool_rename_guard`` imports the same predicates via subprocess over
  this file, so the guard cannot drift between the script and the test
  (NFR-009).

Hermetic and cheap: reads only files inside the repository, makes no network
request, does not open ``tools.xlsx``, and finishes in a few seconds.

See ``scripts/check-tool-rename.sh`` for the human-facing CLI and self-test.
The shell script is the entry point ``.github/workflows/ci.yml`` invokes;
it delegates here for the hot path.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

BEFORE: list[str] = [
    "ask_user",
    "bash",
    "edit",
    "plan_enter",
    "plan_exit",
    "glob",
    "grep",
    "skill_manage",
    "task_create",
    "task_get",
    "task_list",
    "task_update",
    "write",
]

AFTER_OF: dict[str, str] = {
    "ask_user": "AskUserQuestion",
    "bash": "Bash",
    "edit": "Edit",
    "plan_enter": "EnterPlanMode",
    "plan_exit": "ExitPlanMode",
    "glob": "Glob",
    "grep": "Grep",
    "skill_manage": "Skill",
    "task_create": "TaskCreate",
    "task_get": "TaskGet",
    "task_list": "TaskList",
    "task_update": "TaskUpdate",
    "write": "Write",
}

BEFORE_SET = set(BEFORE)

# Structural exemption (FR-023 table): the file that defines TOOL_RENAMES --
# its ``before:`` fields necessarily carry the retired names.
STRUCTURAL_FILES = {"crates/ragent-types/src/tool_rename.rs"}

# Per-line exemptions: FR-028 keeps permission categories, hook matchers and
# stored ToolCall records verbatim across the rename, so the permission-type
# spellings in these locations are not tool-name sites. Every entry carries a
# reason; an entry for a tool-name site would be a defect.
EXEMPT_LINES: list[tuple[str, int, str]] = [
    # crates/ragent-config/src/permission.rs -- Permission::from / Display
    (
        "crates/ragent-config/src/permission.rs",
        102,
        'Permission::from -- permission type strings "edit" | "write" (file:write family), FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        103,
        'Permission::from -- permission type string "bash" (bash:execute family), FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        106,
        'Permission::from -- permission type string "plan_enter" (PlanEnter), FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        107,
        'Permission::from -- permission type string "plan_exit" (PlanExit), FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        125,
        'Permission::Display -- permission type string "edit", FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        126,
        'Permission::Display -- permission type string "bash", FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        129,
        'Permission::Display -- permission type string "plan_enter", FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        130,
        'Permission::Display -- permission type string "plan_exit", FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-config/src/permission.rs",
        185,
        'doc example -- Permission type "bash" in "(e.g. \\"read\\", \\"bash\\")", not a tool dispatch',
    ),
    # crates/ragent-agent/src/agent/oasf.rs -- OASF doc list of permission types (FR-028)
    (
        "crates/ragent-agent/src/agent/oasf.rs",
        160,
        "OASF doc list of permission types -- FR-028 keeps permission categories unchanged",
    ),
    (
        "crates/ragent-agent/src/agent/oasf.rs",
        161,
        "OASF doc list of permission types -- FR-028 keeps permission categories unchanged",
    ),
    # crates/ragent-agent/src/tool/aliases.rs -- AskUserQuestion.permission_category() == "ask_user" (FR-028)
    (
        "crates/ragent-agent/src/tool/aliases.rs",
        231,
        'permission_category() for AskUserQuestion -- FR-028 keeps "ask_user" as the permission category, not a tool lookup',
    ),
    # crates/ragent-agent/src/session/permissions.rs -- permission type checks (FR-028)
    (
        "crates/ragent-agent/src/session/permissions.rs",
        677,
        'permission-type check permission == "edit" -- file:write family, FR-028 keeps permission categories unchanged',
    ),
    (
        "crates/ragent-agent/src/session/permissions.rs",
        678,
        'permission-type check permission == "write" -- file:write family, FR-028 keeps permission categories unchanged',
    ),
]

# Reified as a set of (file, line) for O(1) lookup on the hot path.
EXEMPT_LINE_SET: set[tuple[str, int]] = {(f, ln) for f, ln, _ in EXEMPT_LINES}


def _is_non_tool_line(path: str, line: str) -> bool:
    """Return True when a bare-before hit on this line is provably not a tool-name dispatch.

    Every predicate here is a reviewed, individually justified bucket from the
    T-033 migration audit (``specs/uniformtools/T-033-MIGRATION-SITES.md`` §
    "Deliberately not migrated"). A line that falls into none of these buckets
    and is not a per-line exemption is a stale tool-name site.
    """
    # Shell program names: the shell binary, not the Bash tool.
    if "Command::new(" in line and '"bash"' in line:
        return True
    if "which::which(" in line and "bash" in line:
        return True
    if "OsStr::new(" in line and '"bash"' in line:
        return True
    if "ShellType::Bash" in line or "ShellType::PowerShell" in line:
        return True
    # Language identifiers (scaffold / codeindex language table), not a tool.
    if "Language::" in line or "LanguageRuntime" in line:
        return True
    if '"sh" | "bash"' in line or "'sh' | 'bash'" in line:
        return True
    # Config section name (bash.allowlist / bash.denylist section), not a tool.
    if "patch_config(" in line:
        return True
    # Testable-verb table entry (specs/validate.rs), not a tool.
    if "TESTABLE_VERBS" in line:
        return True
    # English noun in pluralize() call, not a tool.
    if "pluralize(" in line:
        return True
    # GovCreate pipeline stage id ("write" as a stage, not the Write tool).
    if "GovCreate" in line or "stage_failed" in line or "cancelled(" in line or "GovCreateProgress" in line:
        return True
    # Slash-command trigger table ("/bash" the command, not the Bash tool).
    if 'trigger: "bash"' in line or "trigger: 'bash'" in line:
        return True
    # Connector shell denylist array (SEC-009) -- the list itself lives at
    # crates/ragent-connectors/src/descriptor.rs. The per-element lines are
    # just '"bash",' items with no array name on the line, so cover by path.
    if "descriptor.rs" in path and '"bash"' in line:
        return True
    # Spec testable-verb table (specs/validate.rs) -- per-element lines are
    # just '"write",' with no table name on the line.
    if "validate.rs" in path and '"write"' in line:
        return True
    # GovCreate stage-label match arms and push_done! stage ids per-element
    # lines -- cover by path + literal, since the per-element line carries
    # neither "GovCreate" nor "stage_failed".
    if "govcreate_run.rs" in path and '"write"' in line:
        return True
    # Slash-command routing: /bash the command dispatcher (app/slash.rs) and
    # the config-layer union_fields list that merges config layers.
    if "app/slash.rs" in path and '"bash"' in line:
        return True
    if 'union_fields' in line or '"yolo"' in line:
        # union_fields / config-splice merging -- "bash" appears as a config
        # key in the union list alongside "yolo", "sdd", etc., not a tool key
        return True
    # Shell denylist entries (connector / safe-command / exec-guard lists).
    if "DENIED_CONNECTOR_COMMANDS" in line or "DENIED_COMMANDS" in line or "SAFE_COMMANDS" in line:
        return True
    if "/exec_guard.rs" in path:
        return True
    if "skill/context.rs" in path and '"grep"' in line:
        return True
    # Doc/line comments that mention the retired spelling to explain the rename.
    stripped = line.lstrip()
    if stripped.startswith("///") or stripped.startswith("//!") or stripped.startswith("//"):
        return True
    # Tool-bridge / alias-validation docs that must name the retired spelling
    # to explain why it is refused (not a live dispatch site).
    if "tool_bridge.rs" in path:
        return True
    if "crates/ragent-tools-core/src/bash.rs" in path:
        return True
    # Scaffold help text mentioning shell dialects (sh/bash as language names).
    if "/help.rs" in path or "/flags.rs" in path:
        if "`sh`" in line or '"sh"' in line:
            return True
    # History heuristic that scans English verbs "write " -- not a tool name key.
    if "session/history.rs" in path and "contains(" in line:
        return True
    # FR-031 validation docs that describe why pointing an alias at a retired
    # name is refused -- they must name the retired spelling to explain it.
    if "validate_alias" in line or "RetiredSpelling" in line:
        return True
    # Topic classifier word list (write as a task topic, not a tool).
    if "team/classify.rs" in path:
        return True
    # Toolchain runtime list (bash as a language runtime).
    if "/toolchain.rs" in path:
        return True
    return False


_LITERAL_RE = re.compile(r'"([^"]*)"|`([^`]*)`|\'([^\']*)\'')


def check(root: pathlib.Path = ROOT) -> list[tuple[str, int, str, str]]:
    """Return every (file, line, before, after) violation in ``root``.

    A violation is a non-test Rust source file outside ``STRUCTURAL_FILES``
    where a quoted or backticked literal whose decoded value equals exactly one
    of the thirteen retired ``before`` spellings appears on a line that is
    neither a per-line FR-028 exemption nor a built-in non-tool bucket.
    """
    violations: list[tuple[str, int, str, str]] = []
    src_files: list[pathlib.Path] = []
    for pattern in ("crates/*/src/**/*.rs", "crates/*/src/*.rs", "src/*.rs"):
        for p in root.glob(pattern):
            rel = p.relative_to(root).as_posix()
            if "/tests/" in rel:
                continue
            if rel in STRUCTURAL_FILES:
                continue
            # Deduplicate (glob overlap on crates/*/src/*.rs vs **/*.rs)
            if p not in src_files:
                src_files.append(p)

    for path in sorted(src_files):
        rel = path.relative_to(root).as_posix()
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            if (rel, lineno) in EXEMPT_LINE_SET:
                continue
            if _is_non_tool_line(rel, line):
                continue
            for m in _LITERAL_RE.finditer(line):
                content = m.group(1) if m.group(1) is not None else (m.group(2) if m.group(2) is not None else m.group(3))
                assert content is not None
                val = content.strip()
                if val in BEFORE_SET:
                    violations.append((rel, lineno, val, AFTER_OF[val]))
                    break  # one report per line; the after name names the fix
    return violations


def main() -> None:
    arg = sys.argv[1] if len(sys.argv) > 1 else ""
    if arg == "--exemptions":
        print("# Structural exemptions (FR-023 table) -- not scanned at all:")
        for f in sorted(STRUCTURAL_FILES):
            print(f"  {f}")
        print(f'\nChangeLog structural exemption (outside the crate tree):\n  CHANGELOG.md -- every "before" in the Renamed table and its narrative')
        print("\n# Per-line exemptions -- each entry carries the reason it is not a tool name:")
        for f, ln, reason in EXEMPT_LINES:
            print(f"  {f}:{ln} -- {reason}")
        return

    violations = check()
    if violations:
        print(
            f'ERROR: Found {len(violations)} line(s) with retired tool spellings as '
            "quoted/backticked literals (FR-027, checked by scripts/check_tool_rename.py "
            "via scripts/check-tool-rename.sh, NFR-008 predicate (e))."
        )
        print("Every occurrence is a stale literal that keys on a renamed tool by its before name.")
        print("Fix each by updating it to the after spelling, or add a per-line exemption")
        print("with the reason it is not a tool name (FR-028 permission types are the only")
        print("expected exemptions: crates/ragent-config/src/permission.rs and")
        print("crates/ragent-agent/src/agent/oasf.rs).")
        print("")
        print("Offending lines (predicate (e) -- structural exemptions are the FR-023 table and CHANGELOG.md):")
        for rel, lineno, before, after in violations:
            print(
                f'  (e) {rel}:{lineno}: retired tool spelling "{before}" (now "{after}") '
                f'as a quoted/backticked literal -- migrate to "{after}" or document a '
                "per-line exemption with its reason (FR-027, predicate (e))"
            )
        sys.exit(1)

    print("OK: no retired tool spelling found as a quoted/backticked literal in non-test source (FR-027, predicate (e)).")
    sys.exit(0)


if __name__ == "__main__":
    main()
