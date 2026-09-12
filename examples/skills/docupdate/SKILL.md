---
description: "Update all the Major DOC file"
user-invocable: true
disable-model-invocation: true
arguments: true
---
Perform an update of key documentation files.

The skill accepts one optional argument `$ARGUMENTS`:

- If omitted, treat it as `10`.
- If it is a positive integer `N`, use the last `N` commits.
- If it is the literal word `uncommitted` or `uncommited` (both spellings accepted, case-insensitive), document the current uncommitted working-tree changes (staged and unstaged) **in addition to** the last 10 commits.

Steps:

1. **Read the current version** from the workspace `Cargo.toml` (the `version = "..."` line near the top).
2. **Run `cargo check`** to ensure the change doesn't break the build.
3. **Run `cargo audit`** to ensure that there are no new security issues introduced; stop if there are security issues.
4. Gather the source changes to document:

   - If the argument is `uncommitted` or `uncommited`, describe BOTH sources:
     - Uncommitted changes: `git status --short`, `git diff --stat`, and `git diff --cached --stat` (if any).
     - The last 10 commits: `git log --oneline -n 10`.
   - Otherwise, use the last `N` commits (`git log --oneline -n $ARGUMENTS`) to describe recent changes.
5. Update the following files with the gathered changes:

   - `CHANGELOG.md` — add a new entry for each commit and, when the `uncommitted`/`uncommited` argument is used, an additional "uncommitted" entry summarising the worktree changes, including the commit hash or "uncommitted" marker and a short description.
   - `README.md` — update any relevant sections to reflect recent changes.
   - `STATS.md` — Update the project stats.
   - `SPEC.md` - Update the master spec file.
   - `QUICKSTART.md` - Update the quickstart file.
   - `TUI-QUICKSTART.md` - Update the tui info file.

   ###### Update the following files in the docs/howtos folder:


   - `docs/howtos` directory — update any relevant documentation files with new information or corrections.
   - convert each howto .md file to pdf using the command sequence `pandoc ./[howtofile] -o ./pdf/[howtofile].pdf --pdf-engine=xelatex -V mainfont="Liberation Serif" -V geometry:a4paper`.The command should be run in the`docs/howtos` folder. The `-V geometry:a4paper` flag ensures the generated PDFs use the A4 page size.
   - convert each slashcommand howto .md file to pdf using the command sequence `pandoc ./slashcommands/[howtofile] -o ./slashcommands/pdf/[howtofile].pdf --pdf-engine=xelatex -V mainfont="Liberation Serif" -V geometry:a4paper`.The command should be run in the`docs/howtos` folder. The `-V geometry:a4paper` flag ensures the generated PDFs use the A4 page size.
6. **Stage all modified files** with `git add -A`.
