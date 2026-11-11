#!/usr/bin/env python3
"""Offline check of relative links and anchors in tracked Markdown files.

For every git-tracked `*.md` file (except generated research output), each
inline link `[text](target)` that is not an external URL must point at a file
or directory that exists, and a `#fragment` on a Markdown target must match a
heading anchor in that file, using GitHub's slug rules:

- lower-case; drop every character that is not a letter, digit, space, `-`
  or `_` (so `_` is kept and `--flag` keeps both hyphens);
- spaces become `-`;
- a repeated heading gets `-1`, `-2`, ... suffixes;
- explicit `<a name="x">` / `id="x"` anchors also count.

Links inside fenced code blocks and inline code spans are ignored.

Usage:
    scripts/check-doc-links.py              # check the repository
    scripts/check-doc-links.py --self-test  # prove the checker catches breaks

Exit status is 1 when any link is broken, 0 otherwise.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from urllib.parse import unquote
from functools import lru_cache

EXCLUDED_PREFIXES = ("research/",)
LINK_RE = re.compile(r"(?<!\!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
IMAGE_RE = re.compile(r"!\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
HEADING_RE = re.compile(r"^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
# Any indentation: fences nested in list items are indented 4+ spaces.
FENCE_RE = re.compile(r"^\s*(`{3,}|~{3,})")
HTML_ANCHOR_RE = re.compile(r"""<a\s+(?:name|id)=["']([^"']+)["']|\sid=["']([^"']+)["']""")
INLINE_CODE_RE = re.compile(r"(`+)(?:(?!\1).)+?\1")


def slugify(heading: str) -> str:
    """Return GitHub's anchor slug for a heading's text."""
    # Inline HTML is dropped, but `<name>` inside a code span is literal text.
    parts = re.split(r"(`+[^`]*`+)", heading)
    text = "".join(p if p.startswith("`") else re.sub(r"<[^>]+>", "", p) for p in parts)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)  # links keep their text
    text = text.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def strip_code(lines: list[str], inline: bool = True) -> list[str]:
    """Blank out fenced code blocks (and, if `inline`, inline code spans),
    keeping the line count."""
    out = []
    fence = None
    for line in lines:
        m = FENCE_RE.match(line)
        if fence:
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence):
                fence = None
            out.append("")
            continue
        if m:
            fence = m.group(1)
            out.append("")
            continue
        out.append(INLINE_CODE_RE.sub("", line) if inline else line)
    return out


@lru_cache(maxsize=None)
def anchors_of(path: str) -> frozenset[str]:
    """Return every anchor a Markdown file defines."""
    with open(path, encoding="utf-8") as fh:
        # Headings keep inline code text: GitHub slugs `--flag` as --flag.
        lines = strip_code(fh.read().splitlines(), inline=False)
    seen: dict[str, int] = {}
    found = set()
    for line in lines:
        m = HEADING_RE.match(line)
        if m:
            # Like GitHub: take the next unused id, so `# Repeat`, `# Repeat-1`,
            # `# Repeat` yield repeat, repeat-1, repeat-2.
            base = slugify(m.group(2))
            n = seen.get(base, 0)
            candidate = base if n == 0 else f"{base}-{n}"
            while candidate in found:
                n += 1
                candidate = f"{base}-{n}"
            found.add(candidate)
            seen[base] = n + 1
        # Explicit anchors count only outside inline code.
        for a, b in HTML_ANCHOR_RE.findall(INLINE_CODE_RE.sub("", line)):
            found.add(a or b)
    return frozenset(found)


def check_file(path: str) -> list[str]:
    """Return one message per broken link in `path`."""
    with open(path, encoding="utf-8") as fh:
        lines = strip_code(fh.read().splitlines())
    problems = []
    for lineno, line in enumerate(lines, 1):
        for regex in (LINK_RE, IMAGE_RE):
            for target in regex.findall(line):
                if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target):
                    continue  # http:, https:, mailto:, ...
                target_path, _, fragment = target.partition("#")
                target_path, fragment = unquote(target_path), unquote(fragment)
                if target_path.startswith("/"):
                    resolved = target_path.lstrip("/")
                elif target_path:
                    resolved = os.path.normpath(
                        os.path.join(os.path.dirname(path), target_path)
                    )
                else:
                    resolved = path
                if not os.path.exists(resolved):
                    problems.append(f"{path}:{lineno}: missing target {target}")
                    continue
                if fragment and resolved.endswith(".md") and os.path.isfile(resolved):
                    if fragment.lower() not in anchors_of(resolved):
                        problems.append(f"{path}:{lineno}: no anchor #{fragment} in {resolved}")
    return problems


def tracked_markdown() -> list[str]:
    """Return git-tracked Markdown files, minus excluded generated trees."""
    out = subprocess.run(
        ["git", "ls-files", "-z", "*.md"], capture_output=True, text=True, check=True
    ).stdout.split("\0")
    return [f for f in out if f and not f.startswith(EXCLUDED_PREFIXES) and os.path.exists(f)]


def run(files: list[str]) -> int:
    problems = [p for f in files for p in check_file(f)]
    for p in problems:
        print(p)
    if problems:
        print(f"\n{len(problems)} broken link(s). Fix the link or the heading it targets.")
        return 1
    print(f"doc links ok ({len(files)} files)")
    return 0


def self_test() -> int:
    """Seed one good and several bad links; the checker must flag only the bad."""
    scratch = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "temp")
    os.makedirs(scratch, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch) as tmp:
        target = os.path.join(tmp, "target.md")
        with open(target, "w", encoding="utf-8") as fh:
            fh.write(
                "# Top\n\n## `--flag` option\n\n## Repeat\n\n## Repeat\n\n## snake_case\n\n"
                "## 4.1 `/cmd <name>` - deck\n\n## Dup\n\n## Dup-1\n\n## Dup\n\n## Café\n\n"
                "`<a id=\"not-real\">`\n"
            )
        with open(os.path.join(tmp, "my guide.md"), "w", encoding="utf-8") as fh:
            fh.write("# Guide\n")
        good = os.path.join(tmp, "good.md")
        with open(good, "w", encoding="utf-8") as fh:
            fh.write(
                "[a](target.md#top) [b](target.md#--flag-option) [c](target.md#repeat-1)\n"
                "[d](target.md#snake_case) [e](https://example.com/x)\n"
                "[f](target.md#41-cmd-name---deck) [g](target.md#dup-2) [h](my%20guide.md)\n"
                "[i](target.md#caf%C3%A9)\n"
                "- item\n\n    ```\n    [nested fence](missing.md)\n    ```\n"
                "```\n[ignored](missing.md)\n```\n`[ignored](missing.md)`\n"
            )
        bad = os.path.join(tmp, "bad.md")
        with open(bad, "w", encoding="utf-8") as fh:
            fh.write("[x](missing.md) [y](target.md#nope) [z](target.md#repeat-2) [w](target.md#not-real)\n")
        good_problems = check_file(good)
        bad_problems = check_file(bad)
    if good_problems or len(bad_problems) != 4:
        print("self-test FAILED")
        print("good:", good_problems)
        print("bad:", bad_problems)
        return 1
    print("self-test ok")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv[1:]:
        sys.exit(self_test())
    sys.exit(run(tracked_markdown()))
