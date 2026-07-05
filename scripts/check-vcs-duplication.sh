#!/usr/bin/env bash
# CI guard: ensure the ragent-agent crate does not re-introduce local copies
# of GitHub or GitLab VCS tool implementations.
#
# The canonical VCS tool sources live in `crates/ragent-tools-vcs/src/github/`
# and `crates/ragent-tools-vcs/src/gitlab/`.  They are compiled into
# `ragent-agent` via the `ExtractedVcsToolAdapter` wrapper that calls
# `ragent_tools_vcs::registry::create_vcs_registry()` (see
# `crates/ragent-agent/src/tool/mod.rs`).  No `github_*.rs` or `gitlab_*.rs`
# file should exist physically under `crates/ragent-agent/src/tool/`.
#
# See `DUPPLAN.md` Milestone A for the consolidation history.

set -euo pipefail

TOOLS_DUP_DIR="crates/ragent-agent/src/tool"

exit_code=0

# No github_*.rs or gitlab_*.rs file may exist physically under ragent-agent/src/tool/.
for f in "$TOOLS_DUP_DIR"/github_*.rs "$TOOLS_DUP_DIR"/gitlab_*.rs; do
    [ -e "$f" ] || continue
    name=$(basename "$f")
    echo "ERROR: duplicate VCS tool source file detected: $f"
    echo "       GitHub/GitLab tools live canonically in crates/ragent-tools-vcs/src/"
    echo "       and are registered into ragent-agent via the ExtractedVcsToolAdapter"
    echo "       in crates/ragent-agent/src/tool/mod.rs. Remove the local copy."
    exit_code=1
done

if [ "$exit_code" -ne 0 ]; then
    exit "$exit_code"
fi

echo "OK: ragent-agent has no local GitHub/GitLab VCS tool copies (uses ragent-tools-vcs)."