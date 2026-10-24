#!/usr/bin/env python3
"""Unit tests for tools/analyze_function_lengths.py.

Run with:

    python3 -m unittest tools/test_analyze_function_lengths.py -v

The tests build a small synthetic tree under ``target/temp`` (never touching the
repository's own sources), so a run leaves the working tree byte-identical.
"""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT = REPO_ROOT / "tools" / "analyze_function_lengths.py"


def _load_module():
    """Import the analyzer script by path (it is not on sys.path)."""
    spec = importlib.util.spec_from_file_location("analyze_function_lengths", SCRIPT)
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


AFL = _load_module()


def _padded_body(lines: int) -> str:
    """Return ``lines`` filler statements at the given body indentation."""
    return "".join(f"    x{i} = {i}\n" for i in range(lines))


class TempTreeCase(unittest.TestCase):
    """Base case giving each test a fresh scratch directory."""

    def setUp(self) -> None:
        scratch = REPO_ROOT / "target" / "temp"
        scratch.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(dir=str(scratch), prefix="funclen-"))

    def tearDown(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)

    def write(self, relative: str, content: str) -> Path:
        """Create ``relative`` under the scratch root and return its path."""
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path


class PythonAnalysisTests(unittest.TestCase):
    def test_reports_nested_functions_and_decorator_start(self) -> None:
        source = (
            "import functools\n"
            "\n"
            "\n"
            "@functools.cache\n"
            "def outer():\n"
            "    def inner():\n"
            "        return 1\n"
            "    return inner\n"
        )
        spans = {s.name: s for s in AFL.analyse_text(source, "python")}
        self.assertEqual(sorted(spans), ["inner", "outer"])
        # Decorator line 4 is the reported start of outer.
        self.assertEqual(spans["outer"].start_line, 4)
        self.assertEqual(spans["outer"].end_line, 8)
        self.assertEqual(spans["inner"].start_line, 6)
        self.assertEqual(spans["inner"].end_line, 7)
        self.assertEqual(spans["inner"].line_count, 2)

    def test_async_method_is_reported(self) -> None:
        source = "class C:\n    async def m(self):\n        return None\n"
        spans = AFL.analyse_text(source, "python")
        self.assertEqual([(s.name, s.start_line, s.end_line) for s in spans],
                         [("m", 2, 3)])

    def test_syntax_error_raises(self) -> None:
        with self.assertRaises(SyntaxError):
            AFL.analyse_text("def broken(:\n", "python")


class BraceLanguageTests(unittest.TestCase):
    def test_rust_fn_ignores_braces_in_strings_and_comments(self) -> None:
        source = (
            "fn small() {\n"          # 1
            "    let s = \"}{ not code {\";\n"  # 2
            "    // } comment brace {\n"        # 3
            "    /* } block { */\n"             # 4
            "}\n"                     # 5
            "\n"
            "fn big() {\n"            # 7
            "    if x {\n"            # 8
            "        y();\n"          # 9
            "    }\n"                 # 10
            "}\n"                     # 11
        )
        spans = {s.name: s for s in AFL.analyse_text(source, "rust")}
        self.assertEqual(sorted(spans), ["big", "small"])
        self.assertEqual((spans["small"].start_line, spans["small"].end_line), (1, 5))
        self.assertEqual((spans["big"].start_line, spans["big"].end_line), (7, 11))

    def test_rust_trait_declaration_without_body_is_not_a_function(self) -> None:
        source = "trait T {\n    fn no_body(&self) -> u8;\n}\n"
        self.assertEqual(AFL.analyse_text(source, "rust"), [])

    def test_javascript_function_arrow_and_method(self) -> None:
        source = (
            "export async function declared() {\n"   # 1
            "  return 1;\n"                            # 2
            "}\n"                                      # 3
            "const arrow = async (a) => {\n"           # 4
            "  return a;\n"                            # 5
            "};\n"                                     # 6
            "class K {\n"                              # 7
            "  method(a) {\n"                          # 8
            "    return a;\n"                          # 9
            "  }\n"                                    # 10
            "}\n"                                      # 11
        )
        names = {s.name: (s.start_line, s.end_line)
                 for s in AFL.analyse_text(source, "javascript")}
        self.assertEqual(names, {"declared": (1, 3), "arrow": (4, 6),
                                "method": (8, 10)})

    def test_javascript_control_keywords_are_not_functions(self) -> None:
        source = (
            "function real() {\n"
            "  if (a) {\n"
            "  }\n"
            "  for (;;) {\n"
            "  }\n"
            "}\n"
        )
        self.assertEqual([s.name for s in AFL.analyse_text(source, "javascript")],
                         ["real"])

    def test_go_method_with_receiver(self) -> None:
        source = "func (s *Svc) Run(x int) {\n    return\n}\n"
        spans = AFL.analyse_text(source, "go")
        self.assertEqual([(s.name, s.start_line, s.end_line) for s in spans],
                         [("Run", 1, 3)])

    def test_go_func_literal_is_not_reported(self) -> None:
        source = "var f = func() {\n}\n"
        self.assertEqual(AFL.analyse_text(source, "go"), [])

    def test_java_method(self) -> None:
        source = (
            "public class C {\n"
            "    public static void main(String[] args) {\n"
            "        run();\n"
            "    }\n"
            "}\n"
        )
        spans = {s.name: s for s in AFL.analyse_text(source, "java")}
        self.assertIn("main", spans)
        self.assertEqual((spans["main"].start_line, spans["main"].end_line), (2, 4))

    def test_c_function_and_struct_skipped(self) -> None:
        source = (
            "struct point { int x; };\n"
            "static int add(int a, int b)\n"
            "{\n"
            "    return a + b;\n"
            "}\n"
        )
        names = [s.name for s in AFL.analyse_text(source, "c")]
        self.assertEqual(names, ["add"])

    def test_php_method(self) -> None:
        source = (
            "<?php\n"
            "class C {\n"
            "    public function run($a) {\n"
            "        return $a;\n"
            "    }\n"
            "}\n"
        )
        spans = AFL.analyse_text(source, "php")
        self.assertEqual([(s.name, s.start_line, s.end_line) for s in spans],
                         [("run", 3, 5)])

    def test_nested_closure_is_reported_once(self) -> None:
        source = (
            "function outer() {\n"
            "  const inner = () => {\n"
            "    return 1;\n"
            "  };\n"
            "}\n"
        )
        self.assertEqual([s.name for s in AFL.analyse_text(source, "javascript")],
                         ["outer"])


class RubyAnalysisTests(unittest.TestCase):
    def test_def_end_with_nested_blocks(self) -> None:
        source = (
            "def small\n"          # 1
            "  if a\n"             # 2
            "    1\n"              # 3
            "  end\n"              # 4
            "end\n"                # 5
            "\n"
            "def big(x)\n"         # 7
            "  [1].each do |i|\n"  # 8
            "    puts i # end\n"   # 9
            "  end\n"              # 10
            "end\n"                # 11
        )
        spans = {s.name: s for s in AFL.analyse_text(source, "ruby")}
        self.assertEqual({name: (s.start_line, s.end_line)
                          for name, s in spans.items()},
                         {"small": (1, 5), "big": (7, 11)})

    def test_unterminated_def_is_not_reported(self) -> None:
        source = "def broken\n  x = 1\n"
        self.assertEqual(AFL.analyse_text(source, "ruby"), [])


class ScanTests(TempTreeCase):
    def test_threshold_filters_and_summary_counts(self) -> None:
        body = _padded_body(12)
        self.write("pkg/short.py", f"def s():\n    return 1\n")
        self.write("pkg/long.py", f"def big():\n{body}")
        self.write("pkg/lib.rs", "fn tiny() {\n}\n")
        report = AFL.scan(self.root, threshold=5)
        summary = report["summary"]
        self.assertEqual(summary["files_scanned"], 3)
        self.assertEqual(summary["files_skipped"], 0)
        self.assertEqual(summary["functions_analysed"], 3)
        self.assertEqual(summary["functions_over_threshold"], 1)
        offender = report["offenders"][0]
        self.assertEqual(offender["file"], str(Path("pkg") / "long.py"))
        self.assertEqual(offender["function"], "big")
        self.assertEqual(offender["language"], "python")
        self.assertEqual((offender["start_line"], offender["end_line"]), (1, 13))
        self.assertEqual(offender["line_count"], 13)
        self.assertEqual(summary["by_language"]["python"],
                         {"files": 2, "functions": 2, "over_threshold": 1})
        self.assertEqual(summary["by_language"]["rust"],
                         {"files": 1, "functions": 1, "over_threshold": 0})

    def test_offenders_sorted_by_line_count_descending(self) -> None:
        self.write("a.py", f"def a():\n{_padded_body(4)}")
        self.write("b.py", f"def b():\n{_padded_body(9)}")
        report = AFL.scan(self.root, threshold=1)
        self.assertEqual([o["function"] for o in report["offenders"]], ["b", "a"])

    def test_skip_dirs_are_never_walked(self) -> None:
        for name in ("node_modules", "vendor", "dist", "build", ".venv",
                     "__pycache__", "target"):
            self.write(f"{name}/junk.py", "def f():\n    return 1\n")
        self.write(".git/config.py", "def f():\n    return 1\n")
        self.write("src/keep.py", "def f():\n    return 1\n")
        report = AFL.scan(self.root, threshold=1000)
        self.assertEqual(report["summary"]["files_scanned"], 1)
        self.assertEqual(report["summary"]["by_language"]["python"]["files"], 1)

    def test_unknown_extensions_are_ignored(self) -> None:
        self.write("notes.md", "def f():\n")
        self.write("data.json", "{}\n")
        report = AFL.scan(self.root, threshold=1000)
        self.assertEqual(report["summary"]["files_scanned"], 0)
        self.assertEqual(report["summary"]["by_language"], {})

    def test_unparsable_python_is_skipped_with_reason(self) -> None:
        self.write("broken.py", "def broken(:\n")
        self.write("good.py", "def ok():\n    return 1\n")
        report = AFL.scan(self.root, threshold=1000)
        self.assertEqual(report["summary"]["files_scanned"], 1)
        self.assertEqual(report["summary"]["files_skipped"], 1)
        self.assertEqual(report["skipped_files"][0]["file"], "broken.py")
        self.assertIn("SyntaxError", report["skipped_files"][0]["reason"])

    @unittest.skipIf(os.geteuid() == 0, "root can read any file")
    def test_unreadable_file_is_skipped_with_reason(self) -> None:
        path = self.write("locked.py", "def f():\n    return 1\n")
        path.chmod(0o000)
        try:
            report = AFL.scan(self.root, threshold=1000)
        finally:
            path.chmod(0o600)
        self.assertEqual(report["summary"]["files_scanned"], 0)
        self.assertEqual(report["summary"]["files_skipped"], 1)
        self.assertIn("PermissionError", report["skipped_files"][0]["reason"])

    def test_undecodable_file_is_skipped_with_reason(self) -> None:
        path = self.root / "binary.py"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"\xff\xfe def f():\n")
        report = AFL.scan(self.root, threshold=1000)
        self.assertEqual(report["summary"]["files_skipped"], 1)
        self.assertIn("UnicodeDecodeError", report["skipped_files"][0]["reason"])


class CliTests(TempTreeCase):
    def test_main_writes_json_report_to_output_file(self) -> None:
        self.write("src/big.rs", "fn huge() {\n" + "\n" * 9 + "}\n")
        output = self.root / "out" / "report.json"
        code = AFL.main(["--root", str(self.root), "--threshold", "5",
                         "--output", str(output)])
        self.assertEqual(code, 0)
        report = json.loads(output.read_text(encoding="utf-8"))
        self.assertEqual(report["summary"]["threshold"], 5)
        self.assertEqual(report["summary"]["functions_over_threshold"], 1)
        self.assertEqual(report["offenders"][0]["function"], "huge")
        self.assertEqual(report["offenders"][0]["line_count"], 11)

    def test_main_prints_json_to_stdout(self) -> None:
        self.write("a.py", "def f():\n    return 1\n")
        stdout = sys.stdout
        from io import StringIO

        buffer = StringIO()
        sys.stdout = buffer
        try:
            code = AFL.main(["--root", str(self.root)])
        finally:
            sys.stdout = stdout
        self.assertEqual(code, 0)
        report = json.loads(buffer.getvalue())
        self.assertEqual(report["summary"]["threshold"], AFL.DEFAULT_THRESHOLD)
        self.assertEqual(report["offenders"], [])
        self.assertIn("summary", report)

    def test_main_rejects_bad_root(self) -> None:
        missing = self.root / "nope"
        self.assertEqual(AFL.main(["--root", str(missing)]), 2)

    def test_main_rejects_non_positive_threshold(self) -> None:
        with self.assertRaises(SystemExit):
            AFL.main(["--root", str(self.root), "--threshold", "0"])


if __name__ == "__main__":
    unittest.main()
